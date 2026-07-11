//! STRATEGY_UPDATE — 策略交易更新推送
//!
//! 官方描述: STRATEGY_UPDATE 在策略交易创建、取消、失效等等时候更新。
//! 注意: 本项目自行实现网格策略（virs-bot/grid），不依赖币安原生策略交易。
//! 此事件仅作日志记录，不参与业务逻辑。
//!
//! 官方文档: https://developers.binance.com/zh-CN/docs/products/derivatives-trading-usds-futures/user-data-streams

use serde::Deserialize;
use virs_types::WsFeedEvent;

/// STRATEGY_UPDATE 事件
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct StrategyUpdateEvent {
    /// 事件类型
    #[serde(rename = "e")]
    pub event_type: String,
    /// 撮合时间
    #[serde(rename = "T")]
    pub transaction_time: i64,
    /// 事件时间
    #[serde(rename = "E")]
    pub event_time: i64,
    /// 策略更新数据
    #[serde(rename = "su")]
    pub strategy_update: StrategyUpdate,
}

/// 策略更新数据 (su 字段)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct StrategyUpdate {
    /// 策略 ID
    #[serde(rename = "si")]
    pub strategy_id: i64,
    /// 策略类型: GRID 等
    #[serde(rename = "st")]
    pub strategy_type: String,
    /// 策略状态: NEW / WORKING / CANCELLED / EXPIRED
    #[serde(rename = "ss")]
    pub strategy_status: String,
    /// 订单符号
    #[serde(rename = "s")]
    pub symbol: String,
    /// 更新时间
    #[serde(rename = "ut")]
    pub update_time: i64,
    /// opCode (8001-8015)
    #[serde(rename = "c")]
    pub op_code: i32,
}

/// 处理 STRATEGY_UPDATE 原始 JSON
///
/// 仅日志记录，不生成 WsFeedEvent（本项目不使用币安原生策略交易）。
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
