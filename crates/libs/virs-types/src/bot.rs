use async_trait::async_trait;
use uuid::Uuid;

use virs_error::{BotResult, VirsResult};

use crate::enums::{PositionSide, Side};
use crate::market::Balance;
use crate::position::Position;


#[derive(Debug, Clone)]
pub struct OrderInfo {
    pub id: Uuid,
    pub position_id: Option<Uuid>,
    pub symbol: String,
    pub side: Side,
    pub fill_price: Option<f64>,
    pub request_price: Option<f64>,
    pub filled: f64,
    pub client_order_id: Option<String>,

    pub fee: f64,
}


#[derive(Debug, Clone)]
pub enum OrderCommand {
    OpenPosition {
        symbol: String,
        side: PositionSide,
        order_side: Side,
        amount: f64,
        leverage: u32,
        price: Option<f64>,
        client_order_id: Option<String>,
    },
    ClosePosition {
        position_id: Uuid,
        price: Option<f64>,
        client_order_id: Option<String>,
    },
    PlaceOrder {
        symbol: String,
        side: Side,
        amount: f64,
        price: Option<f64>,
        position_side: Option<PositionSide>,
        position_id: Option<Uuid>,
        client_order_id: Option<String>,
    },
    CancelAllOrders {
        symbol: Option<String>,
    },
    CloseAllPositions {
        symbol: String,
        exchange: String,
    },
}


#[derive(Debug, Clone)]
pub enum OrderEvent {
    OrderPlaced {
        order: OrderInfo,
    },
    OrderFilled {
        order: OrderInfo,
    },
    OrderPartiallyFilled {
        order: OrderInfo,
    },
    OrderCanceled {
        order_id: Uuid,
        client_order_id: Option<String>,
        symbol: Option<String>,
    },
    OrderFailed {
        order_id: Uuid,
        client_order_id: Option<String>,
        reason: String,
    },
    RiskAlert {
        level: String,
        message: String,
    },
}


#[async_trait]
pub trait OrderExecutor: Send + Sync {
    async fn send_command(&self, command: OrderCommand) -> BotResult<()>;

    /// 查询单个 open 仓位（Hedge 模式下仅返回第一个匹配，不保证 side）。
    ///
    /// 默认委托 [`query_open_positions`](Self::query_open_positions) 取首个匹配。
    /// 实现方可按需覆写以提供更高效的直查路径。
    async fn query_open_position(&self, symbol: &str) -> BotResult<Option<Position>> {
        Ok(self.query_open_positions(symbol).await?.into_iter().next())
    }

    /// 查询指定 symbol 下所有 open 仓位（Hedge 模式下可能同时返回 Long 和 Short）。
    async fn query_open_positions(&self, symbol: &str) -> BotResult<Vec<Position>>;
}


#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn load_credentials(
        &self,
        user_id: Uuid,
    ) -> BotResult<Vec<(String, String, Option<String>)>>;
}


#[derive(Debug, Clone)]
pub struct MarketSnapshot {
    pub current_price: f64,
    pub funding_rate: f64,
    pub funding_next_time: String,
    pub min_qty: f64,

    pub indicators_json: serde_json::Value,
}


#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    async fn get_market_snapshot(
        &self,
        exchange: &str,
        symbol: &str,
    ) -> VirsResult<MarketSnapshot>;
    async fn get_account_balance(&self, exchange: &str) -> VirsResult<Balance>;
}


pub trait LlmProviderResolver: Send + Sync {
    fn is_available(&self) -> bool;
    fn resolve(
        &self,
        user_credentials: &[(String, String, Option<String>)],
    ) -> BotResult<(String, String, String, String)>;
}
