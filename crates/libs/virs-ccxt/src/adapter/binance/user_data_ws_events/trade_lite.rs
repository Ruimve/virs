use serde::Deserialize;
use virs_types::WsFeedEvent;


#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct TradeLiteEvent {

    #[serde(rename = "e")]
    pub event_type: String,

    #[serde(rename = "E")]
    pub event_time: i64,

    #[serde(rename = "T")]
    pub transaction_time: i64,

    #[serde(rename = "s")]
    pub symbol: String,

    #[serde(rename = "q")]
    pub orig_qty: String,

    #[serde(rename = "p")]
    pub original_price: String,

    #[serde(rename = "m")]
    pub is_maker: bool,

    #[serde(rename = "c")]
    pub client_order_id: String,

    #[serde(rename = "S")]
    pub side: String,

    #[serde(rename = "L")]
    pub last_fill_price: String,

    #[serde(rename = "l")]
    pub last_fill_qty: String,

    #[serde(rename = "t")]
    pub trade_id: i64,

    #[serde(rename = "i")]
    pub order_id: i64,
}


pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: TradeLiteEvent = match serde_json::from_str(json) {
        Ok(e) => e,
        Err(e) => {
            tracing::trace!(
                error = %e,
                "[TRADE_LITE] 解析失败: {}",
                &json[..json.len().min(200)]
            );
            return None;
        }
    };

    tracing::trace!(
        symbol = %event.symbol,
        client_order_id = %event.client_order_id,
        order_id = event.order_id,
        trade_id = event.trade_id,
        last_fill_price = %event.last_fill_price,
        last_fill_qty = %event.last_fill_qty,
        "TRADE_LITE — 低延迟成交推送（仅日志）"
    );

    None
}
