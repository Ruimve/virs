use uuid::Uuid;
use virs_types::position::Position;

/// Per-side pending open order state.
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

/// Per-side pending close order state.
#[derive(Debug)]
pub(crate) struct PendingClose {
    pub side: String,
    pub close_reason: String,
    pub entry_price: f64,
    pub position_size: f64,
    pub client_order_id: String,
    pub sent_at: tokio::time::Instant,
}

/// All per-side state for one direction (Long or Short) of the AutoWorker.
///
/// This struct replaces the 20+ per-side fields that were previously
/// duplicated as `xxx_long` / `xxx_short` on `AutoWorker`.
#[derive(Debug, Default)]
pub(crate) struct SideState {
    /// Cached position from PE (None = no position).
    pub position: Option<Position>,

    /// Pending open order awaiting WS confirmation.
    pub pending_open: Option<PendingOpen>,

    /// Pending close order awaiting WS confirmation.
    pub pending_close: Option<PendingClose>,

    /// Current stop-loss price (0.0 = not set).
    pub stop_loss: f64,

    /// Current take-profit price (0.0 = not set).
    pub take_profit: f64,

    /// When the current position was opened.
    pub position_opened_at: Option<tokio::time::Instant>,

    /// Client order ID of the open order (for trade record close matching).
    pub open_client_order_id: Option<String>,

    /// Analysis log ID associated with the current operation.
    pub log_id: Option<Uuid>,

    /// Fee paid on the open order (used for realized PnL calculation on close).
    pub open_fee: f64,

    /// Last close event (side, reason, timestamp) — used for cooldown.
    pub last_close_event: Option<(String, String, chrono::DateTime<chrono::Utc>)>,
}

impl SideState {
    /// Returns true if this side has an open position with non-zero quantity.
    pub fn has_position(&self) -> bool {
        self.position
            .as_ref()
            .is_some_and(|p| p.is_open() && p.quantity.abs() > 1e-8)
    }

    /// Returns true if there is a pending open or close order on this side.
    pub fn is_pending(&self) -> bool {
        self.pending_open.is_some() || self.pending_close.is_some()
    }

    /// Returns the position reference if present.
    pub fn get_position(&self) -> Option<&Position> {
        self.position.as_ref()
    }

    /// Clears position-related fields (keeps log_id and last_close_event).
    ///
    /// Used when a position is closed by an external event and we need
    /// to reset SL/TP/opened_at/oid/fee without touching log or cooldown.
    pub fn clear_position(&mut self) {
        self.position = None;
        self.stop_loss = 0.0;
        self.take_profit = 0.0;
        self.position_opened_at = None;
        self.open_client_order_id = None;
        self.open_fee = 0.0;
    }

    /// Full close cleanup: clears position fields + log_id + sets last_close_event.
    ///
    /// Used when a close is confirmed (apply_pending_close) or an external
    /// close is detected (on_pe_event PositionClosed).
    pub fn clear_on_close(&mut self, close_event: (String, String, chrono::DateTime<chrono::Utc>)) {
        self.clear_position();
        self.log_id = None;
        self.last_close_event = Some(close_event);
    }
}
