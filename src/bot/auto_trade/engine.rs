use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};
use tracing::{info, warn};
use uuid::Uuid;

use crate::bot::auto_trade::ai::AutoAiService;
use crate::bot::auto_trade::ports::*;
use crate::bot::auto_trade::types::{AutoCommand, AutoEvent};
use crate::bot::auto_trade::worker::AutoWorker;
use crate::engine::kline::KlineEngine;
use crate::engine::kline::types::MarketType;

#[cfg(test)]
pub struct NoopLlmResolver;

#[cfg(test)]
impl LlmProviderResolver for NoopLlmResolver {
    fn is_available(&self) -> bool {
        false
    }

    fn resolve(
        &self,
        _user_credentials: &[(String, String)],
    ) -> anyhow::Result<(String, String, String, String)> {
        anyhow::bail!("No LLM provider configured")
    }
}

pub struct AutoEngine {
    store: Arc<dyn AutoStore>,
    ai_service: Arc<AutoAiService>,
    price_provider: Arc<dyn PriceProvider>,
    order_executor: Arc<dyn OrderExecutor>,
    market_data_provider: Arc<dyn MarketDataProvider>,
    event_tx: broadcast::Sender<OrderEvent>,
    auto_event_tx: broadcast::Sender<AutoEvent>,
    cmd_rx: Option<mpsc::Receiver<AutoCommand>>,
    workers: HashMap<Uuid, tokio::task::JoinHandle<()>>,
    shutdown_txs: HashMap<Uuid, mpsc::Sender<()>>,
    bot_symbols: HashMap<Uuid, String>,
    kline_engine: Option<Arc<KlineEngine>>,
}

impl AutoEngine {
    pub fn new(
        store: Arc<dyn AutoStore>,
        ai_service: Arc<AutoAiService>,
        price_provider: Arc<dyn PriceProvider>,
        order_executor: Arc<dyn OrderExecutor>,
        market_data_provider: Arc<dyn MarketDataProvider>,
        event_tx: broadcast::Sender<OrderEvent>,
        kline_engine: Option<Arc<KlineEngine>>,
    ) -> (Self, mpsc::Sender<AutoCommand>, broadcast::Sender<AutoEvent>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        let (auto_event_tx, _) = broadcast::channel(256);

        let engine = Self {
            store,
            ai_service,
            price_provider,
            order_executor,
            market_data_provider,
            event_tx,
            auto_event_tx: auto_event_tx.clone(),
            cmd_rx: Some(cmd_rx),
            workers: HashMap::new(),
            shutdown_txs: HashMap::new(),
            bot_symbols: HashMap::new(),
            kline_engine,
        };

        (engine, cmd_tx, auto_event_tx)
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<AutoEvent> {
        self.auto_event_tx.subscribe()
    }

    pub async fn run(&mut self) {
        let mut cmd_rx = self.cmd_rx.take().expect("AutoEngine already running");

        self.restore_running_bots().await;

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                AutoCommand::StartBot { bot_id } => self.start_bot(bot_id).await,
                AutoCommand::StopBot { bot_id } => self.stop_bot(bot_id, "user requested").await,
                AutoCommand::PauseBot { bot_id } => self.pause_bot(bot_id).await,
                AutoCommand::ResumeBot { bot_id } => self.resume_bot(bot_id).await,
                AutoCommand::DeleteBot { bot_id, close_position } => self.delete_bot(bot_id, close_position).await,
                AutoCommand::Shutdown => {
                    self.shutdown_all().await;
                    break;
                }
            }
        }

        info!("AutoEngine shutdown complete");
    }

    pub(crate) async fn restore_running_bots(&mut self) {
        let running_bots = self.store.load_running_bots().await.unwrap_or_default();
        for bot in running_bots {
            info!(bot_id = %bot.id, name = %bot.name, "Restoring running auto bot");
            let _ = self.store.update_bot_status(bot.id, "stopped").await;
            self.start_bot(bot.id).await;
        }
    }

    pub(crate) async fn start_bot(&mut self, bot_id: Uuid) {
        if self.workers.contains_key(&bot_id) {
            warn!(bot_id = %bot_id, "Auto bot already running");
            return;
        }

        let mut bot = match self.store.load_bot(bot_id).await {
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

        if !bot.symbol.contains('/') {
            let normalized = crate::api::normalize_symbol(&bot.symbol);
            if normalized != bot.symbol {
                warn!(bot_id = %bot_id, old = %bot.symbol, new = %normalized, "Normalizing symbol format");
                bot.symbol = normalized;
            }
        }

        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);
        let event_rx = self.event_tx.subscribe();
        let auto_event_tx = self.auto_event_tx.clone();
        let store = self.store.clone();
        let price_provider = self.price_provider.clone();
        let order_executor = self.order_executor.clone();
        let ai_service = self.ai_service.clone();
        let market_data_provider = self.market_data_provider.clone();
        let bot_symbol = bot.symbol.clone();

        if let Some(ref engine) = self.kline_engine {
            let mt = match bot.market_type {
                crate::bot::auto_trade::types::MarketType::Perpetual => MarketType::Perpetual,
                crate::bot::auto_trade::types::MarketType::Spot => MarketType::Spot,
            };
            if let Err(e) = engine.subscribe(&bot.exchange, &bot.symbol, mt).await {
                warn!(bot_id = %bot_id, exchange = %bot.exchange, symbol = %bot.symbol, error = %e, "Failed to subscribe KlineEngine for auto bot");
            }
        }

        let handle = tokio::spawn(async move {
            let mut worker = AutoWorker::new(
                bot, price_provider, order_executor, ai_service, store,
                market_data_provider, event_rx, auto_event_tx,
            );
            worker.run(shutdown_rx).await;
        });

        self.workers.insert(bot_id, handle);
        self.shutdown_txs.insert(bot_id, shutdown_tx);
        self.bot_symbols.insert(bot_id, bot_symbol);

        let _ = self.store.update_bot_status(bot_id, "running").await;
        let _ = self.auto_event_tx.send(AutoEvent::BotStarted { bot_id });
        info!(bot_id = %bot_id, "Auto bot started");
    }

    pub(crate) async fn stop_bot(&mut self, bot_id: Uuid, reason: &str) {
        self.stop_or_pause_bot(bot_id, reason, "stopped").await;
    }

    pub(crate) async fn pause_bot(&mut self, bot_id: Uuid) {
        self.stop_or_pause_bot(bot_id, "paused", "paused").await;
    }

    async fn stop_or_pause_bot(&mut self, bot_id: Uuid, reason: &str, target_status: &str) {
        let cancel_symbol = self.bot_symbols.get(&bot_id).cloned();

        let _ = self.order_executor.send_command(OrderCommand::CancelAllOrders {
            symbol: cancel_symbol,
        }).await;

        self.graceful_shutdown_worker(bot_id).await;

        let _ = self.store.update_bot_status(bot_id, target_status).await;
        let _ = self.auto_event_tx.send(AutoEvent::BotStopped { bot_id, reason: reason.to_string() });
        info!(bot_id = %bot_id, "Auto bot {}: {}", target_status, reason);
    }

    async fn graceful_shutdown_worker(&mut self, bot_id: Uuid) {
        if let Some(tx) = self.shutdown_txs.remove(&bot_id) {
            let _ = tx.send(()).await;
        }
        self.bot_symbols.remove(&bot_id);
        if let Some(handle) = self.workers.remove(&bot_id) {
            let abort_handle = handle.abort_handle();
            match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                Ok(Ok(())) => {
                    info!(bot_id = %bot_id, "Auto worker exited gracefully");
                }
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

    pub(crate) async fn resume_bot(&mut self, bot_id: Uuid) {
        let _ = self.store.update_bot_status(bot_id, "stopped").await;
        self.start_bot(bot_id).await;
    }

    pub(crate) async fn delete_bot(&mut self, bot_id: Uuid, close_position: bool) {
        let bot_info = self.store.load_bot(bot_id).await.ok().flatten();
        let symbol = bot_info.as_ref().map(|b| b.symbol.clone());
        let exchange = bot_info.as_ref().map(|b| b.exchange.clone());

        if close_position {
            if let (Some(ref sym), Some(ref ex)) = (&symbol, &exchange) {
                let _ = self.order_executor.send_command(OrderCommand::CancelAllOrders {
                    symbol: Some(sym.clone()),
                }).await;
                let _ = self.order_executor.send_command(OrderCommand::CloseAllPositions {
                    symbol: sym.clone(),
                    exchange: ex.clone(),
                }).await;
            }
        }

        self.stop_or_pause_bot(bot_id, "deleted", "stopped").await;
        let _ = self.store.delete_bot(bot_id).await;
        info!(bot_id = %bot_id, close_position, "Auto bot deleted");
    }

    pub(crate) async fn shutdown_all(&mut self) {
        let bot_ids: Vec<Uuid> = self.workers.keys().copied().collect();
        for id in bot_ids {
            self.stop_bot(id, "engine shutdown").await;
        }
    }
}
