use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use virs_error::ExchangeError;

use crate::order::{OrderStatus, OrderType};
use crate::position::PositionSide;
use crate::order::Side;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResult {
    pub order_id: String,
    pub client_order_id: String,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CcxtOrder {

    pub order_id: i64,
    pub client_order_id: String,


    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub position_side: PositionSide,
    pub original_order_type: OrderType,
    pub status: crate::order::enums::CcxtOrderStatus,
    pub execution_type: crate::order::enums::ExecutionType,


    pub orig_qty: String,
    pub original_price: String,
    pub avg_fill_price: String,
    pub filled_qty: String,
    pub last_fill_qty: String,
    pub last_fill_price: String,
    pub stop_price: String,


    pub commission: String,
    pub commission_asset: String,
    pub realized_pnl: String,


    pub reduce_only: bool,
    pub is_maker: bool,
    pub close_position: Option<bool>,
    pub time_in_force: String,
    pub working_type: String,


    pub bids_notional: String,
    pub ask_notional: String,


    pub activation_price: Option<String>,
    pub callback_rate: Option<String>,


    pub price_protection: bool,
    pub stp_mode: String,
    pub price_match_mode: String,
    pub gtd_auto_cancel_time: i64,
    pub expiry_reason: String,


    pub si: Option<i64>,
    pub ss: Option<i64>,


    pub trade_time: i64,
    pub trade_id: i64,


    pub modify_id: Option<String>,


    pub envelope_event_type: String,
    pub envelope_event_time: i64,
    pub envelope_transaction_time: i64,
}

impl CcxtOrder {


    pub fn validate_fields(
        side: &str,
        position_side: Option<&str>,
        status: &str,
    ) -> Result<(), ExchangeError> {
        Self::validate_side(side)?;
        Self::validate_position_side(position_side)?;
        Self::validate_status(status)?;
        Ok(())
    }


    fn validate_side(side: &str) -> Result<(), ExchangeError> {
        match side {
            "BUY" | "SELL" => Ok(()),
            other => Err(ExchangeError::InvalidOrderField(format!(
                "side 非法 (非 BUY/SELL): {other}"
            ))),
        }
    }


    pub fn validate_position_side(position_side: Option<&str>) -> Result<(), ExchangeError> {
        match position_side {
            Some("LONG") | Some("SHORT") => Ok(()),
            Some(other) => Err(ExchangeError::InvalidOrderField(format!(
                "position_side 非法 (非 LONG/SHORT): {other}"
            ))),
            None => Err(ExchangeError::InvalidOrderField(
                "position_side 为 None (OneWay 模式?)".to_string(),
            )),
        }
    }


    fn validate_status(status: &str) -> Result<(), ExchangeError> {
        match status {
            "NEW" | "PARTIALLY_FILLED" | "FILLED" | "CANCELED" | "CANCELLED" | "EXPIRED"
            | "EXPIRED_IN_MATCH" => Ok(()),
            other => Err(ExchangeError::InvalidOrderField(format!(
                "status 未知: {other}"
            ))),
        }
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub client_order_id: Option<String>,
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<f64>,
    pub amount: f64,
    pub cost: Option<f64>,
    pub filled: f64,
    pub remaining: f64,
    pub status: OrderStatus,
    pub fee: f64,
    pub fee_currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
