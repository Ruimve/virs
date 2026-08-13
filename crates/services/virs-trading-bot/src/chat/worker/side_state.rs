use uuid::Uuid;
use virs_type::Position;


#[derive(Debug)]
pub(crate) struct PendingOpen {
    pub(crate) side: String,
    pub(crate) entry_price: f64,
    pub(crate) position_size: f64,
    pub(crate) stop_loss: f64,
    pub(crate) take_profit: f64,
    pub(crate) client_order_id: String,
    pub(crate) sent_at: tokio::time::Instant,
}


#[derive(Debug)]
pub(crate) struct PendingClose {
    pub(crate) side: String,
    pub(crate) close_reason: String,
    pub(crate) entry_price: f64,
    pub(crate) position_size: f64,
    pub(crate) client_order_id: String,
    pub(crate) sent_at: tokio::time::Instant,
}


/* SideState：每个方向（多/空）的独立状态机，管理持仓、挂单、止损止盈等。
 * 同一bot可同时持有多和空两个方向的SideState（Hedge模式）。 */
#[derive(Debug, Default)]
pub(crate) struct SideState {

    pub(crate) position: Option<Position>,


    pub(crate) pending_open: Option<PendingOpen>,


    pub(crate) pending_close: Option<PendingClose>,


    pub(crate) stop_loss: f64,


    pub(crate) take_profit: f64,


    pub(crate) position_opened_at: Option<tokio::time::Instant>,


    pub(crate) open_client_order_id: Option<String>,


    pub(crate) log_id: Option<Uuid>,


    pub(crate) open_fee: f64,


    pub(crate) last_close_event: Option<(String, String, chrono::DateTime<chrono::Utc>)>,
}

impl SideState {

    pub(crate) fn has_position(&self) -> bool {
        self.position
            .as_ref()
            .is_some_and(|p| p.is_open() && p.quantity.abs() > 1e-8)
    }


    pub(crate) fn is_pending(&self) -> bool {
        self.pending_open.is_some() || self.pending_close.is_some()
    }


    pub(crate) fn get_position(&self) -> Option<&Position> {
        self.position.as_ref()
    }


    pub(crate) fn clear_position(&mut self) {
        self.position = None;
        self.stop_loss = 0.0;
        self.take_profit = 0.0;
        self.position_opened_at = None;
        self.open_client_order_id = None;
        self.open_fee = 0.0;
    }


    /* 平仓后清理状态并记录平仓事件，用于后续冷却期计算 */
    pub(crate) fn clear_on_close(&mut self, close_event: (String, String, chrono::DateTime<chrono::Utc>)) {
        self.clear_position();
        self.log_id = None;
        self.last_close_event = Some(close_event);
    }
}
