use chrono::{DateTime, Utc};
use serde::Deserialize;
use virs_types::{OrderStatus, PositionSide, WsFeedEvent};


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionType {

    New,

    Trade,

    Canceled,

    Calculated,

    Expired,

    Amendment,

    Unknown(String),
}

impl ExecutionType {
    pub fn from_str(s: &str) -> Self {
        match s {
            "NEW" => Self::New,
            "TRADE" => Self::Trade,
            "CANCELED" => Self::Canceled,
            "CALCULATED" => Self::Calculated,
            "EXPIRED" => Self::Expired,
            "AMENDMENT" => Self::Amendment,
            other => Self::Unknown(other.to_string()),
        }
    }


    pub fn is_liquidation(&self) -> bool {
        matches!(self, Self::Calculated)
    }
}


#[derive(Debug, Clone, Deserialize)]
pub struct OrderTradeUpdateEvent {

    #[serde(rename = "e")]
    pub event_type: String,

    #[serde(rename = "E")]
    pub event_time: i64,

    #[serde(rename = "T")]
    pub transaction_time: i64,

    #[serde(rename = "o")]
    pub order: OrderTradeUpdateData,
}


#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OrderTradeUpdateData {

    #[serde(rename = "s")]
    pub symbol: String,

    #[serde(rename = "c")]
    pub client_order_id: String,

    #[serde(rename = "S")]
    pub side: String,

    #[serde(rename = "o")]
    pub order_type: String,

    #[serde(rename = "f")]
    pub time_in_force: String,

    #[serde(rename = "q")]
    pub orig_qty: String,

    #[serde(rename = "p")]
    pub original_price: String,

    #[serde(rename = "ap")]
    pub avg_fill_price: Option<String>,

    #[serde(rename = "sp")]
    pub stop_price: Option<String>,

    #[serde(rename = "x")]
    pub execution_type: String,

    #[serde(rename = "X")]
    pub status: String,

    #[serde(rename = "i")]
    pub order_id: i64,

    #[serde(rename = "l")]
    pub last_fill_qty: String,

    #[serde(rename = "z")]
    pub filled_qty: String,

    #[serde(rename = "L")]
    pub last_fill_price: String,

    #[serde(rename = "N")]
    pub commission_asset: String,

    #[serde(rename = "n")]
    pub commission: String,

    #[serde(rename = "T")]
    pub trade_time: i64,

    #[serde(rename = "t")]
    pub trade_id: i64,

    #[serde(rename = "b")]
    pub bids_notional: Option<String>,

    #[serde(rename = "a")]
    pub ask_notional: Option<String>,

    #[serde(rename = "m")]
    pub is_maker: bool,

    #[serde(rename = "R")]
    pub is_reduce_only: bool,

    #[serde(rename = "wt")]
    pub working_type: Option<String>,

    #[serde(rename = "ot")]
    pub original_order_type: Option<String>,

    #[serde(rename = "ps")]
    pub position_side: Option<String>,

    #[serde(rename = "cp")]
    pub is_close_all: Option<bool>,

    #[serde(rename = "AP")]
    pub activation_price: Option<String>,

    #[serde(rename = "cr")]
    pub callback_rate: Option<String>,

    #[serde(rename = "pP")]
    pub price_protection: Option<bool>,

    #[serde(rename = "rp")]
    pub realized_pnl: Option<String>,

    #[serde(rename = "V")]
    pub stp_mode: Option<String>,

    #[serde(rename = "pm")]
    pub price_match_mode: Option<String>,

    #[serde(rename = "gtd")]
    pub gtd_auto_cancel_time: Option<i64>,

    #[serde(rename = "er")]
    pub expiry_reason: Option<String>,
}

impl OrderTradeUpdateData {

    fn to_order_status(&self) -> Option<OrderStatus> {
        match self.status.as_str() {
            "NEW" => Some(OrderStatus::Open),
            "PARTIALLY_FILLED" => Some(OrderStatus::PartiallyFilled),
            "FILLED" => Some(OrderStatus::Filled),
            "CANCELED" => Some(OrderStatus::Canceled),
            "EXPIRED" => Some(OrderStatus::Canceled),
            "EXPIRED_IN_MATCH" => Some(OrderStatus::Canceled),
            "REJECTED" => Some(OrderStatus::Failed),
            _ => None,
        }
    }


    fn to_position_side(&self) -> Option<PositionSide> {
        self.position_side.as_ref().and_then(|ps| match ps.as_str() {
            "LONG" => Some(PositionSide::Long),
            "SHORT" => Some(PositionSide::Short),
            _ => None,
        })
    }


    pub fn is_liquidation(&self) -> bool {
        self.execution_type == "CALCULATED" && self.client_order_id.starts_with("autoclose-")
    }


    pub fn is_adl(&self) -> bool {
        self.execution_type == "CALCULATED" && self.client_order_id == "adl_autoclose"
    }


    pub fn to_ws_feed_event(&self) -> Option<WsFeedEvent> {
        let status = self.to_order_status()?;
        let execution_type = ExecutionType::from_str(&self.execution_type);
        let position_side = self.to_position_side();


        if self.is_liquidation() {
            tracing::error!(
                symbol = %self.symbol,
                order_id = self.order_id,
                client_order_id = %self.client_order_id,
                "强制平仓事件 — 仓位已被交易所强平"
            );
        } else if self.is_adl() {
            tracing::error!(
                symbol = %self.symbol,
                order_id = self.order_id,
                client_order_id = %self.client_order_id,
                "ADL 事件 — 仓位被自动减仓"
            );
        }


        let filled = self.filled_qty.parse::<f64>().unwrap_or_else(|e| {
            tracing::error!(
                filled_qty = %self.filled_qty,
                error = %e,
                "解析 filled_qty 失败 — 跳过事件以避免 0.0 传播"
            );
            f64::NAN
        });
        if filled.is_nan() {
            return None;
        }


        let amount = self.orig_qty.parse::<f64>().unwrap_or_else(|e| {
            tracing::error!(
                orig_qty = %self.orig_qty,
                error = %e,
                "解析 orig_qty 失败 — 跳过事件以避免 0.0 传播"
            );
            f64::NAN
        });
        if amount.is_nan() {
            return None;
        }

        let remaining = (amount - filled).max(0.0);


        let price = match self
            .avg_fill_price
            .as_ref()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|&p| p > 0.0)
        {
            Some(p) => p,
            None => match self.last_fill_price.parse::<f64>() {
                Ok(p) if p > 0.0 => p,
                Ok(_) => {
                    tracing::warn!(
                        last_fill_price = %self.last_fill_price,
                        symbol = %self.symbol,
                        "last_fill_price 为 0.0 — 订单可能尚未成交"
                    );
                    0.0
                }
                Err(e) => {
                    tracing::error!(
                        last_fill_price = %self.last_fill_price,
                        error = %e,
                        "解析 last_fill_price 失败 — 跳过事件以避免 0.0 传播"
                    );
                    return None;
                }
            },
        };


        let commission = match self.commission.parse::<f64>() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(
                    commission = %self.commission,
                    error = %e,
                    "解析 commission 失败 — 跳过事件以避免 0.0 传播"
                );
                return None;
            }
        };


        let last_fill_qty = self.last_fill_qty.parse::<f64>().unwrap_or(0.0);


        let realized_pnl = self
            .realized_pnl
            .as_ref()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);


        let trade_id = if self.trade_id > 0 {
            Some(self.trade_id)
        } else {
            None
        };

        let timestamp = DateTime::from_timestamp_millis(self.trade_time).unwrap_or_else(|| {
            tracing::warn!(
                trade_time = self.trade_time,
                symbol = %self.symbol,
                order_id = self.order_id,
                "WS order trade_time 无效 — 使用本地时间作为 fallback"
            );
            Utc::now()
        });


        tracing::debug!(
            client_order_id = %self.client_order_id,
            execution_type = ?execution_type,
            last_fill_qty = last_fill_qty,
            realized_pnl = realized_pnl,
            trade_id = ?trade_id,
            is_reduce_only = self.is_reduce_only,
            is_maker = self.is_maker,
            "ORDER_TRADE_UPDATE 额外字段（待 WsFeedEvent 扩展后传递到下游）"
        );

        Some(WsFeedEvent::OrderUpdate {
            exchange_order_id: self.order_id.to_string(),
            client_order_id: Some(self.client_order_id.clone()),
            symbol: self.symbol.clone(),
            status,
            filled,
            remaining,
            price,
            amount,
            commission,
            timestamp,
            position_side,
        })
    }
}


pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: OrderTradeUpdateEvent = serde_json::from_str(json).ok()?;
    event.order.to_ws_feed_event()
}
