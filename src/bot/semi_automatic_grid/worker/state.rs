use std::time::Duration;

use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};

use crate::bot::semi_automatic_grid::worker::GridWorker;

impl GridWorker {
/** 主运行循环

启动后先获取初始价格、加载历史成交、挂初始订单，
然后进入 select 循环处理价格 tick、LLM 决策、订单事件和外部命令 */
    pub async fn run(&mut self, mut shutdown_rx: mpsc::Receiver<()>, mut adjust_rx: mpsc::Receiver<()>) {
        info!(
            bot_id = %self.bot.id,
            symbol = %self.bot.symbol,
            grid_count = self.bot.grid_count,
            "GridWorker starting"
        );

        if self.levels.is_empty() {
            if self.bot.dynamic_adjust {
                warn!(bot_id = %self.bot.id, "No grid levels yet, will trigger initial LLM analysis after price fetch");
            } else {
                error!(bot_id = %self.bot.id, "No grid levels calculated and dynamic_adjust is disabled, check bot parameters");
                return;
            }
        }

        /* 尝试获取初始价格，最多重试 10 次 */
        for attempt in 1..=10 {
            self.current_price = self.fetch_current_price().await;
            if self.current_price > 0.0 {
                break;
            }
            warn!(bot_id = %self.bot.id, attempt, "Failed to fetch initial price, retrying in 5s...");
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        if self.current_price <= 0.0 {
            error!(bot_id = %self.bot.id, "Failed to fetch initial price after 10 attempts, worker will continue retrying in main loop");
        } else {
            info!(bot_id = %self.bot.id, price = self.current_price, "Initial price fetched");
        }

        self.load_existing_trades().await;

        if self.bot.upper_price <= 0.0 || self.bot.lower_price <= 0.0 || self.levels.is_empty() {
            info!(bot_id = %self.bot.id, "Grid parameters empty, triggering initial LLM analysis");
            self.on_llm_decision().await;
            if self.levels.is_empty() {
                error!(bot_id = %self.bot.id, "Initial LLM analysis did not set grid parameters, worker cannot continue");
                return;
            }
        }

        self.place_initial_orders().await;

        let mut price_tick = tokio::time::interval(Duration::from_secs(5));

        /* 启动 LLM 周期性分析定时器 */
        let (llm_signal_tx, mut llm_signal_rx) = mpsc::channel::<()>(1);
        if self.bot.dynamic_adjust {
            let interval_secs = self.bot.adjust_interval_secs.max(60) as u64;
            info!(bot_id = %self.bot.id, interval_secs, "LLM periodic analysis enabled");
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(interval_secs));
                tick.tick().await;
                loop {
                    tick.tick().await;
                    if llm_signal_tx.send(()).await.is_err() {
                        break;
                    }
                }
            });
        } else {
            info!(bot_id = %self.bot.id, "LLM periodic analysis disabled (dynamic_adjust=false)");
        }

        /* 主事件循环 */
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!(bot_id = %self.bot.id, "GridWorker shutting down");
                    break;
                }
                Some(()) = adjust_rx.recv() => {
                    self.on_adjust_signal().await;
                }
                _ = price_tick.tick() => {
                    self.current_price = self.fetch_current_price().await;
                    if !self.paused {
                        self.on_price_tick().await;
                    }
                }
                Some(()) = llm_signal_rx.recv() => {
                    self.on_llm_decision().await;
                }
                event = self.event_rx.recv() => {
                    match event {
                        Ok(event) => self.on_order_event(event).await,
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(bot_id = %self.bot.id, lagged = n, "Event lagged, clearing pending orders to prevent stale state");
                            self.clear_pending_orders();
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            warn!(bot_id = %self.bot.id, "Event channel closed");
                            break;
                        }
                    }
                }
            }
        }

        self.save_stats().await;
    }

/** 获取实时价格

优先从价格提供者获取，失败时回退到上次缓存价格 */
    pub(crate) async fn fetch_current_price(&self) -> f64 {
        match self.price_provider.get_price(&self.bot.exchange, &self.bot.symbol).await {
            Some(price) if price > 0.0 => price,
            _ => self.current_price,
        }
    }
}
