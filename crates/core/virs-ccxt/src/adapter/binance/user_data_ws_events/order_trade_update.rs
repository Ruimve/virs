use std::sync::Arc;

use serde::Deserialize;
use virs_type::{
    CcxtOrder, CcxtOrderStatus, ExecutionType as CcxtExecutionType, PositionSide, WsFeedEvent,
};


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

    #[serde(default)]
    #[serde(rename = "ap")]
    pub avg_fill_price: String,

    #[serde(default)]
    #[serde(rename = "sp")]
    pub stop_price: String,

    #[serde(rename = "x")]
    pub execution_type: String,

    #[serde(rename = "X")]
    pub status: String,

    #[serde(rename = "i")]
    pub order_id: i64,

    #[serde(rename = "M")]
    pub modify_id: Option<String>,

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

    #[serde(default)]
    #[serde(rename = "b")]
    pub bids_notional: String,

    #[serde(default)]
    #[serde(rename = "a")]
    pub ask_notional: String,

    #[serde(rename = "m")]
    pub is_maker: bool,

    #[serde(rename = "R")]
    pub reduce_only: bool,

    #[serde(default)]
    #[serde(rename = "wt")]
    pub working_type: String,

    #[serde(default)]
    #[serde(rename = "ot")]
    pub original_order_type: String,

    #[serde(rename = "ps")]
    pub position_side: Option<String>,

    #[serde(rename = "cp")]
    pub close_position: Option<bool>,

    #[serde(rename = "AP")]
    pub activation_price: Option<String>,

    #[serde(rename = "cr")]
    pub callback_rate: Option<String>,

    #[serde(default)]
    #[serde(rename = "pP")]
    pub price_protection: bool,

    #[serde(default)]
    #[serde(rename = "rp")]
    pub realized_pnl: String,

    #[serde(default)]
    #[serde(rename = "V")]
    pub stp_mode: String,

    #[serde(default)]
    #[serde(rename = "pm")]
    pub price_match_mode: String,

    #[serde(default)]
    #[serde(rename = "gtd")]
    pub gtd_auto_cancel_time: i64,

    #[serde(default)]
    #[serde(rename = "er")]
    pub expiry_reason: String,

    #[serde(rename = "si")]
    pub si: Option<i64>,

    #[serde(rename = "ss")]
    pub ss: Option<i64>,
}

impl OrderTradeUpdateData {

    pub fn is_liquidation(&self) -> bool {
        self.execution_type == "CALCULATED" && self.client_order_id.starts_with("autoclose-")
    }


    pub fn is_adl(&self) -> bool {
        self.execution_type == "CALCULATED" && self.client_order_id == "adl_autoclose"
    }


    pub fn to_ws_feed_event(
        &self,
        envelope_event_type: &str,
        envelope_event_time: i64,
        envelope_transaction_time: i64,
    ) -> Option<WsFeedEvent> {

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


        let ccxt_order = self.to_ccxt_order(
            envelope_event_type,
            envelope_event_time,
            envelope_transaction_time,
        );
        Some(WsFeedEvent::OrderUpdate { order: Arc::new(ccxt_order) })
    }


    pub fn to_ccxt_order(
        &self,
        envelope_event_type: &str,
        envelope_event_time: i64,
        envelope_transaction_time: i64,
    ) -> CcxtOrder {
        let side = match self.side.as_str() {
            "BUY" => virs_type::Side::Buy,
            "SELL" => virs_type::Side::Sell,
            other => virs_type::Side::Unknown(other.to_string()),
        };

        let order_type =
            crate::adapter::binance::BinanceExchange::parse_order_type(&self.order_type);

        let position_side = match self.position_side.as_deref() {
            Some("LONG") => PositionSide::Long,
            Some("SHORT") => PositionSide::Short,
            Some(other) => PositionSide::Unknown(other.to_string()),
            None => PositionSide::Unknown("None".to_string()),
        };

        let status: CcxtOrderStatus = self.status.parse().unwrap();

        let execution_type: CcxtExecutionType = self.execution_type.parse().unwrap();

        CcxtOrder {
            order_id: self.order_id,
            client_order_id: self.client_order_id.clone(),
            symbol: self.symbol.clone(),
            side,
            order_type,
            position_side,
            original_order_type: crate::adapter::binance::BinanceExchange::parse_order_type(&self.original_order_type),
            status,
            execution_type,
            orig_qty: self.orig_qty.clone(),
            original_price: self.original_price.clone(),
            avg_fill_price: self.avg_fill_price.clone(),
            filled_qty: self.filled_qty.clone(),
            last_fill_qty: self.last_fill_qty.clone(),
            last_fill_price: self.last_fill_price.clone(),
            stop_price: self.stop_price.clone(),
            commission: self.commission.clone(),
            commission_asset: self.commission_asset.clone(),
            realized_pnl: self.realized_pnl.clone(),
            reduce_only: self.reduce_only,
            is_maker: self.is_maker,
            close_position: self.close_position,
            time_in_force: self.time_in_force.clone(),
            working_type: self.working_type.clone(),
            bids_notional: self.bids_notional.clone(),
            ask_notional: self.ask_notional.clone(),
            activation_price: self.activation_price.clone(),
            callback_rate: self.callback_rate.clone(),
            price_protection: self.price_protection,
            stp_mode: self.stp_mode.clone(),
            price_match_mode: self.price_match_mode.clone(),
            gtd_auto_cancel_time: self.gtd_auto_cancel_time,
            expiry_reason: self.expiry_reason.clone(),
            si: self.si,
            ss: self.ss,
            trade_time: self.trade_time,
            trade_id: self.trade_id,
            modify_id: self.modify_id.clone(),
            envelope_event_type: envelope_event_type.to_string(),
            envelope_event_time,
            envelope_transaction_time,
        }
    }
}


pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: OrderTradeUpdateEvent = serde_json::from_str(json).ok()?;
    event.order.to_ws_feed_event(
        &event.event_type,
        event.event_time,
        event.transaction_time,
    )
}
