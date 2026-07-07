//! Auto engine — manages auto trading bot lifecycle.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::auto::ai::AutoAiService;
use crate::auto::ports::*;
use crate::auto::types::AutoCommand;
use crate::auto::worker::AutoWorker;
use virs_config::TimeConfig;
use virs_types::position::EngineEvent;

/// 全自动交易引擎
pub struct AutoEngine {
    store: Arc<dyn AutoStore>,
    ai_service: Arc<AutoAiService>,
    price_provider: Arc<dyn PriceProvider>,
    order_executor: Arc<dyn OrderExecutor>,
    market_data_provider: Arc<dyn MarketDataProvider>,
    event_tx: broadcast::Sender<OrderEvent>,
    pe_event_tx: broadcast::Sender<EngineEvent>,
    cmd_rx: Option<mpsc::Receiver<AutoCommand>>,
    workers: HashMap<Uuid, tokio::task::JoinHandle<()>>,
    shutdown_txs: HashMap<Uuid, mpsc::Sender<()>>,
    bot_symbols: HashMap<Uuid, String>,
    /// T12: 时间配置（从环境变量加载，替代硬编码常量）
    time_config: TimeConfig,
}

impl AutoEngine {
    pub fn new(
        store: Arc<dyn AutoStore>,
        ai_service: Arc<AutoAiService>,
        price_provider: Arc<dyn PriceProvider>,
        order_executor: Arc<dyn OrderExecutor>,
        market_data_provider: Arc<dyn MarketDataProvider>,
        event_tx: broadcast::Sender<OrderEvent>,
        pe_event_tx: broadcast::Sender<EngineEvent>,
        time_config: TimeConfig,
    ) -> (Self, mpsc::Sender<AutoCommand>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);

        let engine = Self {
            store,
            ai_service,
            price_provider,
            order_executor,
            market_data_provider,
            event_tx,
            pe_event_tx,
            cmd_rx: Some(cmd_rx),
            workers: HashMap::new(),
            shutdown_txs: HashMap::new(),
            bot_symbols: HashMap::new(),
            time_config,
        };

        (engine, cmd_tx)
    }

    pub async fn run(&mut self) {
        let mut cmd_rx = match self.cmd_rx.take() {
            Some(rx) => rx,
            None => {
                error!("AutoEngine already running — run() called twice. Skipping.");
                return;
            }
        };
        self.restore_running_bots().await;

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                AutoCommand::StartBot { bot_id } => self.start_bot(bot_id).await,
                AutoCommand::StopBot { bot_id } => self.stop_bot(bot_id, "user requested").await,
                AutoCommand::DeleteBot {
                    bot_id,
                    close_position,
                } => self.delete_bot(bot_id, close_position).await,
            }
        }
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

        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);
        let event_rx = self.event_tx.subscribe();
        let pe_event_rx = self.pe_event_tx.subscribe();
        let store = self.store.clone();
        let price_provider = self.price_provider.clone();
        let order_executor = self.order_executor.clone();
        let ai_service = self.ai_service.clone();
        let market_data_provider = self.market_data_provider.clone();
        let bot_symbol = bot.symbol.clone();
        let time_config = self.time_config.clone();

        let handle = tokio::spawn(async move {
            let mut worker = AutoWorker::new(
                bot,
                price_provider,
                order_executor,
                ai_service,
                store,
                market_data_provider,
                event_rx,
                pe_event_rx,
                time_config,
            );
            worker.run(shutdown_rx).await;
        });

        self.workers.insert(bot_id, handle);
        self.shutdown_txs.insert(bot_id, shutdown_tx);
        self.bot_symbols.insert(bot_id, bot_symbol);

        if let Err(e) = self.store.update_bot_status(bot_id, "running").await {
            warn!(bot_id = %bot_id, error = %e, "Failed to update bot status to running");
        }
    }

    async fn stop_bot(&mut self, bot_id: Uuid, reason: &str) {
        self.stop_or_pause_bot(bot_id, reason, "stopped").await;
    }

    async fn stop_or_pause_bot(&mut self, bot_id: Uuid, _reason: &str, target_status: &str) {
        let cancel_symbol = self.bot_symbols.get(&bot_id).cloned();
        if let Err(e) = self
            .order_executor
            .send_command(OrderCommand::CancelAllOrders {
                symbol: cancel_symbol,
            })
            .await
        {
            warn!(bot_id = %bot_id, error = %e, "Failed to send CancelAllOrders command");
        }

        self.graceful_shutdown_worker(bot_id).await;
        if let Err(e) = self.store.update_bot_status(bot_id, target_status).await {
            warn!(bot_id = %bot_id, error = %e, "Failed to update bot status to {}", target_status);
        }
    }

    async fn graceful_shutdown_worker(&mut self, bot_id: Uuid) {
        if let Some(tx) = self.shutdown_txs.remove(&bot_id) {
            if let Err(e) = tx.send(()).await {
                warn!(bot_id = %bot_id, error = %e, "Failed to send shutdown signal — worker may have already exited");
            }
        }
        self.bot_symbols.remove(&bot_id);
        if let Some(handle) = self.workers.remove(&bot_id) {
            let abort_handle = handle.abort_handle();
            match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    warn!(bot_id = %bot_id, error = %e, "Auto worker exited with error");
                }
                Err(_) => {
                    abort_handle.abort();
                    warn!(bot_id = %bot_id, "Auto worker shutdown timed out, aborted");
                }
            }
        }
    }

    async fn delete_bot(&mut self, bot_id: Uuid, close_position: bool) {
        let bot_info = match self.store.load_bot(bot_id).await {
            Ok(info) => info,
            Err(e) => {
                warn!(bot_id = %bot_id, error = %e, "Failed to load bot info for deletion");
                None
            }
        };
        let symbol = bot_info.as_ref().map(|b| b.symbol.clone());
        let exchange = bot_info.as_ref().map(|b| b.exchange.clone());

        if close_position {
            if let (Some(ref sym), Some(ref ex)) = (&symbol, &exchange) {
                if let Err(e) = self
                    .order_executor
                    .send_command(OrderCommand::CancelAllOrders {
                        symbol: Some(sym.clone()),
                    })
                    .await
                {
                    warn!(bot_id = %bot_id, error = %e, "Failed to cancel orders during bot deletion");
                }
                if let Err(e) = self
                    .order_executor
                    .send_command(OrderCommand::CloseAllPositions {
                        symbol: sym.clone(),
                        exchange: ex.clone(),
                    })
                    .await
                {
                    error!(bot_id = %bot_id, error = %e, "Failed to close positions during bot deletion");
                }
            }
        }

        self.stop_or_pause_bot(bot_id, "deleted", "stopped").await;
        if let Err(e) = self.store.delete_bot(bot_id).await {
            error!(bot_id = %bot_id, error = %e, "Failed to delete bot from database");
        }
    }
}
