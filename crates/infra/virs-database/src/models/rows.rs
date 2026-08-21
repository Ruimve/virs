use chrono::DateTime;
use sqlx::FromRow;
use uuid::Uuid;

/* 交易历史查询行：用于策略评估的交易记录 */
#[derive(Debug, FromRow)]
pub struct TradeHistoryRow {
    pub strategy_file: Option<String>,
    pub symbol: String,
    pub side: String,
    pub opened_at: DateTime<chrono::Utc>,
    pub closed_at: DateTime<chrono::Utc>,
    pub entry_price: f64,
    pub exit_price: f64,
    pub quantity: f64,
    pub realized_pnl: f64,
}

/* Bot交易详情行：用于API返回的交易列表 */
#[derive(Debug, FromRow)]
pub struct BotTradeRow {
    pub open_client_order_id: String,
    pub close_client_order_id: Option<String>,
    pub bot_id: Uuid,
    pub symbol: String,
    pub exchange: String,
    pub open_side: String,
    pub open_price: f64,
    pub open_quantity: f64,
    pub open_fee: f64,
    pub opened_at: DateTime<chrono::Utc>,
    pub close_side: Option<String>,
    pub close_price: Option<f64>,
    pub close_quantity: Option<f64>,
    pub close_fee: f64,
    pub closed_at: Option<DateTime<chrono::Utc>>,
    pub pnl: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub close_reason: Option<String>,
    pub status: String,
}

/* 持仓重建行：通过聚合pe_trades表重建持仓状态 */
#[derive(Debug, FromRow)]
pub struct ReplayOrderRow {
    pub symbol: String,
    pub position_side: String,
    pub side: String,
    pub last_fill_qty: String,
    pub last_fill_price: String,
    pub realized_pnl: Option<String>,
    pub trade_time: i64,
    pub client_order_id: String,
}

/* 活跃订单行：从pe_order_latest视图查询NEW/PARTIALLY_FILLED状态订单 */
#[derive(Debug, FromRow)]
pub struct OrderRow {
    pub client_order_id: String,
    pub order_id: i64,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub position_side: String,
    pub original_order_type: String,
    pub status: String,
    pub execution_type: String,
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

impl OrderRow {
    /* DB字段转CcxtOrder：side/position_side/status不合法时返回None并记录错误 */
    pub fn into_ccxt_order(self) -> Option<virs_type::CcxtOrder> {
        use virs_type::{
            CcxtOrder, CcxtOrderStatus, ExecutionType, OrderType, PositionSide, Side,
        };

        if let Err(e) = CcxtOrder::validate_fields(
            &self.side,
            Some(&self.position_side),
            &self.status,
        ) {
            tracing::error!(
                client_order_id = %self.client_order_id,
                error = %e,
                "DB 订单字段校验失败，跳过该订单"
            );
            return None;
        }
        let side = match self.side.as_str() {
            "BUY" => Side::Buy,
            "SELL" => Side::Sell,
            _ => unreachable!("CcxtOrder::validate_fields 已保证到达此处时 side 为 BUY/SELL"),
        };

        let order_type = match self.order_type.as_str() {
            "LIMIT" => OrderType::Limit,
            "MARKET" => OrderType::Market,
            "STOP" => OrderType::Stop,
            "STOP_MARKET" => OrderType::StopMarket,
            "TAKE_PROFIT" => OrderType::TakeProfit,
            "TAKE_PROFIT_MARKET" => OrderType::TakeProfitMarket,
            "TRAILING_STOP_MARKET" => OrderType::TrailingStopMarket,
            "LIQUIDATION" => OrderType::Liquidation,
            other => OrderType::Unknown(other.to_string()),
        };

        let original_order_type = match self.original_order_type.as_str() {
            "LIMIT" => OrderType::Limit,
            "MARKET" => OrderType::Market,
            "STOP" => OrderType::Stop,
            "STOP_MARKET" => OrderType::StopMarket,
            "TAKE_PROFIT" => OrderType::TakeProfit,
            "TAKE_PROFIT_MARKET" => OrderType::TakeProfitMarket,
            "TRAILING_STOP_MARKET" => OrderType::TrailingStopMarket,
            "LIQUIDATION" => OrderType::Liquidation,
            other => OrderType::Unknown(other.to_string()),
        };
        let position_side = match self.position_side.as_str() {
            "LONG" => PositionSide::Long,
            "SHORT" => PositionSide::Short,
            _ => unreachable!("CcxtOrder::validate_fields 已保证到达此处时 position_side 为 LONG/SHORT"),
        };
        let status: CcxtOrderStatus = self.status.parse().unwrap();
        let execution_type: ExecutionType = self.execution_type.parse().unwrap();

        Some(CcxtOrder {
            order_id: self.order_id,
            client_order_id: self.client_order_id,
            symbol: self.symbol,
            side,
            order_type,
            position_side,
            original_order_type,
            status,
            execution_type,
            orig_qty: self.orig_qty,
            original_price: self.original_price,
            avg_fill_price: self.avg_fill_price,
            filled_qty: self.filled_qty,
            last_fill_qty: self.last_fill_qty,
            last_fill_price: self.last_fill_price,
            stop_price: self.stop_price,
            commission: self.commission,
            commission_asset: self.commission_asset,
            realized_pnl: self.realized_pnl,
            reduce_only: self.reduce_only,
            is_maker: self.is_maker,
            close_position: self.close_position,
            time_in_force: self.time_in_force,
            working_type: self.working_type,
            bids_notional: self.bids_notional,
            ask_notional: self.ask_notional,
            activation_price: self.activation_price,
            callback_rate: self.callback_rate,
            price_protection: self.price_protection,
            stp_mode: self.stp_mode,
            price_match_mode: self.price_match_mode,
            gtd_auto_cancel_time: self.gtd_auto_cancel_time,
            expiry_reason: self.expiry_reason,
            si: self.si,
            ss: self.ss,
            trade_time: self.trade_time,
            trade_id: self.trade_id,
            modify_id: self.modify_id,
            envelope_event_type: self.envelope_event_type,
            envelope_event_time: self.envelope_event_time,
            envelope_transaction_time: self.envelope_transaction_time,
        })
    }
}
