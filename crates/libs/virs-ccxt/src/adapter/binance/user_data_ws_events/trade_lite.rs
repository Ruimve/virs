//! TRADE_LITE — 精简交易推送
//!
//! 官方描述: 精简交易推送相比原有的 ORDER_TRADE_UPDATE 流减少了数据延迟，
//! 但该交易推送仅推送和交易相关的字段。
//! 注意: 仅推送 TRADE 执行类型，不含 NEW/CANCELED 等。
//!
//! 官方文档: https://developers.binance.com/zh-CN/docs/products/derivatives-trading-usds-futures/user-data-streams

use serde::Deserialize;
use virs_types::WsFeedEvent;

/// TRADE_LITE 事件
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct TradeLiteEvent {
    /// 事件类型
    #[serde(rename = "e")]
    pub event_type: String,
    /// 事件时间
    #[serde(rename = "E")]
    pub event_time: i64,
    /// 撮合时间
    #[serde(rename = "T")]
    pub transaction_time: i64,
    /// 订单符号 Symbol
    #[serde(rename = "s")]
    pub symbol: String,
    /// 原始订单数量 Original Quantity
    #[serde(rename = "q")]
    pub orig_qty: String,
    /// 原始价格 Original Price
    #[serde(rename = "p")]
    pub original_price: String,
    /// 是否为 maker Is this trade the maker side?
    #[serde(rename = "m")]
    pub is_maker: bool,
    /// 客户端订单 ID Client Order Id
    #[serde(rename = "c")]
    pub client_order_id: String,
    /// 订单方向 Side: BUY / SELL
    #[serde(rename = "S")]
    pub side: String,
    /// 本次成交价 Last Filled Price (本笔维度)
    #[serde(rename = "L")]
    pub last_fill_price: String,
    /// 本次成交数量 Order Last Filled Quantity (本笔维度)
    #[serde(rename = "l")]
    pub last_fill_qty: String,
    /// 成交 ID Trade Id
    #[serde(rename = "t")]
    pub trade_id: i64,
    /// 订单 ID Order Id
    #[serde(rename = "i")]
    pub order_id: i64,
}

/// 处理 TRADE_LITE 原始 JSON
///
/// 当前阶段: 解析并记录 trace 日志，返回 None。
/// TRADE_LITE 是可选的低延迟流，如果同时订阅了 ORDER_TRADE_UPDATE 则仅作参考。
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
