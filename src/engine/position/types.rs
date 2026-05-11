use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// 1. 基础枚举
// ============================================================================

/// 交易方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

/// 持仓方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionSide {
    Long,
    Short,
    Both,
}

/// 订单类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderType {
    Limit,
    Market,
    StopMarket,
    TakeProfitMarket,
}

/// 订单状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Canceled,
    Failed,
}

/// 持仓状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionStatus {
    Empty,
    Opening,
    Open,
    Closing,
    Closed,
}

/// 市场类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketType {
    Spot,
    Perpetual,
}

/// 引擎状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineState {
    Created,
    Running,
    Paused,
    ShuttingDown,
    Stopped,
}

// ============================================================================
// 2. 辅助结构体
// ============================================================================

/// 行情快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticker {
    pub symbol: String,
    pub price: f64,
    pub bid: f64,
    pub ask: f64,
    pub volume_24h: f64,
    pub timestamp: DateTime<Utc>,
}

/// 账户余额
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub free: f64,
    pub used: f64,
    pub total: f64,
}

/// 交易所持仓信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangePosition {
    pub symbol: String,
    pub side: PositionSide,
    pub size: f64,
    pub entry_price: f64,
    pub leverage: u32,
    pub unrealized_pnl: f64,
    pub liquidation_price: Option<f64>,
}

/// 资金费率
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRate {
    pub symbol: String,
    pub rate: f64,
    pub next_funding_time: DateTime<Utc>,
}

/// 手续费率
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeRates {
    pub symbol: String,
    pub maker_rate: f64,
    pub taker_rate: f64,
}

// ============================================================================
// 3. WsFeedEvent 枚举
// ============================================================================

/// WebSocket 推送事件
#[derive(Debug, Clone)]
pub enum WsFeedEvent {
    OrderUpdate {
        exchange_order_id: String,
        symbol: String,
        status: OrderStatus,
        filled: f64,
        remaining: f64,
        price: f64,
        amount: f64,
        commission: f64,
        timestamp: DateTime<Utc>,
    },
    ConnectionChanged {
        connected: bool,
    },
}

// ============================================================================
// 4. 核心结构体
// ============================================================================

/// 持仓
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: Uuid,
    pub engine_id: String,
    pub strategy_id: Option<String>,
    pub exchange: String,
    pub symbol: String,
    pub side: PositionSide,
    pub status: PositionStatus,
    pub size: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub leverage: u32,
    pub margin: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub liquidation_price: Option<f64>,
    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
}

/// 订单
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub position_id: Uuid,
    pub exchange_order_id: Option<String>,
    pub client_order_id: Option<String>,
    pub exchange: String,
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub request_price: Option<f64>,
    pub fill_price: Option<f64>,
    pub amount: f64,
    pub filled: f64,
    pub remaining: f64,
    pub status: OrderStatus,
    pub reduce_only: bool,
    pub fee: f64,
    pub fee_currency: String,
    pub slippage: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 成交记录
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// 成交类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeType {
    Open,
    Close,
}

// ============================================================================
// 5. 命令和事件枚举
// ============================================================================

/// 引擎命令
#[derive(Debug, Clone)]
pub enum EngineCommand {
    OpenPosition {
        exchange: String,
        symbol: String,
        side: PositionSide,
        size: f64,
        leverage: Option<u32>,
        order_type: OrderType,
        price: Option<f64>,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
        strategy_id: Option<String>,
    },
    ClosePosition {
        position_id: Uuid,
        order_type: OrderType,
        price: Option<f64>,
    },
    ModifyPosition {
        position_id: Uuid,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
    },
    PlaceOrder {
        params: PlaceOrderParams,
    },
    CancelOrder {
        order_id: Uuid,
    },
    CancelAllOrders {
        position_id: Option<Uuid>,
        symbol: Option<String>,
    },
    SyncPositions,
    Shutdown,
}

/// 引擎事件
#[derive(Debug, Clone)]
pub enum EngineEvent {
    PositionOpened {
        position: Position,
    },
    PositionClosed {
        position: Position,
    },
    PositionModified {
        position_id: Uuid,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
    },
    OrderPlaced {
        order: Order,
    },
    OrderFilled {
        order: Order,
        trade: Trade,
    },
    OrderPartiallyFilled {
        order: Order,
        trade: Trade,
    },
    OrderCanceled {
        order: Order,
    },
    OrderFailed {
        order_id: Uuid,
        reason: String,
    },
    RiskAlert {
        level: String,
        message: String,
    },
    PositionSynced {
        positions: Vec<ExchangePosition>,
    },
    LiquidationWarning {
        position_id: Uuid,
        symbol: String,
        liquidation_price: f64,
        current_price: f64,
    },
}

// ============================================================================
// 6. PlaceOrderParams 结构体
// ============================================================================

/// 下单参数
#[derive(Debug, Clone)]
pub struct PlaceOrderParams {
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub amount: f64,
    pub price: Option<f64>,
    pub reduce_only: bool,
    pub position_side: Option<PositionSide>,
    pub position_id: Option<Uuid>,
    pub client_order_id: Option<String>,
}
