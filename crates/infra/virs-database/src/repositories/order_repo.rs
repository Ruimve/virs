use async_trait::async_trait;
use chrono::DateTime;
use sqlx::PgPool;
use virs_error::{Context, VirsError, VirsResult};
use virs_type::{
    CcxtOrder, CcxtOrderStatus, ExecutionType, OrderType, Position, PositionSide, PositionPersistence,
    Side,
};
use crate::models::{OrderRow, ReplayOrderRow};

pub struct PgOrderPersistence {
    db: PgPool,
}

impl PgOrderPersistence {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PositionPersistence for PgOrderPersistence {
    async fn get_positions_from_orders(&self, exchange: &str) -> VirsResult<Vec<Position>> {
        self.get_positions_from_orders_impl(exchange).await
    }

    async fn persist_order(&self, order: &CcxtOrder) -> VirsResult<()> {
        self.persist_order_impl(order).await
    }

    async fn persist_rejected_order(&self, order: &CcxtOrder, reason: &str) -> VirsResult<()> {
        self.persist_rejected_order_impl(order, reason).await
    }

    async fn get_active_orders(&self) -> VirsResult<Vec<CcxtOrder>> {
        self.get_active_orders_impl().await
    }
}

impl PgOrderPersistence {
    /* 通过聚合pe_trades表中的成交记录重建持仓状态，不依赖pe_positions表。
     * SQL使用代际过滤（generation_filtered）确保只回放当前持仓代际，
     * 避免历史已平仓订单污染当前持仓数据。 */
    async fn get_positions_from_orders_impl(&self, exchange: &str) -> VirsResult<Vec<Position>> {
        let rows = sqlx::query_as::<_, ReplayOrderRow>(
            r#"
            WITH classified AS (
                SELECT
                    symbol, position_side, side,
                    last_fill_qty, last_fill_price, realized_pnl,
                    trade_time, trade_id, client_order_id,
                    CASE WHEN (side = 'BUY' AND position_side = 'LONG')
                           OR (side = 'SELL' AND position_side = 'SHORT')
                         THEN 1 ELSE 0 END AS is_open
                FROM pe_trades
                WHERE last_fill_qty::float8 > 0
            ),
            ordered AS (
                SELECT
                    *,
                    SUM(CASE WHEN is_open = 1 THEN last_fill_qty::float8 ELSE -last_fill_qty::float8 END)
                        OVER (PARTITION BY symbol, position_side ORDER BY trade_time, trade_id)
                        AS running_qty
                FROM classified
            ),
            -- 找到每个 (symbol, position_side) 分组内最后一次净持仓归零点
            -- 仅保留归零点之后的行，确保只回放当前代际（避免历史已平仓订单污染）
            generation_filtered AS (
                SELECT * FROM (
                    SELECT
                        *,
                        MAX(CASE WHEN running_qty <= 0.00000001 THEN trade_time END)
                            OVER (PARTITION BY symbol, position_side) AS last_zero_time
                    FROM ordered
                ) t
                WHERE last_zero_time IS NULL OR trade_time > last_zero_time
            )
            SELECT
                symbol, position_side, side,
                last_fill_qty, last_fill_price, realized_pnl,
                trade_time, client_order_id
            FROM generation_filtered
            ORDER BY symbol, position_side, trade_time, trade_id
            "#,
        )
        .fetch_all(&self.db)
        .await?;

        let mut positions: Vec<Position> = Vec::new();
        let mut current_key: Option<(String, String)> = None;
        let mut current_pos: Option<Position> = None;

        for row in rows {
            let group_key = (row.symbol.clone(), row.position_side.clone());

            if current_key.as_ref() != Some(&group_key) {
                if let Some(pos) = current_pos.take() {
                    if pos.quantity > 1e-8 {
                        positions.push(pos);
                    }
                }

                virs_type::CcxtOrder::validate_position_side(Some(&row.position_side))
                    .context("DB replay position_side validation")?;
                let side = match row.position_side.as_str() {
                    "LONG" => PositionSide::Long,
                    "SHORT" => PositionSide::Short,
                    _ => unreachable!("CcxtOrder::validate_position_side 已保证为 LONG/SHORT"),
                };

                let created_at = DateTime::from_timestamp_millis(row.trade_time)
                    .ok_or_else(|| VirsError::bad_request(format!(
                        "Invalid trade_time {} for order {} during replay",
                        row.trade_time, row.client_order_id
                    )))?;

                current_pos = Some(Position::new_for_replay(
                    exchange,
                    &row.symbol,
                    side,
                    Some(row.client_order_id.clone()),
                    created_at,
                ));
                current_key = Some(group_key);
            }

            let pos = current_pos
                .as_mut()
                .expect("current_pos is always Some after group boundary detection");

            let is_close = matches!(
                (row.side.as_str(), row.position_side.as_str()),
                ("SELL", "LONG") | ("BUY", "SHORT")
            );

            let trade_fill: f64 = row
                .last_fill_qty
                .parse()
                .context(format!("parse last_fill_qty '{}' for order {}", row.last_fill_qty, row.client_order_id))?;

            let fill_price: f64 = row
                .last_fill_price
                .parse()
                .context(format!("parse last_fill_price '{}' for order {}", row.last_fill_price, row.client_order_id))?;

            let realized_pnl = match row.realized_pnl.as_deref() {
                Some(s) if !s.is_empty() => s
                    .parse::<f64>()
                    .context(format!("parse realized_pnl '{}' for order {}", s, row.client_order_id))?,
                _ => 0.0,
            };

            let timestamp = DateTime::from_timestamp_millis(row.trade_time)
                .ok_or_else(|| VirsError::bad_request(format!(
                    "Invalid trade_time {} for order {} during replay",
                    row.trade_time, row.client_order_id
                )))?;

            pos.apply_fill(is_close, fill_price, trade_fill, realized_pnl, timestamp);
        }

        if let Some(pos) = current_pos {
            if pos.quantity > 1e-8 {
                positions.push(pos);
            }
        }

        Ok(positions)
    }

    async fn persist_order_impl(&self, order: &CcxtOrder) -> VirsResult<()> {
        /* 根据execution_type路由：Trade事件写入pe_trades表，其他事件写入pe_order_events表 */
        let side_str = match &order.side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
            Side::Unknown(raw) => raw,
        };
        let order_type_str = match &order.order_type {
            OrderType::Limit => "LIMIT",
            OrderType::Market => "MARKET",
            OrderType::Stop => "STOP",
            OrderType::StopMarket => "STOP_MARKET",
            OrderType::TakeProfit => "TAKE_PROFIT",
            OrderType::TakeProfitMarket => "TAKE_PROFIT_MARKET",
            OrderType::TrailingStopMarket => "TRAILING_STOP_MARKET",
            OrderType::Liquidation => "LIQUIDATION",
            OrderType::Unknown(raw) => raw,
        };
        let original_order_type_str = match &order.original_order_type {
            OrderType::Limit => "LIMIT",
            OrderType::Market => "MARKET",
            OrderType::Stop => "STOP",
            OrderType::StopMarket => "STOP_MARKET",
            OrderType::TakeProfit => "TAKE_PROFIT",
            OrderType::TakeProfitMarket => "TAKE_PROFIT_MARKET",
            OrderType::TrailingStopMarket => "TRAILING_STOP_MARKET",
            OrderType::Liquidation => "LIQUIDATION",
            OrderType::Unknown(raw) => raw,
        };
        let position_side_str = match &order.position_side {
            PositionSide::Long => "LONG",
            PositionSide::Short => "SHORT",
            PositionSide::Unknown(raw) => raw,
        };
        let status_str = match &order.status {
            CcxtOrderStatus::New => "NEW",
            CcxtOrderStatus::PartiallyFilled => "PARTIALLY_FILLED",
            CcxtOrderStatus::Filled => "FILLED",
            CcxtOrderStatus::Canceled => "CANCELED",
            CcxtOrderStatus::Expired => "EXPIRED",
            CcxtOrderStatus::ExpiredInMatch => "EXPIRED_IN_MATCH",
            CcxtOrderStatus::Unknown(raw) => raw,
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

        let mut tx = self.db.begin().await.context("begin transaction for persist_order")?;

        /* Trade事件：使用(client_order_id, trade_id)作为唯一约束去重，防止重复写入 */
        if order.execution_type == ExecutionType::Trade {
            sqlx::query(
                r#"
                INSERT INTO pe_trades (
                    client_order_id, order_id, symbol, side, order_type, position_side,
                    original_order_type, status, execution_type,
                    orig_qty, original_price, avg_fill_price, filled_qty,
                    last_fill_qty, last_fill_price, stop_price,
                    commission, commission_asset, realized_pnl,
                    reduce_only, is_maker, close_position, time_in_force, working_type,
                    bids_notional, ask_notional, activation_price, callback_rate,
                    price_protection, stp_mode, price_match_mode, gtd_auto_cancel_time, expiry_reason,
                    si, ss, trade_time, trade_id,
                    modify_id, envelope_event_type, envelope_event_time, envelope_transaction_time
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9,
                    $10, $11, $12, $13, $14, $15, $16,
                    $17, $18, $19,
                    $20, $21, $22, $23, $24,
                    $25, $26, $27, $28,
                    $29, $30, $31, $32, $33,
                    $34, $35, $36, $37,
                    $38, $39, $40, $41
                )
                ON CONFLICT (client_order_id, trade_id)
                DO NOTHING
                "#,
            )
            .bind(&order.client_order_id)
            .bind(order.order_id)
            .bind(&order.symbol)
            .bind(side_str)
            .bind(order_type_str)
            .bind(position_side_str)
            .bind(original_order_type_str)
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
            .bind(&order.modify_id)
            .bind(&order.envelope_event_type)
            .bind(order.envelope_event_time)
            .bind(order.envelope_transaction_time)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO pe_order_events (
                    client_order_id, order_id, symbol, side, order_type, position_side,
                    original_order_type, status, execution_type,
                    orig_qty, original_price, avg_fill_price, filled_qty,
                    last_fill_qty, last_fill_price, stop_price,
                    commission, commission_asset, realized_pnl,
                    reduce_only, is_maker, close_position, time_in_force, working_type,
                    bids_notional, ask_notional, activation_price, callback_rate,
                    price_protection, stp_mode, price_match_mode, gtd_auto_cancel_time, expiry_reason,
                    si, ss, trade_time, trade_id,
                    modify_id, envelope_event_type, envelope_event_time, envelope_transaction_time
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9,
                    $10, $11, $12, $13, $14, $15, $16,
                    $17, $18, $19,
                    $20, $21, $22, $23, $24,
                    $25, $26, $27, $28,
                    $29, $30, $31, $32, $33,
                    $34, $35, $36, $37,
                    $38, $39, $40, $41
                )
                ON CONFLICT (client_order_id, execution_type, trade_id)
                DO NOTHING
                "#,
            )
            .bind(&order.client_order_id)
            .bind(order.order_id)
            .bind(&order.symbol)
            .bind(side_str)
            .bind(order_type_str)
            .bind(position_side_str)
            .bind(original_order_type_str)
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
            .bind(&order.modify_id)
            .bind(&order.envelope_event_type)
            .bind(order.envelope_event_time)
            .bind(order.envelope_transaction_time)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn persist_rejected_order_impl(
        &self,
        order: &CcxtOrder,
        reason: &str,
    ) -> VirsResult<()> {
        let side_str = match &order.side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
            Side::Unknown(raw) => raw,
        };
        let order_type_str = match &order.order_type {
            OrderType::Limit => "LIMIT",
            OrderType::Market => "MARKET",
            OrderType::Stop => "STOP",
            OrderType::StopMarket => "STOP_MARKET",
            OrderType::TakeProfit => "TAKE_PROFIT",
            OrderType::TakeProfitMarket => "TAKE_PROFIT_MARKET",
            OrderType::TrailingStopMarket => "TRAILING_STOP_MARKET",
            OrderType::Liquidation => "LIQUIDATION",
            OrderType::Unknown(raw) => raw,
        };
        let original_order_type_str = match &order.original_order_type {
            OrderType::Limit => "LIMIT",
            OrderType::Market => "MARKET",
            OrderType::Stop => "STOP",
            OrderType::StopMarket => "STOP_MARKET",
            OrderType::TakeProfit => "TAKE_PROFIT",
            OrderType::TakeProfitMarket => "TAKE_PROFIT_MARKET",
            OrderType::TrailingStopMarket => "TRAILING_STOP_MARKET",
            OrderType::Liquidation => "LIQUIDATION",
            OrderType::Unknown(raw) => raw,
        };
        let position_side_str = match &order.position_side {
            PositionSide::Long => "LONG",
            PositionSide::Short => "SHORT",
            PositionSide::Unknown(raw) => raw,
        };
        let status_str = match &order.status {
            CcxtOrderStatus::New => "NEW",
            CcxtOrderStatus::PartiallyFilled => "PARTIALLY_FILLED",
            CcxtOrderStatus::Filled => "FILLED",
            CcxtOrderStatus::Canceled => "CANCELED",
            CcxtOrderStatus::Expired => "EXPIRED",
            CcxtOrderStatus::ExpiredInMatch => "EXPIRED_IN_MATCH",
            CcxtOrderStatus::Unknown(raw) => raw,
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
            INSERT INTO pe_rejected_orders (
                client_order_id, order_id, symbol, side, order_type, position_side,
                original_order_type, status, execution_type,
                orig_qty, original_price, avg_fill_price, filled_qty,
                last_fill_qty, last_fill_price, stop_price,
                commission, commission_asset, realized_pnl,
                reduce_only, is_maker, close_position, time_in_force, working_type,
                bids_notional, ask_notional, activation_price, callback_rate,
                price_protection, stp_mode, price_match_mode, gtd_auto_cancel_time, expiry_reason,
                si, ss, trade_time, trade_id,
                modify_id, envelope_event_type, envelope_event_time, envelope_transaction_time,
                rejection_reason
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9,
                $10, $11, $12, $13, $14, $15, $16,
                $17, $18, $19,
                $20, $21, $22, $23, $24,
                $25, $26, $27, $28,
                $29, $30, $31, $32, $33,
                $34, $35, $36, $37,
                $38, $39, $40, $41,
                $42
            )
            ON CONFLICT (client_order_id, execution_type, trade_id)
            DO NOTHING
            "#,
        )
        .bind(&order.client_order_id)
        .bind(order.order_id)
        .bind(&order.symbol)
        .bind(side_str)
        .bind(order_type_str)
        .bind(position_side_str)
        .bind(original_order_type_str)
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
        .bind(&order.modify_id)
        .bind(&order.envelope_event_type)
        .bind(order.envelope_event_time)
        .bind(order.envelope_transaction_time)
        .bind(reason)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /* 查询所有活跃订单（NEW/PARTIALLY_FILLED状态）用于重启恢复 */
    async fn get_active_orders_impl(&self) -> VirsResult<Vec<CcxtOrder>> {
        let rows = sqlx::query_as::<_, OrderRow>(
            r#"
            SELECT * FROM pe_order_latest
            WHERE status IN ('NEW', 'PARTIALLY_FILLED')
            "#,
        )
        .fetch_all(&self.db)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| r.into_ccxt_order())
            .collect())
    }
}

pub async fn fetch_stop_loss_take_profit(
    db: &PgPool,
    symbol: &str,
    exchange: &str,
    side_str: &str,
) -> (Option<f64>, Option<f64>) {
    let row: Result<(f64, f64), _> = sqlx::query_as(
        r#"SELECT ctx.stop_loss, ctx.take_profit
           FROM pe_bot_order_context ctx
           JOIN pe_order_latest o ON o.client_order_id = ctx.client_order_id
           WHERE ctx.symbol = $1 AND ctx.exchange = $2
             AND ctx.order_role = 'open' AND ctx.status = 'open'
             AND o.position_side = $3
           ORDER BY ctx.created_at DESC LIMIT 1"#,
    )
    .bind(symbol)
    .bind(exchange)
    .bind(side_str)
    .fetch_one(db)
    .await;

    match row {
        Ok((sl, tp)) => {
            let sl = if sl > 0.0 { Some(sl) } else { None };
            let tp = if tp > 0.0 { Some(tp) } else { None };
            (sl, tp)
        }
        Err(_) => (None, None),
    }
}
