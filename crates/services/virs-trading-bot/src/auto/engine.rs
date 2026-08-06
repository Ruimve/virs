use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};
use uuid::Uuid;
use virs_task::{spawn, Stop, TaskHandle};

use crate::auto::ai::AutoAiService;
use crate::auto::types::AutoCommand;
use crate::auto::worker::AutoWorker;
use virs_type::{AutoStore, KlineEventSource};
use virs_type::{MarketDataProvider, OrderCommand, OrderEvent, OrderExecutor};
use virs_prompt::PromptProvider;
use virs_config::TimeConfig;
use virs_type::EngineEvent;

pub struct AutoEngine {
    store: Arc<dyn AutoStore>,
    ai_service: Arc<AutoAiService>,
    kline_engine: Arc<dyn KlineEventSource>,
    order_executor: Arc<dyn OrderExecutor>,
    market_data_provider: Arc<dyn MarketDataProvider>,
    event_tx: broadcast::Sender<OrderEvent>,
    pe_event_tx: broadcast::Sender<EngineEvent>,
    cmd_rx: Option<mpsc::Receiver<AutoCommand>>,
    workers: HashMap<Uuid, TaskHandle>,
    bot_symbols: HashMap<Uuid, String>,

    time_config: TimeConfig,
    prompt_loader: Arc<dyn PromptProvider>,
    strategy_engine: Option<Arc<dyn virs_prompt::StrategyHotSwapSource>>,
}

impl AutoEngine {
    pub fn new(
        store: Arc<dyn AutoStore>,
        ai_service: Arc<AutoAiService>,
        kline_engine: Arc<dyn KlineEventSource>,
        order_executor: Arc<dyn OrderExecutor>,
        market_data_provider: Arc<dyn MarketDataProvider>,
        event_tx: broadcast::Sender<OrderEvent>,
        pe_event_tx: broadcast::Sender<EngineEvent>,
        time_config: TimeConfig,
        prompt_loader: Arc<dyn PromptProvider>,
        strategy_engine: Option<Arc<dyn virs_prompt::StrategyHotSwapSource>>,
    ) -> (Self, mpsc::Sender<AutoCommand>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);

        let engine = Self {
            store,
            ai_service,
            kline_engine,
            order_executor,
            market_data_provider,
            event_tx,
            pe_event_tx,
            cmd_rx: Some(cmd_rx),
            workers: HashMap::new(),
            bot_symbols: HashMap::new(),
            time_config,
            prompt_loader,
            strategy_engine,
        };

        (engine, cmd_tx)
    }

    pub async fn run(&mut self, stop: Stop) {
        let mut cmd_rx = match self.cmd_rx.take() {
            Some(rx) => rx,
            None => {
                error!("AutoEngine already running — run() called twice. Skipping.");
                return;
            }
        };
        self.restore_running_bots().await;

        loop {
            tokio::select! {
                _ = stop.cancelled() => break,
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(AutoCommand::StartBot { bot_id }) => self.start_bot(bot_id).await,
                        Some(AutoCommand::StopBot { bot_id }) => self.stop_bot(bot_id, "user requested").await,
                        Some(AutoCommand::DeleteBot {
                            bot_id,
                            close_position,
                            response_tx,
                        }) => self.delete_bot(bot_id, close_position, response_tx).await,
                        None => break,
                    }
                }
            }
        }

        info!("AutoEngine shutting down all workers");

        let handles: Vec<TaskHandle> = self.workers.drain().map(|(_, h)| h).collect();
        for h in &handles {
            h.cancel();
        }
        let mut join_set = tokio::task::JoinSet::new();
        for h in handles {
            join_set.spawn(h.join());
        }
        while join_set.join_next().await.is_some() {}

        self.bot_symbols.clear();

        info!("AutoEngine shutdown complete");
    }

    async fn restore_running_bots(&mut self) {
        let running_bots = match self.store.load_running_bots().await {
            Ok(bots) => bots,
            Err(e) => {
                error!(error = %e, "Failed to load running auto bots, skipping restore");
                return;
            }
        };
        for bot in running_bots {
            if let Err(e) = self.store.update_bot_status(bot.id, "stopped").await {
                warn!(bot_id = %bot.id, error = %e, "Failed to update bot status to stopped");
            }
            self.start_bot(bot.id).await;
        }
    }

    async fn start_bot(&mut self, bot_id: Uuid) {
        if self.workers.contains_key(&bot_id) {
            warn!(bot_id = %bot_id, "Auto bot already running");
            return;
        }

        let bot = match self.store.load_bot(bot_id).await {
            Ok(Some(b)) => b,
            Ok(None) => {
                warn!(bot_id = %bot_id, "Auto bot not found");
                return;
            }
            Err(e) => {
                warn!(bot_id = %bot_id, error = %e, "Failed to load auto bot");
                return;
            }
        };

        let event_rx = self.event_tx.subscribe();
        let pe_event_rx = self.pe_event_tx.subscribe();
        let store = self.store.clone();
        let kline_rx = self.kline_engine.subscribe_kline_events();
        let order_executor = self.order_executor.clone();
        let ai_service = self.ai_service.clone();
        let market_data_provider = self.market_data_provider.clone();
        let bot_symbol = bot.symbol.clone();
        let time_config = self.time_config.clone();
        let prompt_loader = self.prompt_loader.clone();
        let strategy_update_rx = if bot.auto_optimize_enabled {
            self.strategy_engine.as_ref().map(|se| se.subscribe())
        } else {
            None
        };

        let handle = spawn("auto_worker", move |stop: Stop| async move {
            let worker = AutoWorker::new(
                bot,
                kline_rx,
                order_executor,
                ai_service,
                store,
                market_data_provider,
                event_rx,
                pe_event_rx,
                time_config,
                prompt_loader,
                strategy_update_rx,
            );
            worker.run(stop).await;
        });

        self.workers.insert(bot_id, handle);
        self.bot_symbols.insert(bot_id, bot_symbol);

        if let Err(e) = self.store.update_bot_status(bot_id, "running").await {
            warn!(bot_id = %bot_id, error = %e, "Failed to update bot status to running");
        }
    }

    async fn stop_bot(&mut self, bot_id: Uuid, reason: &str) {
        self.stop_or_pause_bot(bot_id, reason, "stopped", true).await;
    }

    async fn stop_or_pause_bot(
        &mut self,
        bot_id: Uuid,
        _reason: &str,
        target_status: &str,
        cancel_orders: bool,
    ) {
        if cancel_orders {
            if let Some(sym) = self.bot_symbols.get(&bot_id).cloned() {
                if let Err(e) = self
                    .order_executor
                    .send_command(OrderCommand::CancelAllOrders {
                        symbol: Some(sym),
                    })
                    .await
                {
                    warn!(bot_id = %bot_id, error = %e, "Failed to send CancelAllOrders command");
                }
            }
        }

        self.graceful_shutdown_worker(bot_id).await;
        if let Err(e) = self.store.update_bot_status(bot_id, target_status).await {
            warn!(bot_id = %bot_id, error = %e, target_status = %target_status, "Failed to update bot status");
        }
    }

    async fn graceful_shutdown_worker(&mut self, bot_id: Uuid) {
        self.bot_symbols.remove(&bot_id);
        if let Some(handle) = self.workers.remove(&bot_id) {
            handle.cancel();
            handle.join().await;
        }
    }

    async fn delete_bot(
        &mut self,
        bot_id: Uuid,
        close_position: bool,
        response_tx: tokio::sync::oneshot::Sender<Result<(), String>>,
    ) {
        let bot_info = match self.store.load_bot(bot_id).await {
            Ok(Some(info)) => info,
            Ok(None) => {
                warn!(bot_id = %bot_id, "Bot not found during deletion (already deleted?)");
                let _ = response_tx.send(Ok(()));
                return;
            }
            Err(e) => {
                error!(bot_id = %bot_id, error = %e, "Failed to load bot info for deletion — aborting");
                let _ = response_tx.send(Err(format!("Failed to load bot: {e}")));
                return;
            }
        };

        let symbol = &bot_info.symbol;
        let exchange = &bot_info.exchange;

        if close_position {
            if let Err(e) = self
                .order_executor
                .send_command(OrderCommand::CancelAllOrders {
                    symbol: Some(symbol.clone()),
                })
                .await
            {
                error!(bot_id = %bot_id, error = %e, "Failed to cancel orders during deletion — aborting");
                let _ = response_tx.send(Err(format!("Failed to cancel orders: {e}")));
                return;
            }
            if let Err(e) = self
                .order_executor
                .send_command(OrderCommand::CloseAllPositions {
                    symbol: symbol.clone(),
                    exchange: exchange.clone(),
                })
                .await
            {
                error!(bot_id = %bot_id, error = %e, "Failed to close positions during deletion — aborting");
                let _ = response_tx.send(Err(format!("Failed to close positions: {e}")));
                return;
            }
        }

        self.stop_or_pause_bot(bot_id, "deleted", "stopped", !close_position)
            .await;

        if let Err(e) = self.store.delete_bot(bot_id).await {
            error!(bot_id = %bot_id, error = %e, "Failed to delete bot from database");
            let _ = response_tx.send(Err(format!("Failed to delete bot from database: {e}")));
            return;
        }

        info!(bot_id = %bot_id, "Bot deleted successfully");
        let _ = response_tx.send(Ok(()));
    }
}
