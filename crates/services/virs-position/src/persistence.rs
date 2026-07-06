//! Position persistence layer — PostgreSQL implementation.
//!
//! 仅保留 `pe_positions` 表用于引擎重启后快速恢复内存仓位状态。
//! 订单（pe_orders）、成交（pe_trades）、PnL 快照（pe_pnl_snapshots）、事件（pe_events）
//! 已删除：业务数据由 `qd_auto_trades`/`qd_grid_trades` 承载，引擎运行时状态在内存中维护。

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use virs_types::enums::{PositionSide, PositionStatus};
use virs_types::position::Position;
use virs_error::PositionResult;

// ============================================================================
// PositionPersistence trait
// ============================================================================

#[async_trait::async_trait]
pub trait PositionPersistence: Send + Sync {
    /// 写入/更新仓位。
    async fn upsert_position(&self, pos: &Position) -> PositionResult<()>;
    /// 获取所有未平仓仓位（用于重启恢复）。
    async fn get_open_positions(&self) -> PositionResult<Vec<Position>>;
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
    async fn upsert_position(&self, pos: &Position) -> PositionResult<()> {
        self.upsert_position_impl(pos).await
    }

    async fn get_open_positions(&self) -> PositionResult<Vec<Position>> {
        self.get_open_positions_impl().await
    }
}

impl Persistence {
    // ===================================================================
    // Position CRUD
    // ===================================================================

    async fn upsert_position_impl(&self, pos: &Position) -> PositionResult<()> {
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

    async fn get_open_positions_impl(&self) -> PositionResult<Vec<Position>> {
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
}

// ============================================================================
// 内部 Row 类型
// ============================================================================

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
