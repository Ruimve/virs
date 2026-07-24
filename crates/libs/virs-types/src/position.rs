use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ccxt_order::{CcxtOrder, OrderResult};
use crate::enums::*;


/// 基于 (exchange, symbol, side) 生成确定性 UUID v5，
/// 保证同一仓位在重启前后 ID 一致。
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
    /// 创建 Opening 状态的初始仓位（quantity=0, entry_price=0, realized_pnl=0）。
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

    /// 从 DB 回放创建初始仓位（replay 恢复用）。
    /// 与 new_opening 类似但接受 created_at 参数，用于精确恢复时间戳。
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

    /// 应用成交：原子更新 realized_pnl + quantity + entry_price + status。
    /// 返回 true 表示仓位已完全平仓（调用方应从 DashMap 移除）。
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
                self.quantity -= trade_fill;
                if self.quantity.abs() < 1e-8 {
                    self.quantity = 0.0;
                    self.status = PositionStatus::Closed;
                } else {
                    self.status = PositionStatus::Open;
                }
            } else {
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

    /// 设置为 Closing 状态（关仓下单时调用）。
    pub fn set_closing(&mut self, now: DateTime<Utc>) {
        self.status = PositionStatus::Closing;
        self.updated_at = now;
    }

    /// 回滚 Closing → Open（关仓下单失败时调用）。
    pub fn rollback_to_open(&mut self, now: DateTime<Utc>) {
        self.status = PositionStatus::Open;
        self.updated_at = now;
    }

    /// 判断是否为幽灵 Opening 仓位（未成交的开仓单占位）。
    pub fn is_ghost(&self) -> bool {
        self.status == PositionStatus::Opening && self.quantity == 0.0
    }

    pub fn is_open(&self) -> bool {
        self.status.is_open()
    }

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


#[derive(Debug, Clone, PartialEq)]
pub enum WsFeedEvent {
    OrderUpdate {
        order: CcxtOrder,
    },
    ConnectionChanged {
        connected: bool,
    },
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
}


// 下单预注册: client_order_id 先存入内存，REST + WS 双确认后才存入 orders
#[derive(Debug, Clone)]
pub struct PendingOrder {
    pub client_order_id: String,
    pub params: PlaceOrderParams,
    pub rest_result: Option<OrderResult>,
    pub ws_order: Option<CcxtOrder>,
    pub position_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}


#[derive(Debug, Clone)]
pub enum EngineCommand {
    OpenPosition {
        exchange: String,
        symbol: String,
        side: PositionSide,
        order_side: Side,
        quantity: f64,
        leverage: u32,
        order_type: OrderType,
        price: Option<f64>,
        client_order_id: Option<String>,
    },
    ClosePosition {
        position_id: Uuid,
        order_type: OrderType,
        price: Option<f64>,
        client_order_id: Option<String>,
    },
    PlaceOrder {
        params: PlaceOrderParams,
    },
    CancelAllOrders {
        position_id: Option<Uuid>,
        symbol: Option<String>,
    },
    CloseAllPositions {
        symbol: String,
    },
    PriceTick {
        symbol: String,
        price: f64,
    },
}


#[derive(Debug, Clone)]
pub enum EngineEvent {
    PositionOpened {
        position: Position,
    },
    PositionClosed {
        position: Position,
    },
    PositionUpdated {
        position: Position,
    },
    OrderPlaced {
        order: CcxtOrder,
    },
    OrderFilled {
        order: CcxtOrder,
        trade: Trade,
    },
    OrderPartiallyFilled {
        order: CcxtOrder,
        trade: Trade,
    },
    OrderCanceled {
        order: CcxtOrder,
    },
    OrderFailed {
        client_order_id: String,
        reason: String,
    },
    RiskAlert {
        level: String,
        message: String,
    },
}
