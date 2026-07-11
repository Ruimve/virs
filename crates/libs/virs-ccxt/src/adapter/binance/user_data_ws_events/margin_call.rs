//! MARGIN_CALL — 追加保证金通知
//!
//! 官方描述: 当用户持仓风险过高，会推送此消息。
//! 此消息仅作为风险指导信息，不建议用于投资策略。
//! 在大波动市场行情下，不排除此消息发出的同时用户仓位已被强平的可能。
//!
//! 官方文档: https://developers.binance.com/zh-CN/docs/products/derivatives-trading-usds-futures/user-data-streams

use serde::Deserialize;
use virs_types::WsFeedEvent;

/// MARGIN_CALL 事件
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct MarginCallEvent {
    /// 事件类型
    #[serde(rename = "e")]
    pub event_type: String,
    /// 事件时间
    #[serde(rename = "E")]
    pub event_time: i64,
    /// 全仓钱包余额（仅全仓持仓 margin call 时推送）
    #[serde(rename = "cw")]
    pub cross_wallet_balance: String,
    /// 追加保证金的持仓列表
    #[serde(rename = "p")]
    pub positions: Vec<MarginCallPosition>,
}

/// Margin Call 持仓信息 (p 数组元素)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct MarginCallPosition {
    /// 订单符号 Symbol
    #[serde(rename = "s")]
    pub symbol: String,
    /// 持仓方向 Position Side: LONG / SHORT / BOTH
    #[serde(rename = "ps")]
    pub position_side: String,
    /// 持仓数量 Position Amount
    #[serde(rename = "pa")]
    pub position_amount: String,
    /// 保证金类型 Margin Type: CROSSED / ISOLATED
    #[serde(rename = "mt")]
    pub margin_type: String,
    /// 逐仓钱包余额 Isolated Wallet (if isolated position)
    #[serde(rename = "iw")]
    pub isolated_wallet: String,
    /// 标记价格 Mark Price
    #[serde(rename = "mp")]
    pub mark_price: String,
    /// 未实现盈亏 Unrealized PnL
    #[serde(rename = "up")]
    pub unrealized_pnl: String,
    /// 维持保证金 Maintenance Margin Required
    #[serde(rename = "mm")]
    pub maintenance_margin: String,
}

/// 处理 MARGIN_CALL 原始 JSON
///
/// 当前阶段: 解析并记录 error 日志（风控关键事件），返回 None。
/// 后续阶段: 返回 WsFeedEvent::MarginCall
pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: MarginCallEvent = match serde_json::from_str(json) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "[MARGIN_CALL] 解析失败: {}",
                &json[..json.len().min(200)]
            );
            return None;
        }
    };

    for p in &event.positions {
        tracing::error!(
            symbol = %p.symbol,
            position_side = %p.position_side,
            position_amount = %p.position_amount,
            margin_type = %p.margin_type,
            mark_price = %p.mark_price,
            unrealized_pnl = %p.unrealized_pnl,
            maintenance_margin = %p.maintenance_margin,
            cross_wallet_balance = %event.cross_wallet_balance,
            "追加保证金通知 — 持仓风险过高，可能面临强平"
        );
    }

    None
}
