use serde::Deserialize;
use virs_types::WsFeedEvent;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct MarginCallEvent {
    #[serde(rename = "e")]
    pub event_type: String,

    #[serde(rename = "E")]
    pub event_time: i64,

    #[serde(rename = "cw")]
    pub cross_wallet_balance: String,

    #[serde(rename = "p")]
    pub positions: Vec<MarginCallPosition>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct MarginCallPosition {
    #[serde(rename = "s")]
    pub symbol: String,

    #[serde(rename = "ps")]
    pub position_side: String,

    #[serde(rename = "pa")]
    pub position_amount: String,

    #[serde(rename = "mt")]
    pub margin_type: String,

    #[serde(rename = "iw")]
    pub isolated_wallet: String,

    #[serde(rename = "mp")]
    pub mark_price: String,

    #[serde(rename = "up")]
    pub unrealized_pnl: String,

    #[serde(rename = "mm")]
    pub maintenance_margin: String,
}

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
