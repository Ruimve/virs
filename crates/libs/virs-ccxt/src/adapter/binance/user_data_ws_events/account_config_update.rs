use serde::Deserialize;
use virs_types::WsFeedEvent;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AccountConfigUpdateEvent {
    #[serde(rename = "e")]
    pub event_type: String,

    #[serde(rename = "E")]
    pub event_time: i64,

    #[serde(rename = "T")]
    pub transaction_time: i64,

    #[serde(rename = "ac")]
    pub account_config: Option<AccountConfig>,

    #[serde(rename = "ai")]
    pub account_info: Option<AccountInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AccountConfig {
    #[serde(rename = "s")]
    pub symbol: String,

    #[serde(rename = "l")]
    pub leverage: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AccountInfo {
    #[serde(rename = "j")]
    pub multi_assets_mode: bool,
}

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
