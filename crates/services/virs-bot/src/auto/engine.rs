use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};
use uuid::Uuid;
use virs_runtime::CancellationToken;

use crate::auto::ai::AutoAiService;
use crate::auto::types::AutoCommand;
use crate::auto::worker::AutoWorker;
use virs_types::auto::AutoStore;
use virs_types::bot::{MarketDataProvider, OrderCommand, OrderEvent, OrderExecutor};
use virs_strategy::prompt::PromptLoader;
use virs_config::TimeConfig;
use virs_market::KlineEngine;
use virs_types::position::EngineEvent;

pub struct AutoEngine {
    store: Arc<dyn AutoStore>,
    ai_service: Arc<AutoAiService>,
    kline_engine: Arc<KlineEngine>,
    order_executor: Arc<dyn OrderExecutor>,
    market_data_provider: Arc<dyn MarketDataProvider>,
    event_tx: broadcast::Sender<OrderEvent>,
    pe_event_tx: broadcast::Sender<EngineEvent>,
    cmd_rx: Option<mpsc::Receiver<AutoCommand>>,
    /// worker 的 child_token + JoinHandle，用于单个 worker 优雅停止
    workers: HashMap<Uuid, (CancellationToken, tokio::task::JoinHandle<()>)>,
    cancel: CancellationToken,
    bot_symbols: HashMap<Uuid, String>,

    time_config: TimeConfig,
    prompt_loader: PromptLoader,
}

impl AutoEngine {
    pub fn new(
        store: Arc<dyn AutoStore>,
        ai_service: Arc<AutoAiService>,
        kline_engine: Arc<KlineEngine>,
        order_executor: Arc<dyn OrderExecutor>,
        market_data_provider: Arc<dyn MarketDataProvider>,
        event_tx: broadcast::Sender<OrderEvent>,
        pe_event_tx: broadcast::Sender<EngineEvent>,
        time_config: TimeConfig,
        prompt_loader: PromptLoader,
        parent_cancel: CancellationToken,
    ) -> (Self, mpsc::Sender<AutoCommand>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);

        // 作为 parent_cancel 的子令牌：父取消时级联取消 engine 及所有 worker
        let cancel = parent_cancel.child_token();

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
            cancel,
            bot_symbols: HashMap::new(),
            time_config,
            prompt_loader,
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
                    response_tx,
                } => self.delete_bot(bot_id, close_position, response_tx).await,
            }
        }

        // cmd_rx 返回 None：所有 sender 已 drop，开始关闭流程
        info!("AutoEngine command channel closed, shutting down all workers");

        // 1. 取消所有 worker 的 CancellationToken（child_token 随父取消自动传播）
        self.cancel.cancel();

        // 2. 并发等待所有 worker 退出（总超时 5 秒，而非 N × 5 秒）
        let workers: Vec<(Uuid, CancellationToken, tokio::task::JoinHandle<()>)> =
            self.workers.drain().map(|(id, (cancel, handle))| (id, cancel, handle)).collect();
        let mut join_set = tokio::task::JoinSet::new();
        for (bot_id, _worker_cancel, handle) in workers {
            join_set.spawn(async move {
                let abort_handle = handle.abort_handle();
                match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        warn!(bot_id = %bot_id, error = %e, "Auto worker exited with error during shutdown");
                    }
                    Err(_) => {
                        abort_handle.abort();
                        warn!(bot_id = %bot_id, "Auto worker shutdown timed out during engine shutdown, aborted");
                    }
                }
            });
        }
        while join_set.join_next().await.is_some() {}

        // 3. 清理 bot_symbols
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

        let worker_cancel = self.cancel.child_token();
        let worker_cancel_for_shutdown = worker_cancel.clone();
        let event_rx = self.event_tx.subscribe();
        let pe_event_rx = self.pe_event_tx.subscribe();
        let store = self.store.clone();
        let kline_rx = self.kline_engine.subscribe_events();
        let order_executor = self.order_executor.clone();
        let ai_service = self.ai_service.clone();
        let market_data_provider = self.market_data_provider.clone();
        let bot_symbol = bot.symbol.clone();
        let time_config = self.time_config.clone();
        let prompt_loader = self.prompt_loader.clone();

        let handle = tokio::spawn(async move {
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
            );
            worker.run(worker_cancel).await;
        });

        self.workers.insert(bot_id, (worker_cancel_for_shutdown, handle));
        self.bot_symbols.insert(bot_id, bot_symbol);

        if let Err(e) = self.store.update_bot_status(bot_id, "running").await {
            warn!(bot_id = %bot_id, error = %e, "Failed to update bot status to running");
        }
    }

    async fn stop_bot(&mut self, bot_id: Uuid, reason: &str) {
        self.stop_or_pause_bot(bot_id, reason, "stopped", true).await;
    }

    /// Stop or pause a bot.
    ///
    /// `cancel_orders`: when true, cancel all open orders for the bot's symbol first.
    /// Set to false when the caller has already cancelled orders (e.g. delete_bot
    /// cancels before closing positions).
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
        // 先取消 child_token，让 worker 通过 select! 优雅退出
        self.bot_symbols.remove(&bot_id);
        if let Some((worker_cancel, handle)) = self.workers.remove(&bot_id) {
            worker_cancel.cancel();
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

    async fn delete_bot(
        &mut self,
        bot_id: Uuid,
        close_position: bool,
        response_tx: tokio::sync::oneshot::Sender<Result<(), String>>,
    ) {
        // 1. Load bot info — if DB fails, must NOT proceed (would risk orphan positions)
        let bot_info = match self.store.load_bot(bot_id).await {
            Ok(Some(info)) => info,
            Ok(None) => {
                // Bot already deleted — idempotent success
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

        // 2. Cancel orders and close positions (abort on failure to avoid orphan positions)
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

        // 3. Stop worker (skip CancelAllOrders — already done above when close_position=true)
        self.stop_or_pause_bot(bot_id, "deleted", "stopped", !close_position)
            .await;

        // 4. Delete from DB
        if let Err(e) = self.store.delete_bot(bot_id).await {
            error!(bot_id = %bot_id, error = %e, "Failed to delete bot from database");
            let _ = response_tx.send(Err(format!("Failed to delete bot from database: {e}")));
            return;
        }

        info!(bot_id = %bot_id, "Bot deleted successfully");
        let _ = response_tx.send(Ok(()));
    }
}
