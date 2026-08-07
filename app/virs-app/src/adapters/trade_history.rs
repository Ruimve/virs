use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::warn;
use virs_type::{TradeHistoryProvider, TradeRecord};

/// 基于 PostgreSQL 的交易历史数据源。
///
/// 从 `pe_auto_order_context` + `pe_order_latest` 查询已平仓交易记录，
/// 供 `StrategyEvaluator` 计算策略绩效指标。
pub struct PgTradeHistoryProvider {
    db: PgPool,
}

impl PgTradeHistoryProvider {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TradeHistoryProvider for PgTradeHistoryProvider {
    async fn query_trades(
        &self,
        strategy_name: &str,
        since: DateTime<Utc>,
    ) -> Vec<TradeRecord> {
        #[derive(sqlx::FromRow)]
        struct TradeRow {
            strategy_file: Option<String>,
            symbol: String,
            side: String,
            opened_at: DateTime<Utc>,
            closed_at: DateTime<Utc>,
            entry_price: f64,
            exit_price: f64,
            quantity: f64,
            realized_pnl: f64,
        }

        let rows = sqlx::query_as::<_, TradeRow>(
            r#"SELECT
                open_ctx.strategy_file,
                open_ctx.symbol,
                LOWER(open_ord.side) AS side,
                open_ctx.created_at AS opened_at,
                close_ctx.created_at AS closed_at,
                open_ord.avg_fill_price::float AS entry_price,
                close_ord.avg_fill_price::float AS exit_price,
                open_ord.filled_qty::float AS quantity,
                COALESCE(close_ord.realized_pnl::float, 0) AS realized_pnl
            FROM pe_auto_order_context open_ctx
            JOIN pe_order_latest open_ord ON open_ord.client_order_id = open_ctx.client_order_id
            JOIN pe_auto_order_context close_ctx ON close_ctx.paired_client_order_id = open_ctx.client_order_id
            JOIN pe_order_latest close_ord ON close_ord.client_order_id = close_ctx.client_order_id
            WHERE open_ctx.strategy_file = $1
              AND open_ctx.order_role = 'open'
              AND open_ctx.status = 'closed'
              AND close_ctx.created_at >= $2
            ORDER BY open_ctx.created_at ASC"#,
        )
        .bind(strategy_name)
        .bind(since)
        .fetch_all(&self.db)
        .await;

        match rows {
            Ok(rows) => rows
                .into_iter()
                .map(|r| TradeRecord {
                    strategy_name: r.strategy_file.unwrap_or_else(|| strategy_name.to_string()),
                    symbol: r.symbol,
                    side: r.side,
                    opened_at: r.opened_at,
                    closed_at: r.closed_at,
                    entry_price: r.entry_price,
                    exit_price: r.exit_price,
                    quantity: r.quantity,
                    realized_pnl: r.realized_pnl,
                })
                .collect(),
            Err(e) => {
                warn!(
                    strategy = %strategy_name,
                    error = %e,
                    "Failed to query trade history for strategy"
                );
                Vec::new()
            }
        }
    }
}
