pub mod account_config_update;
pub mod account_update;
pub mod algo_update;
pub mod conditional_order_trigger_reject;
pub mod listen_key_expired;
pub mod margin_call;
pub mod order_trade_update;
pub mod strategy_update;
pub mod trade_lite;

use virs_type::WsFeedEvent;


pub fn dispatch_event(text: &str) -> Option<WsFeedEvent> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;


    let payload = value.get("data").unwrap_or(&value);
    let event_type = payload
        .get("e")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let payload_str = payload.to_string();


    match event_type {
        "ORDER_TRADE_UPDATE" => order_trade_update::process(&payload_str),
        "ACCOUNT_UPDATE" => account_update::process(&payload_str),
        "MARGIN_CALL" => margin_call::process(&payload_str),
        "ACCOUNT_CONFIG_UPDATE" => account_config_update::process(&payload_str),
        "TRADE_LITE" => trade_lite::process(&payload_str),
        "CONDITIONAL_ORDER_TRIGGER_REJECT" => {
            conditional_order_trigger_reject::process(&payload_str)
        }
        "STRATEGY_UPDATE" => strategy_update::process(&payload_str),
        "ALGO_UPDATE" => algo_update::process(&payload_str),
        "listenKeyExpired" => listen_key_expired::process(&payload_str),
        other => {

            tracing::trace!(event_type = %other, "未知事件类型，忽略");
            None
        }
    }
}
