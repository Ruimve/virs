//! Grid engine — manages grid bot lifecycle.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::grid::ai::GridAiService;
use crate::grid::ports::*;
use crate::grid::types::{GridCommand, GridEvent};
use crate::grid::worker::GridWorker;

/// 网格引擎
///
/// 管理所有网格 bot 的生命周期，包括启动、停止、暂停、恢复、删除和调整。
pub struct GridEngine {
    store: Arc<dyn GridStore>,
    ai_service: Arc<GridAiService>,
    price_provider: Arc<dyn PriceProvider>,
    order_executor: Arc<dyn OrderExecutor>,
    market_data_provider: Arc<dyn MarketDataProvider>,
    event_tx: broadcast::Sender<OrderEvent>,
    grid_event_tx: broadcast::Sender<GridEvent>,
    cmd_rx: Option<mpsc::Receiver<GridCommand>>,
    workers: HashMap<Uuid, tokio::task::JoinHandle<()>>,
    shutdown_txs: HashMap<Uuid, mpsc::Sender<()>>,
    adjust_txs: HashMap<Uuid, mpsc::Sender<()>>,
    bot_symbols: HashMap<Uuid, String>,
}

impl GridEngine {
    /// 创建 GridEngine 实例，返回 (engine, command_sender, event_sender)
    pub fn new(
        store: Arc<dyn GridStore>,
        ai_service: Arc<GridAiService>,
        price_provider: Arc<dyn PriceProvider>,
        order_executor: Arc<dyn OrderExecutor>,
        market_data_provider: Arc<dyn MarketDataProvider>,
        event_tx: broadcast::Sender<OrderEvent>,
    ) -> (
        Self,
        mpsc::Sender<GridCommand>,
        broadcast::Sender<GridEvent>,
    ) {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        let (grid_event_tx, _) = broadcast::channel(256);

        let engine = Self {
            store,
            ai_service,
            price_provider,
            order_executor,
            market_data_provider,
            event_tx,
            grid_event_tx: grid_event_tx.clone(),
            cmd_rx: Some(cmd_rx),
            workers: HashMap::new(),
            shutdown_txs: HashMap::new(),
            adjust_txs: HashMap::new(),
            bot_symbols: HashMap::new(),
        };

        (engine, cmd_tx, grid_event_tx)
    }

    /// 启动引擎主循环
    pub async fn run(&mut self) {
        let mut cmd_rx = self.cmd_rx.take().expect("GridEngine already running");
        self.restore_running_bots().await;

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                GridCommand::StartBot { bot_id } => self.start_bot(bot_id).await,
                GridCommand::StopBot { bot_id } => self.stop_bot(bot_id, "user requested").await,
                GridCommand::PauseBot { bot_id } => self.pause_bot(bot_id).await,
                GridCommand::ResumeBot { bot_id } => self.resume_bot(bot_id).await,
                GridCommand::DeleteBot {
                    bot_id,
                    close_position,
                } => self.delete_bot(bot_id, close_position).await,
                GridCommand::AdjustGrid { bot_id } => self.adjust_grid(bot_id).await,
                GridCommand::Shutdown => {
                    self.shutdown_all().await;
                    break;
                }
            }
        }
        info!("GridEngine shutdown complete");
    }

    async fn restore_running_bots(&mut self) {
        let running_bots = match self.store.load_running_bots().await {
            Ok(bots) => bots,
            Err(e) => {
                error!(error = %e, "Failed to load running grid bots, skipping restore");
                return;
            }
        };
        for bot in running_bots {
            info!(bot_id = %bot.id, name = %bot.name, "Restoring running grid bot");
            let _ = self.store.update_bot_status(bot.id, "stopped").await;
            self.start_bot(bot.id).await;
        }
    }

    async fn start_bot(&mut self, bot_id: Uuid) {
        if self.workers.contains_key(&bot_id) {
            warn!(bot_id = %bot_id, "Bot already running");
            return;
        }

        let bot = match self.store.load_bot(bot_id).await {
            Ok(Some(b)) => b,
            Ok(None) => {
                warn!(bot_id = %bot_id, "Bot not found");
                return;
            }
            Err(e) => {
                warn!(bot_id = %bot_id, error = %e, "Failed to load bot");
                return;
            }
        };

        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);
        let (adjust_tx, adjust_rx) = mpsc::channel::<()>(1);
        let event_rx = self.event_tx.subscribe();
        let grid_event_tx = self.grid_event_tx.clone();
        let store = self.store.clone();
        let price_provider = self.price_provider.clone();
        let order_executor = self.order_executor.clone();
        let ai_service = self.ai_service.clone();
        let market_data_provider = self.market_data_provider.clone();
        let bot_symbol = bot.symbol.clone();

        let handle = tokio::spawn(async move {
            let mut worker = GridWorker::new(
                bot,
                price_provider,
                order_executor,
                ai_service,
                store,
                market_data_provider,
                event_rx,
                grid_event_tx,
            );
            worker.run(shutdown_rx, adjust_rx).await;
        });

        self.workers.insert(bot_id, handle);
        self.shutdown_txs.insert(bot_id, shutdown_tx);
        self.adjust_txs.insert(bot_id, adjust_tx);
        self.bot_symbols.insert(bot_id, bot_symbol);

        let _ = self.store.update_bot_status(bot_id, "running").await;
        let _ = self.grid_event_tx.send(GridEvent::BotStarted { bot_id });
        info!(bot_id = %bot_id, "Grid bot started");
    }

    async fn stop_bot(&mut self, bot_id: Uuid, reason: &str) {
        self.stop_or_pause_bot(bot_id, reason, "stopped").await;
    }

    async fn pause_bot(&mut self, bot_id: Uuid) {
        self.stop_or_pause_bot(bot_id, "paused", "paused").await;
    }

    async fn stop_or_pause_bot(&mut self, bot_id: Uuid, reason: &str, target_status: &str) {
        let cancel_symbol = self.bot_symbols.get(&bot_id).cloned();
        let _ = self
            .order_executor
            .send_command(OrderCommand::CancelAllOrders {
                symbol: cancel_symbol,
            })
            .await;

        self.graceful_shutdown_worker(bot_id).await;
        let _ = self.store.update_bot_status(bot_id, target_status).await;
        let _ = self.grid_event_tx.send(GridEvent::BotStopped {
            bot_id,
            reason: reason.to_string(),
        });
        info!(bot_id = %bot_id, "Grid bot {}: {}", target_status, reason);
    }

    async fn graceful_shutdown_worker(&mut self, bot_id: Uuid) {
        if let Some(tx) = self.shutdown_txs.remove(&bot_id) {
            let _ = tx.send(()).await;
        }
        self.adjust_txs.remove(&bot_id);
        self.bot_symbols.remove(&bot_id);
        if let Some(handle) = self.workers.remove(&bot_id) {
            let abort_handle = handle.abort_handle();
            match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                Ok(Ok(())) => {
                    info!(bot_id = %bot_id, "Grid worker exited gracefully");
                }
                Ok(Err(e)) => {
                    warn!(bot_id = %bot_id, error = %e, "Grid worker exited with error");
                }
                Err(_) => {
                    abort_handle.abort();
                    warn!(bot_id = %bot_id, "Grid worker shutdown timed out, aborted");
                }
            }
        }
    }

    async fn resume_bot(&mut self, bot_id: Uuid) {
        let _ = self.store.update_bot_status(bot_id, "stopped").await;
        self.start_bot(bot_id).await;
    }

    async fn delete_bot(&mut self, bot_id: Uuid, close_position: bool) {
        let bot_info = self.store.load_bot(bot_id).await.ok().flatten();
        let symbol = bot_info.as_ref().map(|b| b.symbol.clone());
        let exchange = bot_info.as_ref().map(|b| b.exchange.clone());

        if close_position {
            if let (Some(ref sym), Some(ref ex)) = (&symbol, &exchange) {
                let _ = self
                    .order_executor
                    .send_command(OrderCommand::CancelAllOrders {
                        symbol: Some(sym.clone()),
                    })
                    .await;
                let _ = self
                    .order_executor
                    .send_command(OrderCommand::CloseAllPositions {
                        symbol: sym.clone(),
                        exchange: ex.clone(),
                    })
                    .await;
            }
        }

        self.stop_or_pause_bot(bot_id, "deleted", "stopped").await;
        let _ = self.store.delete_bot(bot_id).await;
        info!(bot_id = %bot_id, close_position, "Grid bot deleted");
    }

    async fn adjust_grid(&mut self, bot_id: Uuid) {
        if let Some(adjust_tx) = self.adjust_txs.get(&bot_id) {
            if let Err(e) = adjust_tx.send(()).await {
                warn!(bot_id = %bot_id, error = %e, "Failed to send adjust signal");
            }
        }
    }

    async fn shutdown_all(&mut self) {
        let bot_ids: Vec<Uuid> = self.workers.keys().copied().collect();
        for id in bot_ids {
            self.stop_bot(id, "engine shutdown").await;
        }
    }
}
