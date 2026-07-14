pub mod account_config_update;
pub mod account_update;
pub mod algo_update;
pub mod conditional_order_trigger_reject;
pub mod grid_update;
pub mod listen_key_expired;
pub mod margin_call;
pub mod order_trade_update;
pub mod strategy_update;
pub mod trade_lite;

use virs_types::WsFeedEvent;


// 事件分发器，按e字段路由到各处理器
pub fn dispatch_event(text: &str) -> Option<WsFeedEvent> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;

    // 兼容组合流(取data)和扁平流(直接用value)
    let payload = value.get("data").unwrap_or(&value);
    let event_type = payload
        .get("e")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let payload_str = payload.to_string();

    // 按事件类型分发
    match event_type {
        "ORDER_TRADE_UPDATE" => order_trade_update::process(&payload_str),         // 订单成交更新
        "ACCOUNT_UPDATE" => account_update::process(&payload_str),                // 账户余额更新(仅日志)
        "MARGIN_CALL" => margin_call::process(&payload_str),                      // 保证金不足(仅error日志)
        "ACCOUNT_CONFIG_UPDATE" => account_config_update::process(&payload_str),  // 账户杠杆配置(仅warn日志)
        "TRADE_LITE" => trade_lite::process(&payload_str),                         // 轻量成交(仅trace日志)
        "CONDITIONAL_ORDER_TRIGGER_REJECT" => conditional_order_trigger_reject::process(&payload_str),
        "STRATEGY_UPDATE" => strategy_update::process(&payload_str),
        "GRID_UPDATE" => grid_update::process(&payload_str),
        "ALGO_UPDATE" => algo_update::process(&payload_str),
        "listenKeyExpired" => listen_key_expired::process(&payload_str),
        other => {
            // 其他事件类型仅日志记录
            tracing::trace!(
                event_type = other,
                "[UserDataWs] 未知事件类型，忽略"
            );
            None
        }
    }
}
