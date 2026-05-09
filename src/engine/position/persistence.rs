use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::error::Result;
use super::types::*;

#[async_trait::async_trait]
pub trait PositionPersistence: Send + Sync {
    async fn init_tables(&self) -> Result<()>;
    async fn upsert_position(&self, pos: &Position) -> Result<()>;
    async fn get_open_positions(&self, engine_id: &str) -> Result<Vec<Position>>;
    async fn get_position(&self, id: &Uuid) -> Result<Option<Position>>;
    async fn insert_order(&self, order: &Order) -> Result<()>;
    async fn update_order(&self, order: &Order) -> Result<()>;
    async fn get_active_orders(&self, engine_id: &str) -> Result<Vec<Order>>;
    async fn insert_trade(&self, trade: &Trade) -> Result<()>;
    async fn get_trades_by_position(&self, position_id: &Uuid) -> Result<Vec<Trade>>;
    async fn insert_pnl_snapshot(&self, engine_id: &str, snapshot: &PnlSnapshotRow) -> Result<()>;
    async fn get_latest_snapshot(&self, engine_id: &str) -> Result<Option<PnlSnapshotRow>>;
    async fn insert_event(&self, engine_id: &str, event_type: &str, symbol: Option<&str>, message: &str, severity: &str) -> Result<()>;
}

// ============================================================================
// PnlSnapshotRow - 本地定义，用于数据库读写
// ============================================================================

/// PnL 快照行记录（数据库读写用）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PnlSnapshotRow {
    pub id: Uuid,
    pub engine_id: String,
    pub timestamp: DateTime<Utc>,
    pub total_unrealized_pnl: f64,
    pub total_realized_pnl: f64,
    pub total_pnl: f64,
    pub position_count: i32,
    pub open_position_count: i32,
    pub total_margin: f64,
    pub drawdown_pct: f64,
    pub peak_equity: f64,
    pub total_trades: i32,
    pub profit_trades: i32,
    pub total_cost: f64,
    pub consecutive_losses: i32,
}

// ============================================================================
// Persistence
// ============================================================================

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
    async fn init_tables(&self) -> Result<()> {
        self.init_tables_impl().await
    }

    async fn upsert_position(&self, pos: &Position) -> Result<()> {
        self.upsert_position_impl(pos).await
    }

    async fn get_open_positions(&self, engine_id: &str) -> Result<Vec<Position>> {
        self.get_open_positions_impl(engine_id).await
    }

    async fn get_position(&self, id: &Uuid) -> Result<Option<Position>> {
        self.get_position_impl(id).await
    }

    async fn insert_order(&self, order: &Order) -> Result<()> {
        self.insert_order_impl(order).await
    }

    async fn update_order(&self, order: &Order) -> Result<()> {
        self.update_order_impl(order).await
    }

    async fn get_active_orders(&self, engine_id: &str) -> Result<Vec<Order>> {
        self.get_active_orders_impl(engine_id).await
    }

    async fn insert_trade(&self, trade: &Trade) -> Result<()> {
        self.insert_trade_impl(trade).await
    }

    async fn get_trades_by_position(&self, position_id: &Uuid) -> Result<Vec<Trade>> {
        self.get_trades_by_position_impl(position_id).await
    }

    async fn insert_pnl_snapshot(&self, engine_id: &str, snapshot: &PnlSnapshotRow) -> Result<()> {
        self.insert_pnl_snapshot_impl(engine_id, snapshot).await
    }

    async fn get_latest_snapshot(&self, engine_id: &str) -> Result<Option<PnlSnapshotRow>> {
        self.get_latest_snapshot_impl(engine_id).await
    }

    async fn insert_event(&self, engine_id: &str, event_type: &str, symbol: Option<&str>, message: &str, severity: &str) -> Result<()> {
        self.insert_event_impl(engine_id, event_type, symbol, message, severity).await
    }
}

impl Persistence {
    // -----------------------------------------------------------------------
    // 初始化数据库表
    // -----------------------------------------------------------------------

    async fn init_tables_impl(&self) -> Result<()> {
        // pe_positions
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pe_positions (
                id              UUID PRIMARY KEY,
                engine_id       TEXT NOT NULL,
                strategy_id     TEXT,
                exchange        TEXT NOT NULL,
                symbol          TEXT NOT NULL,
                side            TEXT NOT NULL,
                status          TEXT NOT NULL,
                size            DOUBLE PRECISION NOT NULL,
                entry_price     DOUBLE PRECISION NOT NULL,
                current_price   DOUBLE PRECISION NOT NULL,
                leverage        INT NOT NULL,
                margin          DOUBLE PRECISION NOT NULL,
                unrealized_pnl  DOUBLE PRECISION NOT NULL,
                realized_pnl    DOUBLE PRECISION NOT NULL,
                stop_loss       DOUBLE PRECISION,
                take_profit     DOUBLE PRECISION,
                liquidation_price DOUBLE PRECISION,
                opened_at       TIMESTAMPTZ NOT NULL,
                updated_at      TIMESTAMPTZ NOT NULL,
                closed_at       TIMESTAMPTZ,
                metadata        JSONB NOT NULL DEFAULT '{}',
                UNIQUE (engine_id, exchange, symbol, side)
            )
            "#,
        )
        .execute(&self.db)
        .await?;

        // pe_orders
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pe_orders (
                id                  UUID PRIMARY KEY,
                position_id         UUID NOT NULL REFERENCES pe_positions(id),
                exchange_order_id   TEXT,
                client_order_id     TEXT,
                exchange            TEXT NOT NULL,
                symbol              TEXT NOT NULL,
                side                TEXT NOT NULL,
                order_type          TEXT NOT NULL,
                request_price       DOUBLE PRECISION,
                fill_price          DOUBLE PRECISION,
                amount              DOUBLE PRECISION NOT NULL,
                filled              DOUBLE PRECISION NOT NULL,
                remaining           DOUBLE PRECISION NOT NULL,
                status              TEXT NOT NULL,
                reduce_only         BOOLEAN NOT NULL DEFAULT FALSE,
                fee                 DOUBLE PRECISION NOT NULL DEFAULT 0,
                fee_currency        TEXT NOT NULL DEFAULT '',
                slippage            DOUBLE PRECISION,
                created_at          TIMESTAMPTZ NOT NULL,
                updated_at          TIMESTAMPTZ NOT NULL
            )
            "#,
        )
        .execute(&self.db)
        .await?;

        // pe_trades
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pe_trades (
                id              UUID PRIMARY KEY,
                position_id     UUID NOT NULL REFERENCES pe_positions(id),
                order_id        UUID NOT NULL REFERENCES pe_orders(id),
                exchange        TEXT NOT NULL,
                symbol          TEXT NOT NULL,
                side            TEXT NOT NULL,
                price           DOUBLE PRECISION NOT NULL,
                amount          DOUBLE PRECISION NOT NULL,
                fee             DOUBLE PRECISION NOT NULL DEFAULT 0,
                fee_currency    TEXT NOT NULL DEFAULT '',
                pnl             DOUBLE PRECISION NOT NULL DEFAULT 0,
                trade_type      TEXT NOT NULL,
                created_at      TIMESTAMPTZ NOT NULL
            )
            "#,
        )
        .execute(&self.db)
        .await?;

        // pe_pnl_snapshots
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pe_pnl_snapshots (
                id                      UUID PRIMARY KEY,
                engine_id               TEXT NOT NULL,
                timestamp               TIMESTAMPTZ NOT NULL,
                total_unrealized_pnl    DOUBLE PRECISION NOT NULL,
                total_realized_pnl      DOUBLE PRECISION NOT NULL,
                total_pnl               DOUBLE PRECISION NOT NULL,
                position_count          INT NOT NULL,
                open_position_count     INT NOT NULL,
                total_margin            DOUBLE PRECISION NOT NULL,
                drawdown_pct            DOUBLE PRECISION NOT NULL DEFAULT 0,
                peak_equity             DOUBLE PRECISION NOT NULL DEFAULT 0,
                total_trades            INT NOT NULL DEFAULT 0,
                profit_trades           INT NOT NULL DEFAULT 0,
                total_cost              DOUBLE PRECISION NOT NULL DEFAULT 0,
                consecutive_losses      INT NOT NULL DEFAULT 0
            )
            "#,
        )
        .execute(&self.db)
        .await?;

        // pe_events
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pe_events (
                id          UUID PRIMARY KEY,
                engine_id   TEXT NOT NULL,
                event_type  TEXT NOT NULL,
                symbol      TEXT,
                message     TEXT NOT NULL,
                severity    TEXT NOT NULL DEFAULT 'info',
                created_at  TIMESTAMPTZ NOT NULL
            )
            "#,
        )
        .execute(&self.db)
        .await?;

        // ---- Indexes ----
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_pe_positions_engine_id ON pe_positions (engine_id)",
        )
        .execute(&self.db)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_pe_positions_status ON pe_positions (status)",
        )
        .execute(&self.db)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_pe_orders_position_id ON pe_orders (position_id)",
        )
        .execute(&self.db)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_pe_orders_status ON pe_orders (status)",
        )
        .execute(&self.db)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_pe_trades_position_id ON pe_trades (position_id)",
        )
        .execute(&self.db)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_pe_trades_order_id ON pe_trades (order_id)",
        )
        .execute(&self.db)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_pe_pnl_snapshots_engine_ts ON pe_pnl_snapshots (engine_id, timestamp DESC)",
        )
        .execute(&self.db)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_pe_events_engine_id ON pe_events (engine_id)",
        )
        .execute(&self.db)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_pe_events_created_at ON pe_events (created_at DESC)",
        )
        .execute(&self.db)
        .await?;

        Ok(())
    }

    // ===================================================================
    // Position CRUD
    // ===================================================================

    /// 插入或更新持仓（基于 UNIQUE(engine_id, exchange, symbol, side)）
    async fn upsert_position_impl(&self, pos: &Position) -> Result<()> {
        let side_str = format!("{:?}", pos.side);
        let status_str = format!("{:?}", pos.status);

        sqlx::query(
            r#"
            INSERT INTO pe_positions (
                id, engine_id, strategy_id, exchange, symbol, side, status,
                size, entry_price, current_price, leverage, margin,
                unrealized_pnl, realized_pnl, stop_loss, take_profit,
                liquidation_price, opened_at, updated_at, closed_at, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)
            ON CONFLICT (engine_id, exchange, symbol, side)
            DO UPDATE SET
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
                updated_at      = EXCLUDED.updated_at,
                closed_at       = EXCLUDED.closed_at,
                metadata        = EXCLUDED.metadata
            "#,
        )
        .bind(pos.id)
        .bind(&pos.engine_id)
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

    /// 获取指定引擎的所有未关闭持仓
    async fn get_open_positions_impl(&self, engine_id: &str) -> Result<Vec<Position>> {
        let rows = sqlx::query_as::<_, PositionRow>(
            r#"
            SELECT * FROM pe_positions
            WHERE engine_id = $1 AND status IN ('Opening', 'Open', 'Closing')
            ORDER BY opened_at
            "#,
        )
        .bind(engine_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rows.into_iter().filter_map(|r| r.into_position()).collect())
    }

    /// 按 ID 获取单个持仓
    async fn get_position_impl(&self, id: &Uuid) -> Result<Option<Position>> {
        let row = sqlx::query_as::<_, PositionRow>(
            "SELECT * FROM pe_positions WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.and_then(|r| r.into_position()))
    }

    // ===================================================================
    // Order CRUD
    // ===================================================================

    /// 插入新订单
    async fn insert_order_impl(&self, order: &Order) -> Result<()> {
        let side_str = format!("{:?}", order.side);
        let order_type_str = format!("{:?}", order.order_type);
        let status_str = format!("{:?}", order.status);

        sqlx::query(
            r#"
            INSERT INTO pe_orders (
                id, position_id, exchange_order_id, client_order_id,
                exchange, symbol, side, order_type, request_price, fill_price,
                amount, filled, remaining, status, reduce_only,
                fee, fee_currency, slippage, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
            "#,
        )
        .bind(order.id)
        .bind(order.position_id)
        .bind(&order.exchange_order_id)
        .bind(&order.client_order_id)
        .bind(&order.exchange)
        .bind(&order.symbol)
        .bind(&side_str)
        .bind(&order_type_str)
        .bind(order.request_price)
        .bind(order.fill_price)
        .bind(order.amount)
        .bind(order.filled)
        .bind(order.remaining)
        .bind(&status_str)
        .bind(order.reduce_only)
        .bind(order.fee)
        .bind(&order.fee_currency)
        .bind(order.slippage)
        .bind(order.created_at)
        .bind(order.updated_at)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// 更新订单（仅更新成交相关字段）
    async fn update_order_impl(&self, order: &Order) -> Result<()> {
        let status_str = format!("{:?}", order.status);

        sqlx::query(
            r#"
            UPDATE pe_orders SET
                filled          = $1,
                remaining       = $2,
                status          = $3,
                fee             = $4,
                fee_currency    = $5,
                slippage        = $6,
                fill_price      = $7,
                updated_at      = $8
            WHERE id = $9
            "#,
        )
        .bind(order.filled)
        .bind(order.remaining)
        .bind(&status_str)
        .bind(order.fee)
        .bind(&order.fee_currency)
        .bind(order.slippage)
        .bind(order.fill_price)
        .bind(order.updated_at)
        .bind(order.id)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// 获取指定引擎的活跃订单（open / partially_filled）
    async fn get_active_orders_impl(&self, engine_id: &str) -> Result<Vec<Order>> {
        let rows = sqlx::query_as::<_, OrderRow>(
            r#"
            SELECT o.* FROM pe_orders o
            INNER JOIN pe_positions p ON o.position_id = p.id
            WHERE p.engine_id = $1 AND o.status IN ('Open', 'PartiallyFilled')
            ORDER BY o.created_at
            "#,
        )
        .bind(engine_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rows.into_iter().filter_map(|r| r.into_order()).collect())
    }

    // ===================================================================
    // Trade
    // ===================================================================

    /// 插入成交记录
    async fn insert_trade_impl(&self, trade: &Trade) -> Result<()> {
        let side_str = format!("{:?}", trade.side);
        let trade_type_str = format!("{:?}", trade.trade_type).to_lowercase();

        sqlx::query(
            r#"
            INSERT INTO pe_trades (
                id, position_id, order_id, exchange, symbol, side,
                price, amount, fee, fee_currency, pnl, trade_type, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(trade.id)
        .bind(trade.position_id)
        .bind(trade.order_id)
        .bind(&trade.exchange)
        .bind(&trade.symbol)
        .bind(&side_str)
        .bind(trade.price)
        .bind(trade.amount)
        .bind(trade.fee)
        .bind(&trade.fee_currency)
        .bind(trade.pnl)
        .bind(&trade_type_str)
        .bind(trade.created_at)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// 获取指定持仓的所有成交记录
    async fn get_trades_by_position_impl(&self, position_id: &Uuid) -> Result<Vec<Trade>> {
        let rows = sqlx::query_as::<_, TradeRow>(
            r#"
            SELECT * FROM pe_trades
            WHERE position_id = $1
            ORDER BY created_at
            "#,
        )
        .bind(position_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rows.into_iter().filter_map(|r| r.into_trade()).collect())
    }

    // ===================================================================
    // PnL Snapshot
    // ===================================================================

    /// 插入 PnL 快照
    async fn insert_pnl_snapshot_impl(
        &self,
        engine_id: &str,
        snapshot: &PnlSnapshotRow,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO pe_pnl_snapshots (
                id, engine_id, timestamp,
                total_unrealized_pnl, total_realized_pnl, total_pnl,
                position_count, open_position_count, total_margin, drawdown_pct,
                peak_equity, total_trades, profit_trades, total_cost, consecutive_losses
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            "#,
        )
        .bind(snapshot.id)
        .bind(engine_id)
        .bind(snapshot.timestamp)
        .bind(snapshot.total_unrealized_pnl)
        .bind(snapshot.total_realized_pnl)
        .bind(snapshot.total_pnl)
        .bind(snapshot.position_count)
        .bind(snapshot.open_position_count)
        .bind(snapshot.total_margin)
        .bind(snapshot.drawdown_pct)
        .bind(snapshot.peak_equity)
        .bind(snapshot.total_trades)
        .bind(snapshot.profit_trades)
        .bind(snapshot.total_cost)
        .bind(snapshot.consecutive_losses)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// 获取指定引擎的最新 PnL 快照
    async fn get_latest_snapshot_impl(&self, engine_id: &str) -> Result<Option<PnlSnapshotRow>> {
        let row = sqlx::query_as::<_, PnlSnapshotRow>(
            r#"
            SELECT * FROM pe_pnl_snapshots
            WHERE engine_id = $1
            ORDER BY timestamp DESC
            LIMIT 1
            "#,
        )
        .bind(engine_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row)
    }

    // ===================================================================
    // Events
    // ===================================================================

    /// 插入引擎事件
    async fn insert_event_impl(
        &self,
        engine_id: &str,
        event_type: &str,
        symbol: Option<&str>,
        message: &str,
        severity: &str,
    ) -> Result<()> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO pe_events (id, engine_id, event_type, symbol, message, severity, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(id)
        .bind(engine_id)
        .bind(event_type)
        .bind(symbol)
        .bind(message)
        .bind(severity)
        .bind(now)
        .execute(&self.db)
        .await?;

        Ok(())
    }
}

// ============================================================================
// 内部 Row 类型（用于 sqlx::query_as 反序列化）
// ============================================================================

/// 持仓数据库行
#[derive(Debug, sqlx::FromRow)]
struct PositionRow {
    id: Uuid,
    engine_id: String,
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
            "Both" => PositionSide::Both,
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
            engine_id: self.engine_id,
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

/// 订单数据库行
#[derive(Debug, sqlx::FromRow)]
struct OrderRow {
    id: Uuid,
    position_id: Uuid,
    exchange_order_id: Option<String>,
    client_order_id: Option<String>,
    exchange: String,
    symbol: String,
    side: String,
    order_type: String,
    request_price: Option<f64>,
    fill_price: Option<f64>,
    amount: f64,
    filled: f64,
    remaining: f64,
    status: String,
    reduce_only: bool,
    fee: f64,
    fee_currency: String,
    slippage: Option<f64>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl OrderRow {
    fn into_order(self) -> Option<Order> {
        let side = match self.side.as_str() {
            "Buy" => Side::Buy,
            "Sell" => Side::Sell,
            _ => return None,
        };
        let order_type = match self.order_type.as_str() {
            "Limit" => OrderType::Limit,
            "Market" => OrderType::Market,
            "StopMarket" => OrderType::StopMarket,
            "TakeProfitMarket" => OrderType::TakeProfitMarket,
            _ => return None,
        };
        let status = match self.status.as_str() {
            "Pending" => OrderStatus::Pending,
            "Open" => OrderStatus::Open,
            "PartiallyFilled" => OrderStatus::PartiallyFilled,
            "Filled" => OrderStatus::Filled,
            "Canceled" => OrderStatus::Canceled,
            "Failed" => OrderStatus::Failed,
            _ => return None,
        };
        Some(Order {
            id: self.id,
            position_id: self.position_id,
            exchange_order_id: self.exchange_order_id,
            client_order_id: self.client_order_id,
            exchange: self.exchange,
            symbol: self.symbol,
            side,
            order_type,
            request_price: self.request_price,
            fill_price: self.fill_price,
            amount: self.amount,
            filled: self.filled,
            remaining: self.remaining,
            status,
            reduce_only: self.reduce_only,
            fee: self.fee,
            fee_currency: self.fee_currency,
            slippage: self.slippage,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

/// 成交记录数据库行
#[derive(Debug, sqlx::FromRow)]
struct TradeRow {
    id: Uuid,
    position_id: Uuid,
    order_id: Uuid,
    exchange: String,
    symbol: String,
    side: String,
    price: f64,
    amount: f64,
    fee: f64,
    fee_currency: String,
    pnl: f64,
    trade_type: String,
    created_at: DateTime<Utc>,
}

impl TradeRow {
    fn into_trade(self) -> Option<Trade> {
        let side = match self.side.as_str() {
            "Buy" => Side::Buy,
            "Sell" => Side::Sell,
            _ => return None,
        };
        let trade_type = match self.trade_type.to_lowercase().as_str() {
            "open" => TradeType::Open,
            _ => TradeType::Close,
        };
        Some(Trade {
            id: self.id,
            position_id: self.position_id,
            order_id: self.order_id,
            exchange: self.exchange,
            symbol: self.symbol,
            side,
            price: self.price,
            amount: self.amount,
            fee: self.fee,
            fee_currency: self.fee_currency,
            pnl: self.pnl,
            trade_type,
            created_at: self.created_at,
        })
    }
}
