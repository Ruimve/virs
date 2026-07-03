//! Position engine types: Position, Order, Trade, EngineCommand, EngineEvent, etc.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use virs_error::{VirsError, VirsResult};

use crate::enums::*;

/// Position
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

impl Position {
    pub fn is_open(&self) -> bool {
        self.status.is_open()
    }

    /// Computes unrealized PnL at a given current price.
    /// Long: (current - entry) * size
    /// Short: (entry - current) * size
    pub fn unrealized_pnl_at(&self, current_price: f64) -> f64 {
        match self.side {
            PositionSide::Long => (current_price - self.entry_price) * self.size,
            PositionSide::Short => (self.entry_price - current_price) * self.size,
            PositionSide::Both => 0.0,
        }
    }
}

/// Order (position engine)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionOrder {
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

/// Trade record
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

/// WebSocket feed event
#[derive(Debug, Clone, PartialEq)]
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
        position_side: Option<PositionSide>,
    },
    ConnectionChanged {
        connected: bool,
    },
}

/// Place order parameters
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

/// Engine command
#[derive(Debug, Clone)]
pub enum EngineCommand {
    OpenPosition {
        exchange: String,
        symbol: String,
        side: PositionSide,
        order_side: Side,
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
        strategy_id: Option<String>,
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
    CloseAllPositions {
        symbol: String,
    },
    PriceTick {
        symbol: String,
        price: f64,
    },
}

/// Engine event
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
    /// 仓位实时状态更新（来自 sync_loop 或 WS ACCOUNT_UPDATE）
    /// 推送给前端订阅 /ws/position 的客户端
    PositionUpdated {
        position: Position,
    },
    OrderPlaced {
        order: PositionOrder,
    },
    OrderFilled {
        order: PositionOrder,
        trade: Trade,
    },
    OrderPartiallyFilled {
        order: PositionOrder,
        trade: Trade,
    },
    OrderCanceled {
        order: PositionOrder,
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
        positions: Vec<crate::market::ExchangePosition>,
    },
    LiquidationWarning {
        position_id: Uuid,
        symbol: String,
        liquidation_price: f64,
        current_price: f64,
    },
}

/// Risk configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskConfig {
    #[serde(default = "default_max_position_per_symbol")]
    pub max_position_per_symbol_pct: f64,
    #[serde(default = "default_max_total_position")]
    pub max_total_position_pct: f64,
    #[serde(default = "default_max_order_amount")]
    pub max_order_amount_pct: f64,
    #[serde(default = "default_max_drawdown")]
    pub max_drawdown_pct: f64,
    #[serde(default = "default_max_leverage")]
    pub max_leverage: u32,
    #[serde(default = "default_funding_rate_threshold")]
    pub funding_rate_threshold: f64,
    #[serde(default = "default_liquidation_buffer")]
    pub liquidation_buffer_pct: f64,
    #[serde(default = "default_max_consecutive_losses")]
    pub max_consecutive_losses: u32,
}

fn default_max_position_per_symbol() -> f64 {
    1.0
}
fn default_max_total_position() -> f64 {
    3.0
}
fn default_max_order_amount() -> f64 {
    0.3
}
fn default_max_drawdown() -> f64 {
    0.15
}
fn default_max_leverage() -> u32 {
    20
}
fn default_funding_rate_threshold() -> f64 {
    0.001
}
fn default_liquidation_buffer() -> f64 {
    0.2
}
fn default_max_consecutive_losses() -> u32 {
    5
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_per_symbol_pct: default_max_position_per_symbol(),
            max_total_position_pct: default_max_total_position(),
            max_order_amount_pct: default_max_order_amount(),
            max_drawdown_pct: default_max_drawdown(),
            max_leverage: default_max_leverage(),
            funding_rate_threshold: default_funding_rate_threshold(),
            liquidation_buffer_pct: default_liquidation_buffer(),
            max_consecutive_losses: default_max_consecutive_losses(),
        }
    }
}

impl RiskConfig {
    /// Validates that all configuration fields are within reasonable bounds.
    /// Returns Ok(()) if valid, or Err(message) describing the first violation.
    pub fn validate(&self) -> VirsResult<()> {
        if self.max_leverage == 0 {
            return Err(VirsError::bad_request("max_leverage must be greater than 0"));
        }
        if self.max_drawdown_pct < 0.0 {
            return Err(VirsError::bad_request("max_drawdown_pct must not be negative"));
        }
        if self.max_position_per_symbol_pct < 0.0 {
            return Err(VirsError::bad_request(
                "max_position_per_symbol_pct must not be negative",
            ));
        }
        if self.max_total_position_pct < 0.0 {
            return Err(VirsError::bad_request(
                "max_total_position_pct must not be negative",
            ));
        }
        if self.max_order_amount_pct < 0.0 {
            return Err(VirsError::bad_request(
                "max_order_amount_pct must not be negative",
            ));
        }
        if self.liquidation_buffer_pct < 0.0 {
            return Err(VirsError::bad_request(
                "liquidation_buffer_pct must not be negative",
            ));
        }
        Ok(())
    }
}

/// Engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    pub engine_id: String,
    #[serde(default = "default_sync_interval")]
    pub sync_interval_secs: u64,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    #[serde(default = "default_ws_reconnect_timeout")]
    pub ws_reconnect_timeout_secs: u64,
    #[serde(default)]
    pub risk: RiskConfig,
    #[serde(default = "default_pnl_snapshot_interval")]
    pub pnl_snapshot_interval_secs: u64,
}

fn default_sync_interval() -> u64 {
    10
}
fn default_poll_interval() -> u64 {
    10
}
fn default_ws_reconnect_timeout() -> u64 {
    30
}
fn default_pnl_snapshot_interval() -> u64 {
    60
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            engine_id: "default".to_string(),
            sync_interval_secs: default_sync_interval(),
            poll_interval_secs: default_poll_interval(),
            ws_reconnect_timeout_secs: default_ws_reconnect_timeout(),
            risk: RiskConfig::default(),
            pnl_snapshot_interval_secs: default_pnl_snapshot_interval(),
        }
    }
}
