use serde::Deserialize;
use virs_types::WsFeedEvent;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct GridUpdateEvent {
    #[serde(rename = "e")]
    pub event_type: String,

    #[serde(rename = "T")]
    pub transaction_time: i64,

    #[serde(rename = "E")]
    pub event_time: i64,

    #[serde(rename = "gu")]
    pub grid_update: GridUpdate,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct GridUpdate {
    #[serde(rename = "si")]
    pub strategy_id: i64,

    #[serde(rename = "st")]
    pub strategy_type: String,

    #[serde(rename = "ss")]
    pub strategy_status: String,

    #[serde(rename = "s")]
    pub symbol: String,

    #[serde(rename = "r")]
    pub realized_pnl: String,

    #[serde(rename = "up")]
    pub unmatched_avg_price: String,

    #[serde(rename = "uq")]
    pub unmatched_qty: String,

    #[serde(rename = "uf")]
    pub unmatched_fee: String,

    #[serde(rename = "mp")]
    pub matched_pnl: String,

    #[serde(rename = "ut")]
    pub update_time: i64,
}

pub fn process(json: &str) -> Option<WsFeedEvent> {
    let _event: GridUpdateEvent = match serde_json::from_str(json) {
        Ok(e) => e,
        Err(e) => {
            tracing::trace!(
                error = %e,
                "[GRID_UPDATE] 解析失败: {}",
                &json[..json.len().min(200)]
            );
            return None;
        }
    };

    tracing::trace!("GRID_UPDATE — 已废弃事件，忽略");

    None
}
