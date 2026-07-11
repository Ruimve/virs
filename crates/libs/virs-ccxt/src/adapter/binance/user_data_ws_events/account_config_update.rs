//! ACCOUNT_CONFIG_UPDATE — 杠杆倍数等账户配置更新推送
//!
//! 官方描述: 当账户配置发生变化时会推送此类事件。
//! - 杠杆倍数变化时推送 ac 对象 (s=交易对, l=杠杆)
//! - 联合保证金状态变化时推送 ai 对象 (j=联合保证金状态)
//! - ac 和 ai 不会同时出现
//!
//! 官方文档: https://developers.binance.com/zh-CN/docs/products/derivatives-trading-usds-futures/user-data-streams

use serde::Deserialize;
use virs_types::WsFeedEvent;

/// ACCOUNT_CONFIG_UPDATE 事件
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AccountConfigUpdateEvent {
    /// 事件类型
    #[serde(rename = "e")]
    pub event_type: String,
    /// 事件时间
    #[serde(rename = "E")]
    pub event_time: i64,
    /// 撮合时间
    #[serde(rename = "T")]
    pub transaction_time: i64,
    /// 杠杆倍数变化时存在
    #[serde(rename = "ac")]
    pub account_config: Option<AccountConfig>,
    /// 联合保证金状态变化时存在
    #[serde(rename = "ai")]
    pub account_info: Option<AccountInfo>,
}

/// 交易对账户配置 (ac 字段)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AccountConfig {
    /// 订单符号 Symbol
    #[serde(rename = "s")]
    pub symbol: String,
    /// 杠杆倍数 Leverage
    #[serde(rename = "l")]
    pub leverage: u32,
}

/// 用户账户配置 (ai 字段)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AccountInfo {
    /// 联合保证金状态 Multi-Assets Mode
    #[serde(rename = "j")]
    pub multi_assets_mode: bool,
}

/// 处理 ACCOUNT_CONFIG_UPDATE 原始 JSON
///
/// 当前阶段: 解析并记录日志，返回 None。
/// 后续阶段: 返回 WsFeedEvent::AccountConfigUpdate
pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: AccountConfigUpdateEvent = match serde_json::from_str(json) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "[ACCOUNT_CONFIG_UPDATE] 解析失败: {}",
                &json[..json.len().min(200)]
            );
            return None;
        }
    };

    if let Some(ac) = &event.account_config {
        tracing::warn!(
            symbol = %ac.symbol,
            leverage = ac.leverage,
            "杠杆倍数已变更 — 需同步本地仓位杠杆"
        );
    }

    if let Some(ai) = &event.account_info {
        tracing::info!(
            multi_assets_mode = ai.multi_assets_mode,
            "联合保证金模式变更"
        );
    }

    None
}
