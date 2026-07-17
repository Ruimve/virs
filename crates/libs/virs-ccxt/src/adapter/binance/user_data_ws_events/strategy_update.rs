use serde::Deserialize;
use virs_types::WsFeedEvent;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct StrategyUpdateEvent {
    #[serde(rename = "e")]
    pub event_type: String,

    #[serde(rename = "T")]
    pub transaction_time: i64,

    #[serde(rename = "E")]
    pub event_time: i64,

    #[serde(rename = "su")]
    pub strategy_update: StrategyUpdate,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct StrategyUpdate {
    #[serde(rename = "si")]
    pub strategy_id: i64,

    #[serde(rename = "st")]
    pub strategy_type: String,

    #[serde(rename = "ss")]
    pub strategy_status: String,

    #[serde(rename = "s")]
    pub symbol: String,

    #[serde(rename = "ut")]
    pub update_time: i64,

    #[serde(rename = "c")]
    pub op_code: i32,
}

pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: StrategyUpdateEvent = match serde_json::from_str(json) {
        Ok(e) => e,
        Err(e) => {
            tracing::trace!(
                error = %e,
                "[STRATEGY_UPDATE] 解析失败: {}",
                &json[..json.len().min(200)]
            );
            return None;
        }
    };

    tracing::trace!(
        strategy_id = event.strategy_update.strategy_id,
        strategy_type = %event.strategy_update.strategy_type,
        status = %event.strategy_update.strategy_status,
        symbol = %event.strategy_update.symbol,
        op_code = event.strategy_update.op_code,
        "STRATEGY_UPDATE — 币安原生策略更新（仅日志）"
    );

    None
}
