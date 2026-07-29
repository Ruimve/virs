use serde::Deserialize;
use virs_types::WsFeedEvent;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ConditionalOrderTriggerRejectEvent {
    #[serde(rename = "e")]
    pub event_type: String,

    #[serde(rename = "E")]
    pub event_time: i64,

    #[serde(rename = "T")]
    pub transaction_time: i64,

    #[serde(rename = "or")]
    pub order_reject: OrderReject,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OrderReject {
    #[serde(rename = "s")]
    pub symbol: String,

    #[serde(rename = "i")]
    pub order_id: i64,

    #[serde(rename = "r")]
    pub reject_reason: String,
}

pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: ConditionalOrderTriggerRejectEvent = match serde_json::from_str(json) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                error = %e,
                raw = &json[..json.len().min(200)],
                "解析失败"
            );
            return None;
        }
    };

    tracing::error!(
        symbol = %event.order_reject.symbol,
        order_id = event.order_reject.order_id,
        reason = %event.order_reject.reject_reason,
        "止盈止损单触发后被拒绝 — 条件单未生效，仓位仍处于风险中"
    );

    None
}
