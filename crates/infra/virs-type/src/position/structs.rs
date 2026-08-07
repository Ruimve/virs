use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::order::{CcxtOrder, OrderResult, Side, OrderType};
use crate::position::{PositionSide, PositionStatus, TradeType};


/* 基于交易所、交易对和持仓方向生成 UUID v5 确定性 ID：同一标的+方向始终映射到同一 UUID，用于持仓幂等性 */
pub fn position_uuid_v5(exchange: &str, symbol: &str, side: &PositionSide) -> Uuid {
    let side_str = match side {
        PositionSide::Long => "LONG",
        PositionSide::Short => "SHORT",
        PositionSide::Unknown(raw) => raw.as_str(),
    };
    let key = format!("{}:{}:{}", exchange, symbol, side_str);
    Uuid::new_v5(&Uuid::NAMESPACE_URL, key.as_bytes())
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub id: Uuid,
    pub exchange: String,
    pub symbol: String,
    pub side: PositionSide,
    pub status: PositionStatus,
    pub quantity: f64,
    pub entry_price: f64,
    pub realized_pnl: f64,
    pub client_order_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Position {

    pub fn new_opening(exchange: &str, symbol: &str, side: PositionSide, client_order_id: Option<String>) -> Self {
        let id = position_uuid_v5(exchange, symbol, &side);
        let now = Utc::now();
        Self {
            id,
            exchange: exchange.to_string(),
            symbol: symbol.to_string(),
            side,
            status: PositionStatus::Opening,
            quantity: 0.0,
            entry_price: 0.0,
            realized_pnl: 0.0,
            client_order_id,
            created_at: now,
            updated_at: now,
        }
    }


    pub fn new_for_replay(
        exchange: &str,
        symbol: &str,
        side: PositionSide,
        client_order_id: Option<String>,
        created_at: DateTime<Utc>,
    ) -> Self {
        let id = position_uuid_v5(exchange, symbol, &side);
        Self {
            id,
            exchange: exchange.to_string(),
            symbol: symbol.to_string(),
            side,
            status: PositionStatus::Opening,
            quantity: 0.0,
            entry_price: 0.0,
            realized_pnl: 0.0,
            client_order_id,
            created_at,
            updated_at: created_at,
        }
    }


    /*
     * 应用成交到持仓：核心 PnL 计算逻辑。
     * 开仓时按加权平均更新 entry_price；平仓时减少数量并在数量归零时标记为已平仓。
     * 返回值表示持仓是否已完全平仓。
     */
    pub fn apply_fill(
        &mut self,
        is_close: bool,
        fill_price: f64,
        trade_fill: f64,
        realized_pnl: f64,
        timestamp: DateTime<Utc>,
    ) -> bool {
        if realized_pnl != 0.0 {
            self.realized_pnl += realized_pnl;
        }
        if trade_fill > 0.0 {
            if is_close {
                /* 平仓：减少持仓数量，数量归零（容差 1e-8）时标记为已平仓 */
                self.quantity -= trade_fill;
                if self.quantity.abs() < 1e-8 {
                    self.quantity = 0.0;
                    self.status = PositionStatus::Closed;
                } else {
                    self.status = PositionStatus::Open;
                }
            } else {
                /* 开仓：按加权平均法更新 entry_price，仅在已有持仓且价格有效时进行加权计算 */
                let old_qty = self.quantity;
                self.quantity += trade_fill;
                if fill_price > 0.0 {
                    if old_qty > 0.0 && self.entry_price > 0.0 {
                        let total_cost = self.entry_price * old_qty + fill_price * trade_fill;
                        self.entry_price = total_cost / self.quantity;
                    } else {
                        self.entry_price = fill_price;
                    }
                }
                self.status = PositionStatus::Open;
            }
        }
        self.updated_at = timestamp;
        self.status == PositionStatus::Closed
    }


    pub fn set_closing(&mut self, now: DateTime<Utc>) {
        self.status = PositionStatus::Closing;
        self.updated_at = now;
    }


    pub fn rollback_to_open(&mut self, now: DateTime<Utc>) {
        self.status = PositionStatus::Open;
        self.updated_at = now;
    }


    /* 幽灵持仓检测：状态为 Opening 且数量为 0，表示已创建但从未有成交的空持仓 */
    pub fn is_ghost(&self) -> bool {
        self.status == PositionStatus::Opening && self.quantity == 0.0
    }

    pub fn is_open(&self) -> bool {
        self.status.is_open()
    }

    /* 计算指定当前价格下的未实现盈亏：多头=(现价-开仓价)*数量, 空头=(开仓价-现价)*数量 */
    pub fn unrealized_pnl_at(&self, current_price: f64) -> f64 {
        match self.side {
            PositionSide::Long => (current_price - self.entry_price) * self.quantity,
            PositionSide::Short => (self.entry_price - current_price) * self.quantity,
            PositionSide::Unknown(_) => 0.0,
        }
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub id: Uuid,
    pub position_id: Uuid,
    pub order_id: Uuid,
    pub exchange: String,
    pub symbol: String,
    pub side: Side,
    pub price: f64,
    pub amount: f64,
    pub fee: f64,
    pub fee_currency: String,
    pub pnl: f64,
    pub trade_type: TradeType,
    pub created_at: DateTime<Utc>,
}


#[derive(Debug, Clone)]
pub struct PlaceOrderParams {
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub amount: f64,
    pub price: Option<f64>,
    pub position_side: Option<PositionSide>,
    pub position_id: Option<Uuid>,
    pub client_order_id: Option<String>,
    pub stop_price: Option<f64>,
    pub time_in_force: Option<crate::order::TimeInForce>,
}


#[derive(Debug, Clone)]
pub struct PendingOrder {
    pub client_order_id: String,
    pub params: PlaceOrderParams,
    pub rest_result: Option<OrderResult>,
    pub ws_order: Option<Arc<CcxtOrder>>,
    pub position_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
