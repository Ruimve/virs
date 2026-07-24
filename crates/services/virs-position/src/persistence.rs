use chrono::{DateTime, Utc};
use sqlx::PgPool;
use virs_error::{Context, VirsResult};
use virs_types::enums::PositionSide;
use virs_types::position::Position;
use virs_types::{CcxtOrder, CcxtOrderStatus, ExecutionType, OrderType, Side};

#[async_trait::async_trait]
pub trait PositionPersistence: Send + Sync {
    /// 从 pe_orders 聚合派生当前持仓，用于重启恢复
    async fn get_positions_from_orders(&self, exchange: &str) -> VirsResult<Vec<Position>>;

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
    async fn get_positions_from_orders(&self, exchange: &str) -> VirsResult<Vec<Position>> {
        self.get_positions_from_orders_impl(exchange).await
    }

    async fn upsert_order(&self, order: &CcxtOrder) -> VirsResult<()> {
        self.upsert_order_impl(order).await
    }

    async fn get_active_orders(&self) -> VirsResult<Vec<CcxtOrder>> {
        self.get_active_orders_impl().await
    }
}

impl Persistence {
    /// 从 pe_orders 回放派生当前持仓（Plan A: Rust replay）
    ///
    /// SQL 仅负责过滤和排序（保留 generation_filtered CTE 确保只回放当前代际），
    /// Rust 按 (symbol, position_side) 分组、按 trade_time 顺序逐笔回放 apply_fill，
    /// 保证 entry_price 计算与运行时一致（边际成本法：平仓不改 entry_price，
    /// 再开仓仅对剩余持仓做加权平均）。
    async fn get_positions_from_orders_impl(&self, exchange: &str) -> VirsResult<Vec<Position>> {
        let rows = sqlx::query_as::<_, ReplayOrderRow>(
            r#"
            WITH classified AS (
                SELECT
                    symbol, position_side, side,
                    filled_qty, avg_fill_price, realized_pnl,
                    trade_time, client_order_id,
                    CASE WHEN (side = 'BUY' AND position_side = 'LONG')
                           OR (side = 'SELL' AND position_side = 'SHORT')
                         THEN 1 ELSE 0 END AS is_open
                FROM pe_orders
                WHERE status IN ('FILLED', 'PARTIALLY_FILLED')
                  AND filled_qty::float8 > 0
            ),
            ordered AS (
                SELECT
                    *,
                    SUM(CASE WHEN is_open = 1 THEN filled_qty::float8 ELSE -filled_qty::float8 END)
                        OVER (PARTITION BY symbol, position_side ORDER BY trade_time, client_order_id)
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
                filled_qty, avg_fill_price, realized_pnl,
                trade_time, client_order_id
            FROM generation_filtered
            ORDER BY symbol, position_side, trade_time, client_order_id
            "#,
        )
        .fetch_all(&self.db)
        .await?;

        // Rust replay: 按 (symbol, position_side) 分组，按时间顺序回放 apply_fill
        let mut positions: Vec<Position> = Vec::new();
        let mut current_key: Option<(String, String)> = None;
        let mut current_pos: Option<Position> = None;

        for row in rows {
            let group_key = (row.symbol.clone(), row.position_side.clone());

            // 检测分组边界
            if current_key.as_ref() != Some(&group_key) {
                // 输出上一组仓位（仅保留有剩余持仓的）
                if let Some(pos) = current_pos.take() {
                    if pos.quantity > 1e-8 {
                        positions.push(pos);
                    }
                }

                // 校验 position_side
                virs_types::validate_position_side(Some(&row.position_side))
                    .context("DB replay position_side validation")?;
                let side = match row.position_side.as_str() {
                    "LONG" => PositionSide::Long,
                    "SHORT" => PositionSide::Short,
                    _ => unreachable!("validate_position_side 已保证为 LONG/SHORT"),
                };

                // generation_filtered 保证每组首单为开仓单（零持仓后只能开仓）
                let created_at = DateTime::from_timestamp_millis(row.trade_time)
                    .unwrap_or_else(Utc::now);

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

            // 判断开平仓方向
            let is_close = matches!(
                (row.side.as_str(), row.position_side.as_str()),
                ("SELL", "LONG") | ("BUY", "SHORT")
            );

            // 解析成交数量 — 不使用默认值，解析失败直接报错
            let trade_fill: f64 = row
                .filled_qty
                .parse()
                .context(format!("parse filled_qty '{}' for order {}", row.filled_qty, row.client_order_id))?;

            // 解析成交价格 — 开仓单必须有 avg_fill_price，平仓单不使用此字段
            let fill_price = if is_close {
                // apply_fill 在 is_close=true 时不读取 fill_price
                0.0
            } else {
                let avg_str = row
                    .avg_fill_price
                    .as_ref()
                    .context(format!(
                        "avg_fill_price is NULL for filled open order {}",
                        row.client_order_id
                    ))?;
                avg_str
                    .parse::<f64>()
                    .context(format!("parse avg_fill_price '{}' for order {}", avg_str, row.client_order_id))?
            };

            // 解析已实现盈亏 — 平仓单必须有 realized_pnl，开仓单 rp=0
            let realized_pnl = if is_close {
                let rp_str = row
                    .realized_pnl
                    .as_ref()
                    .context(format!(
                        "realized_pnl is NULL for close order {}",
                        row.client_order_id
                    ))?;
                rp_str
                    .parse::<f64>()
                    .context(format!("parse realized_pnl '{}' for order {}", rp_str, row.client_order_id))?
            } else {
                // 开仓单的 rp 始终为 0（Binance 对非平仓订单发送 rp=0）
                0.0
            };

            let timestamp = DateTime::from_timestamp_millis(row.trade_time)
                .unwrap_or_else(Utc::now);

            pos.apply_fill(is_close, fill_price, trade_fill, realized_pnl, timestamp);
        }

        // 输出最后一组仓位
        if let Some(pos) = current_pos {
            if pos.quantity > 1e-8 {
                positions.push(pos);
            }
        }

        Ok(positions)
    }

    async fn upsert_order_impl(&self, order: &CcxtOrder) -> VirsResult<()> {
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

        Ok(rows
            .into_iter()
            .filter_map(|r| r.into_ccxt_order())
            .collect())
    }
}

#[derive(Debug, sqlx::FromRow)]
struct ReplayOrderRow {
    symbol: String,
    position_side: String,
    side: String,
    filled_qty: String,
    avg_fill_price: Option<String>,
    realized_pnl: Option<String>,
    trade_time: i64,
    client_order_id: String,
}

#[derive(Debug, sqlx::FromRow)]
struct OrderRow {
    client_order_id: String,
    order_id: i64,
    symbol: String,
    side: String,
    order_type: String,
    position_side: String,
    original_order_type: Option<String>,
    status: String,
    execution_type: String,
    orig_qty: String,
    original_price: String,
    avg_fill_price: Option<String>,
    filled_qty: String,
    last_fill_qty: String,
    last_fill_price: String,
    stop_price: Option<String>,
    commission: String,
    commission_asset: String,
    realized_pnl: Option<String>,
    reduce_only: bool,
    is_maker: bool,
    close_position: Option<bool>,
    time_in_force: String,
    working_type: Option<String>,
    bids_notional: Option<String>,
    ask_notional: Option<String>,
    activation_price: Option<String>,
    callback_rate: Option<String>,
    price_protection: Option<bool>,
    stp_mode: Option<String>,
    price_match_mode: Option<String>,
    gtd_auto_cancel_time: Option<i64>,
    expiry_reason: Option<String>,
    si: Option<i64>,
    ss: Option<i64>,
    trade_time: i64,
    trade_id: i64,
}

impl OrderRow {
    fn into_ccxt_order(self) -> Option<CcxtOrder> {
        // DB 读取校验：side/position_side/status 非法值直接跳过（与 WS validate 共用校验逻辑）
        if let Err(e) = virs_types::validate_order_fields(
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
            _ => unreachable!("validate_order_fields 已保证到达此处时 side 为 BUY/SELL"),
        };
        // order_type: 纯信息字段，透传保留原始字符串
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
        let position_side = match self.position_side.as_str() {
            "LONG" => PositionSide::Long,
            "SHORT" => PositionSide::Short,
            _ => unreachable!("validate_order_fields 已保证到达此处时 position_side 为 LONG/SHORT"),
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
