use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use virs_types::enums::{PositionSide, PositionStatus};
use virs_types::position::Position;
use virs_types::{CcxtOrder, CcxtOrderStatus, ExecutionType, OrderType, Side};
use virs_error::VirsResult;


#[async_trait::async_trait]
pub trait PositionPersistence: Send + Sync {

    async fn upsert_position(&self, pos: &Position) -> VirsResult<()>;

    async fn get_open_positions(&self) -> VirsResult<Vec<Position>>;

    async fn upsert_order(&self, order: &CcxtOrder) -> VirsResult<()>;

    async fn get_active_orders(&self) -> VirsResult<Vec<CcxtOrder>>;
}


pub struct Persistence {
    db: PgPool,
}

impl Persistence {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl PositionPersistence for Persistence {
    async fn upsert_position(&self, pos: &Position) -> VirsResult<()> {
        self.upsert_position_impl(pos).await
    }

    async fn get_open_positions(&self) -> VirsResult<Vec<Position>> {
        self.get_open_positions_impl().await
    }

    async fn upsert_order(&self, order: &CcxtOrder) -> VirsResult<()> {
        self.upsert_order_impl(order).await
    }

    async fn get_active_orders(&self) -> VirsResult<Vec<CcxtOrder>> {
        self.get_active_orders_impl().await
    }
}

impl Persistence {


    async fn upsert_position_impl(&self, pos: &Position) -> VirsResult<()> {
        let side_str = format!("{:?}", pos.side);
        let status_str = format!("{:?}", pos.status);

        sqlx::query(
            r#"
            INSERT INTO pe_positions (
                id, strategy_id, exchange, symbol, side, status,
                size, entry_price, current_price, leverage, margin,
                unrealized_pnl, realized_pnl, stop_loss, take_profit,
                liquidation_price, opened_at, updated_at, closed_at, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
            ON CONFLICT (exchange, symbol, side)
            DO UPDATE SET
                id              = EXCLUDED.id,
                strategy_id     = EXCLUDED.strategy_id,
                status          = EXCLUDED.status,
                size            = EXCLUDED.size,
                entry_price     = EXCLUDED.entry_price,
                current_price   = EXCLUDED.current_price,
                leverage        = EXCLUDED.leverage,
                margin          = EXCLUDED.margin,
                unrealized_pnl  = EXCLUDED.unrealized_pnl,
                realized_pnl    = EXCLUDED.realized_pnl,
                stop_loss       = EXCLUDED.stop_loss,
                take_profit     = EXCLUDED.take_profit,
                liquidation_price = EXCLUDED.liquidation_price,
                opened_at       = EXCLUDED.opened_at,
                updated_at      = EXCLUDED.updated_at,
                closed_at       = EXCLUDED.closed_at,
                metadata        = EXCLUDED.metadata
            "#,
        )
        .bind(pos.id)
        .bind(&pos.strategy_id)
        .bind(&pos.exchange)
        .bind(&pos.symbol)
        .bind(&side_str)
        .bind(&status_str)
        .bind(pos.size)
        .bind(pos.entry_price)
        .bind(pos.current_price)
        .bind(pos.leverage as i32)
        .bind(pos.margin)
        .bind(pos.unrealized_pnl)
        .bind(pos.realized_pnl)
        .bind(pos.stop_loss)
        .bind(pos.take_profit)
        .bind(pos.liquidation_price)
        .bind(pos.opened_at)
        .bind(pos.updated_at)
        .bind(pos.closed_at)
        .bind(sqlx::types::Json(&pos.metadata))
        .execute(&self.db)
        .await?;

        Ok(())
    }

    async fn get_open_positions_impl(&self) -> VirsResult<Vec<Position>> {
        let rows = sqlx::query_as::<_, PositionRow>(
            r#"
            SELECT * FROM pe_positions
            WHERE status IN ('Opening', 'Open', 'Closing')
            ORDER BY opened_at
            "#,
        )
        .fetch_all(&self.db)
        .await?;

        Ok(rows.into_iter().filter_map(|r| r.into_position()).collect())
    }

    async fn upsert_order_impl(&self, order: &CcxtOrder) -> VirsResult<()> {
        let side_str = match order.side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        };
        let order_type_str = match order.order_type {
            OrderType::Limit => "LIMIT",
            OrderType::Market => "MARKET",
            OrderType::Stop => "STOP",
            OrderType::StopMarket => "STOP_MARKET",
            OrderType::TakeProfit => "TAKE_PROFIT",
            OrderType::TakeProfitMarket => "TAKE_PROFIT_MARKET",
            OrderType::TrailingStopMarket => "TRAILING_STOP_MARKET",
            OrderType::Liquidation => "LIQUIDATION",
        };
        let position_side_str = match order.position_side {
            PositionSide::Long => "LONG",
            PositionSide::Short => "SHORT",
        };
        let status_str = match order.status {
            CcxtOrderStatus::New => "NEW",
            CcxtOrderStatus::PartiallyFilled => "PARTIALLY_FILLED",
            CcxtOrderStatus::Filled => "FILLED",
            CcxtOrderStatus::Canceled => "CANCELED",
            CcxtOrderStatus::Expired => "EXPIRED",
            CcxtOrderStatus::ExpiredInMatch => "EXPIRED_IN_MATCH",
        };
        let execution_type_str = match &order.execution_type {
            ExecutionType::New => "NEW",
            ExecutionType::Trade => "TRADE",
            ExecutionType::Canceled => "CANCELED",
            ExecutionType::Calculated => "CALCULATED",
            ExecutionType::Expired => "EXPIRED",
            ExecutionType::Amendment => "AMENDMENT",
            ExecutionType::Unknown(s) => s,
        };

        sqlx::query(
            r#"
            INSERT INTO pe_orders (
                client_order_id, order_id, symbol, side, order_type, position_side,
                original_order_type, status, execution_type,
                orig_qty, original_price, avg_fill_price, filled_qty,
                last_fill_qty, last_fill_price, stop_price,
                commission, commission_asset, realized_pnl,
                reduce_only, is_maker, close_position, time_in_force, working_type,
                bids_notional, ask_notional, activation_price, callback_rate,
                price_protection, stp_mode, price_match_mode, gtd_auto_cancel_time, expiry_reason,
                si, ss, trade_time, trade_id
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9,
                $10, $11, $12, $13, $14, $15, $16,
                $17, $18, $19,
                $20, $21, $22, $23, $24,
                $25, $26, $27, $28,
                $29, $30, $31, $32, $33,
                $34, $35, $36, $37
            )
            ON CONFLICT (client_order_id)
            DO UPDATE SET
                order_id              = EXCLUDED.order_id,
                symbol               = EXCLUDED.symbol,
                side                 = EXCLUDED.side,
                order_type           = EXCLUDED.order_type,
                position_side        = EXCLUDED.position_side,
                original_order_type  = EXCLUDED.original_order_type,
                status               = EXCLUDED.status,
                execution_type       = EXCLUDED.execution_type,
                orig_qty             = EXCLUDED.orig_qty,
                original_price       = EXCLUDED.original_price,
                avg_fill_price       = EXCLUDED.avg_fill_price,
                filled_qty           = EXCLUDED.filled_qty,
                last_fill_qty        = EXCLUDED.last_fill_qty,
                last_fill_price      = EXCLUDED.last_fill_price,
                stop_price           = EXCLUDED.stop_price,
                commission           = EXCLUDED.commission,
                commission_asset     = EXCLUDED.commission_asset,
                realized_pnl         = EXCLUDED.realized_pnl,
                reduce_only          = EXCLUDED.reduce_only,
                is_maker             = EXCLUDED.is_maker,
                close_position       = EXCLUDED.close_position,
                time_in_force        = EXCLUDED.time_in_force,
                working_type         = EXCLUDED.working_type,
                bids_notional        = EXCLUDED.bids_notional,
                ask_notional         = EXCLUDED.ask_notional,
                activation_price     = EXCLUDED.activation_price,
                callback_rate        = EXCLUDED.callback_rate,
                price_protection     = EXCLUDED.price_protection,
                stp_mode             = EXCLUDED.stp_mode,
                price_match_mode     = EXCLUDED.price_match_mode,
                gtd_auto_cancel_time = EXCLUDED.gtd_auto_cancel_time,
                expiry_reason        = EXCLUDED.expiry_reason,
                si                   = EXCLUDED.si,
                ss                   = EXCLUDED.ss,
                trade_time           = EXCLUDED.trade_time,
                trade_id             = EXCLUDED.trade_id
            "#,
        )
        .bind(&order.client_order_id)
        .bind(order.order_id)
        .bind(&order.symbol)
        .bind(side_str)
        .bind(order_type_str)
        .bind(position_side_str)
        .bind(&order.original_order_type)
        .bind(status_str)
        .bind(execution_type_str)
        .bind(&order.orig_qty)
        .bind(&order.original_price)
        .bind(&order.avg_fill_price)
        .bind(&order.filled_qty)
        .bind(&order.last_fill_qty)
        .bind(&order.last_fill_price)
        .bind(&order.stop_price)
        .bind(&order.commission)
        .bind(&order.commission_asset)
        .bind(&order.realized_pnl)
        .bind(order.reduce_only)
        .bind(order.is_maker)
        .bind(order.close_position)
        .bind(&order.time_in_force)
        .bind(&order.working_type)
        .bind(&order.bids_notional)
        .bind(&order.ask_notional)
        .bind(&order.activation_price)
        .bind(&order.callback_rate)
        .bind(order.price_protection)
        .bind(&order.stp_mode)
        .bind(&order.price_match_mode)
        .bind(order.gtd_auto_cancel_time)
        .bind(&order.expiry_reason)
        .bind(order.si)
        .bind(order.ss)
        .bind(order.trade_time)
        .bind(order.trade_id)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    async fn get_active_orders_impl(&self) -> VirsResult<Vec<CcxtOrder>> {
        let rows = sqlx::query_as::<_, OrderRow>(
            r#"
            SELECT * FROM pe_orders
            WHERE status IN ('NEW', 'PARTIALLY_FILLED')
            "#,
        )
        .fetch_all(&self.db)
        .await?;

        Ok(rows.into_iter().filter_map(|r| r.into_ccxt_order()).collect())
    }
}


#[derive(Debug, sqlx::FromRow)]
struct PositionRow {
    id: Uuid,
    strategy_id: Option<String>,
    exchange: String,
    symbol: String,
    side: String,
    status: String,
    size: f64,
    entry_price: f64,
    current_price: f64,
    leverage: i32,
    margin: f64,
    unrealized_pnl: f64,
    realized_pnl: f64,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    liquidation_price: Option<f64>,
    opened_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    metadata: sqlx::types::Json<serde_json::Value>,
}

impl PositionRow {
    fn into_position(self) -> Option<Position> {
        let side = match self.side.as_str() {
            "Long" => PositionSide::Long,
            "Short" => PositionSide::Short,
            _ => return None,
        };
        let status = match self.status.as_str() {
            "Empty" => PositionStatus::Empty,
            "Opening" => PositionStatus::Opening,
            "Open" => PositionStatus::Open,
            "Closing" => PositionStatus::Closing,
            "Closed" => PositionStatus::Closed,
            _ => return None,
        };
        Some(Position {
            id: self.id,
            strategy_id: self.strategy_id,
            exchange: self.exchange,
            symbol: self.symbol,
            side,
            status,
            size: self.size,
            entry_price: self.entry_price,
            current_price: self.current_price,
            leverage: self.leverage as u32,
            margin: self.margin,
            unrealized_pnl: self.unrealized_pnl,
            realized_pnl: self.realized_pnl,
            stop_loss: self.stop_loss,
            take_profit: self.take_profit,
            liquidation_price: self.liquidation_price,
            opened_at: self.opened_at,
            updated_at: self.updated_at,
            closed_at: self.closed_at,
            metadata: self.metadata.0,
        })
    }
}


#[derive(Debug, sqlx::FromRow)]
struct OrderRow {
    client_order_id: String,
    order_id: i64,
    symbol: String,
    side: String,
    order_type: String,
    position_side: String,
    original_order_type: String,
    status: String,
    execution_type: String,
    orig_qty: String,
    original_price: String,
    avg_fill_price: String,
    filled_qty: String,
    last_fill_qty: String,
    last_fill_price: String,
    stop_price: Option<String>,
    commission: String,
    commission_asset: String,
    realized_pnl: String,
    reduce_only: bool,
    is_maker: bool,
    close_position: Option<bool>,
    time_in_force: String,
    working_type: String,
    bids_notional: Option<String>,
    ask_notional: Option<String>,
    activation_price: Option<String>,
    callback_rate: Option<String>,
    price_protection: bool,
    stp_mode: Option<String>,
    price_match_mode: Option<String>,
    gtd_auto_cancel_time: Option<i64>,
    expiry_reason: Option<String>,
    si: i64,
    ss: i64,
    trade_time: i64,
    trade_id: i64,
}

impl OrderRow {
    fn into_ccxt_order(self) -> Option<CcxtOrder> {
        let side = match self.side.as_str() {
            "BUY" => Side::Buy,
            "SELL" => Side::Sell,
            _ => return None,
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
            _ => OrderType::Market,
        };
        let position_side = match self.position_side.as_str() {
            "LONG" => PositionSide::Long,
            "SHORT" => PositionSide::Short,
            _ => PositionSide::Long,
        };
        let status = CcxtOrderStatus::from_str(&self.status);
        let execution_type = ExecutionType::from_str(&self.execution_type);

        Some(CcxtOrder {
            order_id: self.order_id,
            client_order_id: self.client_order_id,
            symbol: self.symbol,
            side,
            order_type,
            position_side,
            original_order_type: self.original_order_type,
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
        })
    }
}
