//! GRID_UPDATE — 网格更新推送 (Deprecated)
//!
//! 官方描述: GRID_UPDATE 在网格子订单有部份或是完全成交时更新。
//! 注意: 此事件已被官方标记为 Deprecated，仅记录日志。
//!
//! 官方文档: https://developers.binance.com/zh-CN/docs/products/derivatives-trading-usds-futures/user-data-streams

use serde::Deserialize;
use virs_types::WsFeedEvent;

/// GRID_UPDATE 事件
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct GridUpdateEvent {
    /// 事件类型
    #[serde(rename = "e")]
    pub event_type: String,
    /// 撮合时间
    #[serde(rename = "T")]
    pub transaction_time: i64,
    /// 事件时间
    #[serde(rename = "E")]
    pub event_time: i64,
    /// 网格更新数据
    #[serde(rename = "gu")]
    pub grid_update: GridUpdate,
}

/// 网格更新数据 (gu 字段)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct GridUpdate {
    /// 策略 ID
    #[serde(rename = "si")]
    pub strategy_id: i64,
    /// 策略类型: GRID
    #[serde(rename = "st")]
    pub strategy_type: String,
    /// 策略状态: NEW / WORKING / CANCELLED / EXPIRED
    #[serde(rename = "ss")]
    pub strategy_status: String,
    /// 订单符号
    #[serde(rename = "s")]
    pub symbol: String,
    /// 已实现盈亏 Realized PNL
    #[serde(rename = "r")]
    pub realized_pnl: String,
    /// 未匹配均价 Unmatched Average Price
    #[serde(rename = "up")]
    pub unmatched_avg_price: String,
    /// 未匹配数量 Unmatched Qty
    #[serde(rename = "uq")]
    pub unmatched_qty: String,
    /// 未匹配手续费 Unmatched Fee
    #[serde(rename = "uf")]
    pub unmatched_fee: String,
    /// 已匹配盈亏 Matched PNL
    #[serde(rename = "mp")]
    pub matched_pnl: String,
    /// 更新时间
    #[serde(rename = "ut")]
    pub update_time: i64,
}

/// 处理 GRID_UPDATE 原始 JSON
///
/// 已废弃事件，仅记录 trace 日志。
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
