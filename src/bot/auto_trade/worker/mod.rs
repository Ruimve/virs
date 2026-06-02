use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tracing::warn;

use crate::bot::auto_trade::ai::AutoAiService;
use crate::bot::auto_trade::ports::*;
use crate::bot::auto_trade::types::{AutoBotConfig, AutoEvent, MarketType};

const PENDING_ORDER_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub(crate) struct PendingOpen {
    pub side: String,
    pub entry_price: f64,
    pub position_size: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub client_order_id: String,
    pub sent_at: tokio::time::Instant,
}

#[derive(Debug)]
pub(crate) struct PendingClose {
    pub side: String,
    pub reason: String,
    pub entry_price: f64,
    pub position_size: f64,
    pub unrealized_pnl: f64,
    pub client_order_id: String,
    pub sent_at: tokio::time::Instant,
}

pub struct AutoWorker {
    pub(crate) bot: AutoBotConfig,
    price_provider: Arc<dyn PriceProvider>,
    order_executor: Arc<dyn OrderExecutor>,
    ai_service: Arc<AutoAiService>,
    store: Arc<dyn AutoStore>,
    market_data_provider: Arc<dyn MarketDataProvider>,
    event_rx: broadcast::Receiver<OrderEvent>,
    auto_event_tx: broadcast::Sender<AutoEvent>,
    pub(crate) current_price: f64,
    pub(crate) consecutive_losses: i32,
    pub(crate) paused: bool,
    pub(crate) pending_open: Option<PendingOpen>,
    pub(crate) pending_close: Option<PendingClose>,
    pub(crate) position_opened_at: Option<tokio::time::Instant>,
    pub(crate) trailing_stop_dirty: bool,
}

impl AutoWorker {
    pub fn new(
        bot: AutoBotConfig,
        price_provider: Arc<dyn PriceProvider>,
        order_executor: Arc<dyn OrderExecutor>,
        ai_service: Arc<AutoAiService>,
        store: Arc<dyn AutoStore>,
        market_data_provider: Arc<dyn MarketDataProvider>,
        event_rx: broadcast::Receiver<OrderEvent>,
        auto_event_tx: broadcast::Sender<AutoEvent>,
    ) -> Self {
        Self {
            bot,
            price_provider,
            order_executor,
            ai_service,
            store,
            market_data_provider,
            event_rx,
            auto_event_tx,
            current_price: 0.0,
            consecutive_losses: 0,
            paused: false,
            pending_open: None,
            pending_close: None,
            position_opened_at: None,
            trailing_stop_dirty: false,
        }
    }

    pub(crate) fn is_spot(&self) -> bool {
        matches!(self.bot.market_type, MarketType::Spot)
    }

    pub(crate) fn has_position(&self) -> bool {
        self.bot.current_side.as_ref().map_or(false, |s| !s.is_empty() && s != "none")
    }

    pub(crate) fn is_long(&self) -> bool {
        self.bot.current_side.as_deref() == Some("long")
    }

    pub(crate) fn is_short(&self) -> bool {
        self.bot.current_side.as_deref() == Some("short")
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.pending_open.is_some() || self.pending_close.is_some()
    }

    pub(crate) fn normalize_side(&self) -> Option<&str> {
        match self.bot.current_side.as_deref() {
            Some(s) if !s.is_empty() && s != "none" => Some(s),
            _ => None,
        }
    }

    pub(crate) async fn fetch_current_price(&self) -> f64 {
        match self.price_provider.get_price(&self.bot.exchange, &self.bot.symbol, self.bot.market_type.as_str()).await {
            Some(price) if price > 0.0 => price,
            _ => self.current_price,
        }
    }

    pub(crate) async fn save_position(&self) {
        let side_str = self.bot.current_side.as_deref().unwrap_or("none");
        let _ = self.store.update_position(
            self.bot.id,
            Some(side_str),
            self.bot.entry_price,
            self.bot.position_size,
            self.bot.stop_loss,
            self.bot.take_profit,
            self.bot.unrealized_pnl,
        ).await;
    }

    pub(crate) async fn save_stats(&self) {
        let _ = self.store.update_stats(
            self.bot.id,
            self.bot.total_pnl,
            self.bot.total_trades,
            self.bot.win_trades,
            self.bot.loss_trades,
        ).await;
    }

    pub(crate) fn check_pending_timeout(&mut self) {
        let now = tokio::time::Instant::now();
        if let Some(ref pending) = self.pending_open {
            if now.duration_since(pending.sent_at) > PENDING_ORDER_TIMEOUT {
                warn!(bot_id = %self.bot.id, "Pending open order timed out, clearing");
                self.pending_open = None;
            }
        }
        if let Some(ref pending) = self.pending_close {
            if now.duration_since(pending.sent_at) > PENDING_ORDER_TIMEOUT {
                warn!(bot_id = %self.bot.id, "Pending close order timed out, clearing");
                self.pending_close = None;
            }
        }
    }

    pub(crate) fn matches_pending_order(&self, client_order_id: Option<&str>) -> bool {
        let bot_id_str = self.bot.id.to_string();
        match client_order_id {
            Some(cid) => cid.contains(&bot_id_str),
            None => false,
        }
    }
}

mod decide;
mod state;
