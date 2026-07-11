//! ALGO_UPDATE — 条件订单交易更新推送
//!
//! 官方描述: 当有新订单创建、订单有新成交或者新的状态变化时会推送此类事件。
//! 追踪条件订单（止盈/止损）的完整生命周期:
//!   NEW → TRIGGERING → TRIGGERED → FINISHED
//!   或: NEW → CANCELED / REJECTED / EXPIRED
//!
//! 官方文档: https://developers.binance.com/zh-CN/docs/products/derivatives-trading-usds-futures/user-data-streams

use serde::Deserialize;
use virs_types::WsFeedEvent;

/// Algo 状态枚举
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlgoStatus {
    /// 条件订单已提交，但尚未触发
    New,
    /// 条件订单已被取消
    Canceled,
    /// 已满足触发条件，已转发至撮合引擎
    Triggering,
    /// 已成功触发并进入撮合引擎
    Triggered,
    /// 触发的条件订单已在撮合引擎中被成交或取消
    Finished,
    /// 条件订单被撮合引擎拒绝（如保证金检查失败）
    Rejected,
    /// 条件订单被系统取消（如 GTE_GTC 条件不再满足）
    Expired,
    /// 未知状态
    Unknown(String),
}

impl AlgoStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "NEW" => Self::New,
            "CANCELED" => Self::Canceled,
            "TRIGGERING" => Self::Triggering,
            "TRIGGERED" => Self::Triggered,
            "FINISHED" => Self::Finished,
            "REJECTED" => Self::Rejected,
            "EXPIRED" => Self::Expired,
            other => Self::Unknown(other.to_string()),
        }
    }
}

/// ALGO_UPDATE 事件
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AlgoUpdateEvent {
    /// 事件类型
    #[serde(rename = "e")]
    pub event_type: String,
    /// 撮合时间
    #[serde(rename = "T")]
    pub transaction_time: i64,
    /// 事件时间
    #[serde(rename = "E")]
    pub event_time: i64,
    /// Algo 订单数据
    #[serde(rename = "o")]
    pub algo_order: AlgoOrder,
}

/// Algo 订单数据 (o 字段)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AlgoOrder {
    /// 客户端 Algo ID Client Algo Id
    #[serde(rename = "caid")]
    pub client_algo_id: String,
    /// Algo ID
    #[serde(rename = "aid")]
    pub algo_id: i64,
    /// Algo 类型: CONDITIONAL 等
    #[serde(rename = "at")]
    pub algo_type: String,
    /// 订单类型: TAKE_PROFIT / STOP 等
    #[serde(rename = "o")]
    pub order_type: String,
    /// 订单符号
    #[serde(rename = "s")]
    pub symbol: String,
    /// 订单方向: BUY / SELL
    #[serde(rename = "S")]
    pub side: String,
    /// 持仓方向: LONG / SHORT / BOTH
    #[serde(rename = "ps")]
    pub position_side: String,
    /// 有效方式: GTC 等
    #[serde(rename = "f")]
    pub time_in_force: String,
    /// 数量
    #[serde(rename = "q")]
    pub quantity: String,
    /// Algo 状态
    #[serde(rename = "X")]
    pub algo_status: String,
    /// 订单 ID（触发后才有）
    #[serde(rename = "ai")]
    pub order_id: String,
    /// 撮合引擎累计成交均价（触发后才有）
    #[serde(rename = "ap")]
    pub avg_fill_price: String,
    /// 撮合引擎累计成交量（触发后才有）
    #[serde(rename = "aq")]
    pub executed_qty: String,
    /// 撮合引擎中实际订单类型（触发后才有）
    #[serde(rename = "act")]
    pub actual_order_type: String,
    /// 触发价格
    #[serde(rename = "tp")]
    pub trigger_price: String,
    /// 订单价格
    #[serde(rename = "p")]
    pub order_price: String,
    /// STP 模式
    #[serde(rename = "V")]
    pub stp_mode: String,
    /// 工作类型
    #[serde(rename = "wt")]
    pub working_type: String,
    /// 价格匹配模式
    #[serde(rename = "pm")]
    pub price_match_mode: String,
    /// 是否全平
    #[serde(rename = "cp")]
    pub is_close_all: bool,
    /// 价格保护
    #[serde(rename = "pP")]
    pub price_protection: bool,
    /// 是否只减仓
    #[serde(rename = "R")]
    pub is_reduce_only: bool,
    /// 触发时间
    #[serde(rename = "tt")]
    pub trigger_time: i64,
    /// GTD 自动取消时间
    #[serde(rename = "gtd")]
    pub gtd_auto_cancel_time: i64,
    /// Algo 订单失败原因
    #[serde(rename = "rm")]
    pub failed_reason: String,
}

/// 处理 ALGO_UPDATE 原始 JSON
///
/// 当前阶段: 解析并按状态记录日志，返回 None。
/// 后续阶段: 返回 WsFeedEvent::AlgoUpdate
pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: AlgoUpdateEvent = match serde_json::from_str(json) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "[ALGO_UPDATE] 解析失败: {}",
                &json[..json.len().min(200)]
            );
            return None;
        }
    };

    let status = AlgoStatus::from_str(&event.algo_order.algo_status);
    let symbol = &event.algo_order.symbol;

    match status {
        AlgoStatus::Triggered => {
            tracing::info!(
                symbol = %symbol,
                algo_id = event.algo_order.algo_id,
                order_type = %event.algo_order.order_type,
                "条件订单已触发 — 进入撮合引擎"
            );
        }
        AlgoStatus::Rejected | AlgoStatus::Expired => {
            tracing::error!(
                symbol = %symbol,
                status = ?status,
                reason = %event.algo_order.failed_reason,
                algo_id = event.algo_order.algo_id,
                "条件订单被拒绝或过期 — 止盈/止损未生效，仓位仍处于风险中"
            );
        }
        AlgoStatus::Finished => {
            tracing::info!(
                symbol = %symbol,
                algo_id = event.algo_order.algo_id,
                "条件订单已完成"
            );
        }
        _ => {
            tracing::debug!(
                symbol = %symbol,
                status = ?status,
                algo_id = event.algo_order.algo_id,
                "条件订单状态更新"
            );
        }
    }

    None
}
