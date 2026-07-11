//! CONDITIONAL_ORDER_TRIGGER_REJECT — 条件订单触发后拒绝更新推送
//!
//! 官方描述: CONDITIONAL_ORDER_TRIGGER_REJECT 在止盈止损单触发后被拒绝时推送
//!
//! 官方文档: https://developers.binance.com/zh-CN/docs/products/derivatives-trading-usds-futures/user-data-streams

use serde::Deserialize;
use virs_types::WsFeedEvent;

/// CONDITIONAL_ORDER_TRIGGER_REJECT 事件
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ConditionalOrderTriggerRejectEvent {
    /// 事件类型
    #[serde(rename = "e")]
    pub event_type: String,
    /// 事件时间
    #[serde(rename = "E")]
    pub event_time: i64,
    /// 消息发送时间
    #[serde(rename = "T")]
    pub transaction_time: i64,
    /// 订单拒绝信息
    #[serde(rename = "or")]
    pub order_reject: OrderReject,
}

/// 订单拒绝信息 (or 字段)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OrderReject {
    /// 订单符号 Symbol
    #[serde(rename = "s")]
    pub symbol: String,
    /// 订单 ID Order Id
    #[serde(rename = "i")]
    pub order_id: i64,
    /// 拒绝原因 Reject Reason
    #[serde(rename = "r")]
    pub reject_reason: String,
}

/// 处理 CONDITIONAL_ORDER_TRIGGER_REJECT 原始 JSON
///
/// 当前阶段: 解析并记录 error 日志，返回 None。
/// 后续阶段: 返回 WsFeedEvent::ConditionalOrderTriggerReject
pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: ConditionalOrderTriggerRejectEvent = match serde_json::from_str(json) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "[CONDITIONAL_ORDER_TRIGGER_REJECT] 解析失败: {}",
                &json[..json.len().min(200)]
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
