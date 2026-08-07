use async_trait::async_trait;
use uuid::Uuid;

use virs_error::{BotResult, VirsResult};

use crate::exchange::{ExchangePe, MarketType};
use crate::market::{Balance, Candle, KlineEvent, Timeframe};
use crate::position::{EngineCommand, EngineEvent, Position};
use crate::ws_types::OrderBookEvent;

use super::structs::MarketSnapshot;


#[async_trait]
pub trait OrderExecutor: Send + Sync {
    async fn send_command(&self, command: super::enums::OrderCommand) -> BotResult<()>;

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

/// K 线事件源 trait。
///
/// `virs-trading-bot` 通过此 trait 订阅 K 线事件，无需依赖 `virs-market` 的 `KlineEngine` 具体类型。
pub trait KlineEventSource: Send + Sync {
    fn subscribe_kline_events(&self) -> tokio::sync::broadcast::Receiver<KlineEvent>;
}

/// K 线引擎句柄 trait — 封装 KlineEngine 的外部可见能力。
///
/// `virs-app` 和 `virs-api` 通过此 trait 操作 K 线引擎，无需依赖 `virs-market` 的具体类型。
/// 继承 [`KlineEventSource`] 以支持向 `Arc<dyn KlineEventSource>` 的自动协变。
#[async_trait]
pub trait KlineEngineHandle: KlineEventSource {
    /// 订阅交易对的市场数据（自动启动引擎）。
    async fn subscribe_market(
        &self,
        exchange: &str,
        symbol: &str,
        market_type: MarketType,
    ) -> VirsResult<()>;

    /// 停止引擎。
    ///
    /// 异步方法：取消内部 WS 循环任务并 `join()` 等待完全停止。
    /// 引擎任务在内部管理（无外部 TaskHandle），由 `stop()` 负责回收。
    async fn stop(&self);

    /// 查询缓存的 K 线数据。
    async fn get_klines(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: Timeframe,
    ) -> Option<Vec<Candle>>;
}

/// OrderBook 引擎句柄 trait — 封装 OrderBookEngine 的外部可见能力。
///
/// `virs-app` 和 `virs-api` 通过此 trait 操作 OrderBook 引擎，无需依赖 `virs-market` 的具体类型。
#[async_trait]
pub trait OrderBookEngineHandle: Send + Sync {
    /// 订阅 OrderBook 事件流。
    fn subscribe_orderbook_events(&self) -> tokio::sync::broadcast::Receiver<OrderBookEvent>;

    /// 订阅交易对的 OrderBook 数据（自动启动引擎）。
    async fn subscribe_market(
        &self,
        exchange: &str,
        symbol: &str,
        market_type: MarketType,
    ) -> VirsResult<()>;

    /// 停止引擎。
    ///
    /// 异步方法：取消内部 WS 循环任务并 `join()` 等待完全停止。
    /// 引擎任务在内部管理（无外部 TaskHandle），由 `stop()` 负责回收。
    async fn stop(&self);
}

/// 仓位引擎句柄 trait — 封装 PositionEngine 的外部可见能力。
///
/// `virs-app` 通过此 trait 操作仓位引擎，无需依赖 `virs-position` 的具体类型。
/// `run()` 方法因需要 `&mut self` 不在此 trait 中，由工厂函数 `create_position_engine` 内部调用。
/// 任务句柄在内部管理，`stop()` 负责取消并 `join()` 等待完全停止。
#[async_trait]
pub trait PositionEngineHandle: Send + Sync {
    /// 获取命令通道发送端。
    fn command_sender(&self) -> tokio::sync::mpsc::Sender<EngineCommand>;

    /// 订阅仓位事件流。
    fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<EngineEvent>;

    /// 获取事件通道发送端。
    fn event_sender(&self) -> tokio::sync::broadcast::Sender<EngineEvent>;

    /// 获取交易所引用。
    fn exchange(&self) -> std::sync::Arc<dyn ExchangePe>;

    /// 获取所有仓位。
    fn get_all_positions(&self) -> Vec<Position>;

    /// 按交易符号获取未平仓位。
    fn get_open_positions_by_symbol(&self, symbol: &str) -> Vec<Position>;

    /// 停止引擎。
    ///
    /// 异步方法：设置引擎状态为 ShuttingDown，触发 CancellationToken，
    /// 并 `join()` 主运行任务等待完全停止。
    /// 引擎任务在内部管理（无外部 TaskHandle），由 `stop()` 负责回收。
    async fn stop(&self);
}
