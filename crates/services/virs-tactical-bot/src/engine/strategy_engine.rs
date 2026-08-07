//! 策略引擎：定时评估策略绩效，对低分策略调用 LLM 优化，热切换到新版本。
//!
//! 工作流程：
//! 1. 每个分析周期遍历 PromptLoader 中所有已加载的策略模板
//! 2. 对每个策略调用 StrategyEvaluator 评估绩效
//! 3. 如果绩效低于阈值（needs_optimization），调用 StrategyOptimizer 优化
//! 4. 优化成功后：
//!    a. 调用 writer::save_template 写入磁盘（版本号已递增）
//!    b. 调用 PromptLoader::upsert 更新内存缓存（热切换）
//!    c. 通过 watch::channel 通知 virs-trading-bot

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tracing::{error, info, warn};
use virs_error::{VirsError, VirsResult};
use virs_prompt::{PromptLoader, StrategyHotSwapSource, StrategySwapEvent, save_template};
use virs_task::{spawn_periodic, TaskHandle};
use virs_type::StrategyType;

use super::evaluator::StrategyEvaluator;
use super::optimizer::StrategyOptimizer;
use super::types::StrategyEngineConfig;
use virs_type::TradeHistoryProvider;

/// 策略引擎。
pub(crate) struct StrategyEngine {
    config: StrategyEngineConfig,
    prompt_loader: PromptLoader,
    evaluator: Arc<StrategyEvaluator>,
    optimizer: Arc<StrategyOptimizer>,
    update_tx: watch::Sender<Option<StrategySwapEvent>>,
}

impl StrategyEngine {
    /// 创建策略引擎。
    ///
    /// `history` 由应用层提供（从数据库查询交易记录）。
    pub(crate) fn new(
        config: StrategyEngineConfig,
        prompt_loader: PromptLoader,
        history: Box<dyn TradeHistoryProvider>,
        http_client: reqwest::Client,
    ) -> Self {
        let evaluator = Arc::new(StrategyEvaluator::new(history));
        let optimizer = Arc::new(StrategyOptimizer::new(
            http_client,
            config.llm_api_key.clone(),
            config.llm_base_url.clone(),
            config.llm_model.clone(),
        ));

        let (update_tx, _initial_rx) = watch::channel(None);

        Self {
            config,
            prompt_loader,
            evaluator,
            optimizer,
            update_tx,
        }
    }

    /// 启动定时分析循环。返回 TaskHandle 用于停止。
    pub(crate) fn start(self: Arc<Self>) -> TaskHandle {
        let interval = Duration::from_secs(self.config.analysis_interval_secs);
        let engine = Arc::clone(&self);

        spawn_periodic(
            "strategy_engine",
            interval,
            false, // 首次延迟一个周期，避免启动时立即触发
            move || {
                let engine = Arc::clone(&engine);
                async move {
                    if let Err(e) = engine.run_cycle().await {
                        error!(error = %e, "StrategyEngine cycle failed");
                    }
                }
            },
        )
    }

    /// 执行一次完整的评估 + 优化循环。
    pub(crate) async fn run_cycle(&self) -> VirsResult<()> {
        info!("StrategyEngine cycle started");

        // 获取所有已加载的 Auto 策略名
        let strategy_names = self.prompt_loader.list(StrategyType::Auto).await;

        if strategy_names.is_empty() {
            info!("No strategies loaded — skipping cycle");
            return Ok(());
        }

        info!(count = strategy_names.len(), "Evaluating strategies");

        for name in &strategy_names {
            if let Err(e) = self.evaluate_and_optimize(name).await {
                warn!(strategy = %name, error = %e, "Failed to evaluate/optimize strategy");
            }
        }

        info!("StrategyEngine cycle completed");
        Ok(())
    }

    /// 评估单个策略，必要时触发优化。
    async fn evaluate_and_optimize(&self, name: &str) -> VirsResult<()> {
        // 1. 评估
        let metrics = self
            .evaluator
            .evaluate(name, self.config.evaluation_window_secs)
            .await
            .ok_or_else(|| {
                VirsError::config(format!(
                    "No trades found for strategy '{}'",
                    name
                ))
            })?;

        info!(
            strategy = %name,
            total_trades = metrics.total_trades,
            win_rate = format!("{:.1}%", metrics.win_rate * 100.0),
            total_pnl = metrics.total_pnl,
            score = metrics.composite_score(),
            "Strategy evaluated"
        );

        // 2. 判断是否需要优化
        if !metrics.needs_optimization(
            self.config.min_trades_for_optimization,
            self.config.optimization_score_threshold,
        ) {
            info!(strategy = %name, score = metrics.composite_score(), "Strategy performing well — no optimization needed");
            return Ok(());
        }

        // 3. 获取当前模板
        let current = self
            .prompt_loader
            .get(StrategyType::Auto, name)
            .await
            .ok_or_else(|| {
                VirsError::config(format!(
                    "Strategy '{}' not found in loader",
                    name
                ))
            })?;

        // 4. 版本号上限检查
        if current.version >= self.config.max_version {
            warn!(
                strategy = %name,
                version = current.version,
                max = self.config.max_version,
                "Strategy reached max version — skipping optimization"
            );
            return Ok(());
        }

        // 5. 调用 LLM 优化
        info!(
            strategy = %name,
            old_version = current.version,
            score = metrics.composite_score(),
            "Triggering LLM optimization"
        );

        let optimization_result = self
            .optimizer
            .optimize(&current, &metrics)
            .await
            .map_err(|e| VirsError::config(format!("Optimization failed: {}", e)))?;

        let old_version = current.version;
        let new_version = optimization_result.template.version;

        info!(
            strategy = %name,
            old_version,
            new_version,
            model = %optimization_result.used_model,
            "Strategy optimized by LLM"
        );

        // 6. 写入磁盘
        if let Err(e) = save_template(&optimization_result.template, true) {
            error!(strategy = %name, error = %e, "Failed to save optimized template to disk");
            return Err(VirsError::config(format!("Save failed: {}", e)));
        }

        // 7. 热切换：更新内存缓存
        self.prompt_loader
            .upsert(optimization_result.template)
            .await;

        // 8. 通知 virs-trading-bot
        let event = StrategySwapEvent {
            strategy_name: name.to_string(),
            old_version,
            new_version,
        };

        info!(
            strategy = %name,
            old_version,
            new_version,
            "Strategy hot-swapped"
        );

        if self.update_tx.send(Some(event)).is_err() {
            warn!("Strategy update receiver dropped — no active subscribers");
        }

        Ok(())
    }
}

impl StrategyHotSwapSource for StrategyEngine {
    fn subscribe(&self) -> watch::Receiver<Option<StrategySwapEvent>> {
        self.update_tx.subscribe()
    }
}

/// 工厂函数：创建 StrategyEngine，启动定时分析循环，并返回 trait 对象和任务句柄。
///
/// `start()` 需要 `self: Arc<Self>`，且 `StrategyEngine` 为 `pub(crate)`，
/// 因此在工厂函数内部创建并启动后返回 `Arc<dyn StrategyHotSwapSource>` 和 `TaskHandle`。
pub fn create_strategy_engine(
    config: StrategyEngineConfig,
    prompt_loader: PromptLoader,
    history: Box<dyn TradeHistoryProvider>,
    http_client: reqwest::Client,
) -> (Arc<dyn StrategyHotSwapSource>, TaskHandle) {
    let engine = Arc::new(StrategyEngine::new(config, prompt_loader, history, http_client));
    let task = Arc::clone(&engine).start();
    (engine, task)
}
