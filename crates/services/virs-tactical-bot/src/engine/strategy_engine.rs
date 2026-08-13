

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tracing::{error, info, warn};
use virs_error::{VirsError, VirsResult};
use virs_prompt::{PromptLoader, StrategyHotSwapSource, StrategySwapEvent, save_new_version};
use virs_task::{spawn_periodic, TaskHandle};
use virs_type::StrategyType;

use super::evaluator::StrategyEvaluator;
use super::optimizer::StrategyOptimizer;
use super::types::StrategyEngineConfig;
use virs_type::TradeHistoryProvider;


/* 策略引擎：周期性评估策略绩效，低于阈值时调用LLM优化prompt，并通过watch::channel推送热更新 */
pub(crate) struct StrategyEngine {
    config: StrategyEngineConfig,
    prompt_loader: PromptLoader,
    evaluator: Arc<StrategyEvaluator>,
    optimizer: Arc<StrategyOptimizer>,
    update_tx: watch::Sender<Option<StrategySwapEvent>>,
}

impl StrategyEngine {


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


    /* 启动周期性策略评估任务，通过virs-task的spawn_periodic调度 */
    pub(crate) fn start(self: Arc<Self>) -> TaskHandle {
        let interval = Duration::from_secs(self.config.analysis_interval_secs);
        let engine = Arc::clone(&self);

        spawn_periodic(
            "strategy_engine",
            interval,
            false,
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


    pub(crate) async fn run_cycle(&self) -> VirsResult<()> {
        info!("StrategyEngine cycle started");


        let strategy_names = self.prompt_loader.list(StrategyType::Chat).await;

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


    /* 评估并优化单个策略：查询绩效 -> 判断是否需要优化 -> 调用LLM优化 -> 保存 -> 推送热更新 */
    async fn evaluate_and_optimize(&self, name: &str) -> VirsResult<()> {

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


        /* 综合评分高于阈值且交易数充足时跳过优化，避免对表现良好的策略过度干预 */
        if !metrics.needs_optimization(
            self.config.min_trades_for_optimization,
            self.config.optimization_score_threshold,
        ) {
            info!(strategy = %name, score = metrics.composite_score(), "Strategy performing well — no optimization needed");
            return Ok(());
        }


        let current = self
            .prompt_loader
            .get(StrategyType::Chat, name)
            .await
            .ok_or_else(|| {
                VirsError::config(format!(
                    "Strategy '{}' not found in loader",
                    name
                ))
            })?;


        /* 策略版本达到上限后停止优化，防止无限迭代 */
        if current.version >= self.config.max_version {
            warn!(
                strategy = %name,
                version = current.version,
                max = self.config.max_version,
                "Strategy reached max version — skipping optimization"
            );
            return Ok(());
        }


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


        if let Err(e) = save_new_version(&optimization_result.template) {
            error!(strategy = %name, error = %e, "Failed to save optimized template to disk");
            return Err(VirsError::config(format!("Save failed: {}", e)));
        }


        self.prompt_loader
            .upsert(optimization_result.template)
            .await;


        /* 通过watch::channel推送策略热更新事件，所有订阅的worker会收到通知 */
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

/* 实现StrategyHotSwapSource trait，供BotEngine以Arc<dyn StrategyHotSwapSource>持有 */
impl StrategyHotSwapSource for StrategyEngine {
    fn subscribe(&self) -> watch::Receiver<Option<StrategySwapEvent>> {
        self.update_tx.subscribe()
    }
}


/* 工厂函数：创建StrategyEngine并启动周期任务，返回trait object和任务句柄 */
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
