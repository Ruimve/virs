//! listenKeyExpired — listenKey 过期推送
//!
//! 官方描述: 当前连接使用的有效 listenKey 过期时，user data stream 将会推送此事件。
//! 注意:
//! - 此事件与 websocket 连接中断没有必然联系
//! - 只有正在连接中的有效 listenKey 过期时才会收到此消息
//! - 收到此消息后 user data stream 将不再更新，直到用户使用新的有效的 listenKey
//!
//! 官方文档: https://developers.binance.com/zh-CN/docs/products/derivatives-trading-usds-futures/user-data-streams

use serde::Deserialize;
use virs_types::WsFeedEvent;

/// listenKeyExpired 事件
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ListenKeyExpiredEvent {
    /// 事件类型
    #[serde(rename = "e")]
    pub event_type: String,
    /// 事件时间
    #[serde(rename = "E")]
    pub event_time: i64,
    /// 已过期的 listenKey
    #[serde(rename = "listenKey")]
    pub listen_key: String,
}

/// listenKey 过期时的处理结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenKeyExpiredAction {
    /// 需要重新创建 listenKey 并重连
    NeedRecreate,
    /// 仅记录日志
    LogOnly,
}

/// 处理 listenKeyExpired 原始 JSON
///
/// 当前阶段: 解析并记录 error 日志，返回 None。
/// 调用方（user_data_ws.rs）需根据返回值决定是否 break 并重建 listenKey。
///
/// 后续阶段: 返回 WsFeedEvent::ListenKeyExpired，由引擎层处理重建逻辑。
pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: ListenKeyExpiredEvent = match serde_json::from_str(json) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "[listenKeyExpired] 解析失败: {}",
                &json[..json.len().min(200)]
            );
            return None;
        }
    };

    tracing::error!(
        expired_key = %event.listen_key,
        event_time = event.event_time,
        "listenKey 已过期 — 需要重新创建 listenKey 后重连，当前重连使用旧 key 将失败"
    );

    // 注意: 当前返回 None，调用方通过 event_type 匹配来处理 break 逻辑。
    // 后续 WsFeedEvent 扩展后，返回 WsFeedEvent::ListenKeyExpired。
    // 调用方需在收到此事件后:
    // 1. 重新调用 POST /fapi/v1/listenKey 创建新 listenKey
    // 2. 用新 listenKey 重建 WS URL
    // 3. 重新连接
    None
}
