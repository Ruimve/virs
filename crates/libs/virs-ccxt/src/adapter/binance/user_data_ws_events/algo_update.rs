use serde::Deserialize;
use virs_types::WsFeedEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlgoStatus {
    New,

    Canceled,

    Triggering,

    Triggered,

    Finished,

    Rejected,

    Expired,

    Unknown(String),
}

impl AlgoStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "NEW" => Self::New,
            "CANCELED" => Self::Canceled,
            "TRIGGERING" => Self::Triggering,
            "TRIGGERED" => Self::Triggered,
            "FINISHED" => Self::Finished,
            "REJECTED" => Self::Rejected,
            "EXPIRED" => Self::Expired,
            other => Self::Unknown(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AlgoUpdateEvent {
    #[serde(rename = "e")]
    pub event_type: String,

    #[serde(rename = "T")]
    pub transaction_time: i64,

    #[serde(rename = "E")]
    pub event_time: i64,

    #[serde(rename = "o")]
    pub algo_order: AlgoOrder,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AlgoOrder {
    #[serde(rename = "caid")]
    pub client_algo_id: String,

    #[serde(rename = "aid")]
    pub algo_id: i64,

    #[serde(rename = "at")]
    pub algo_type: String,

    #[serde(rename = "o")]
    pub order_type: String,

    #[serde(rename = "s")]
    pub symbol: String,

    #[serde(rename = "S")]
    pub side: String,

    #[serde(rename = "ps")]
    pub position_side: String,

    #[serde(rename = "f")]
    pub time_in_force: String,

    #[serde(rename = "q")]
    pub quantity: String,

    #[serde(rename = "X")]
    pub algo_status: String,

    #[serde(rename = "ai")]
    pub order_id: String,

    #[serde(rename = "ap")]
    pub avg_fill_price: String,

    #[serde(rename = "aq")]
    pub executed_qty: String,

    #[serde(rename = "act")]
    pub actual_order_type: String,

    #[serde(rename = "tp")]
    pub trigger_price: String,

    #[serde(rename = "p")]
    pub order_price: String,

    #[serde(rename = "V")]
    pub stp_mode: String,

    #[serde(rename = "wt")]
    pub working_type: String,

    #[serde(rename = "pm")]
    pub price_match_mode: String,

    #[serde(rename = "cp")]
    pub is_close_all: bool,

    #[serde(rename = "pP")]
    pub price_protection: bool,

    #[serde(rename = "R")]
    pub reduce_only: bool,

    #[serde(rename = "tt")]
    pub trigger_time: i64,

    #[serde(rename = "gtd")]
    pub gtd_auto_cancel_time: i64,

    #[serde(rename = "rm")]
    pub failed_reason: String,
}

pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: AlgoUpdateEvent = match serde_json::from_str(json) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "[ALGO_UPDATE] 解析失败: {}",
                &json[..json.len().min(200)]
            );
            return None;
        }
    };

    let status = AlgoStatus::from_str(&event.algo_order.algo_status);
    let symbol = &event.algo_order.symbol;

    match status {
        AlgoStatus::Triggered => {
            tracing::info!(
                symbol = %symbol,
                algo_id = event.algo_order.algo_id,
                order_type = %event.algo_order.order_type,
                "条件订单已触发 — 进入撮合引擎"
            );
        }
        AlgoStatus::Rejected | AlgoStatus::Expired => {
            tracing::error!(
                symbol = %symbol,
                status = ?status,
                reason = %event.algo_order.failed_reason,
                algo_id = event.algo_order.algo_id,
                "条件订单被拒绝或过期 — 止盈/止损未生效，仓位仍处于风险中"
            );
        }
        AlgoStatus::Finished => {
            tracing::info!(
                symbol = %symbol,
                algo_id = event.algo_order.algo_id,
                "条件订单已完成"
            );
        }
        _ => {
            tracing::debug!(
                symbol = %symbol,
                status = ?status,
                algo_id = event.algo_order.algo_id,
                "条件订单状态更新"
            );
        }
    }

    None
}
