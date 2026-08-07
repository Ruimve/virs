use async_trait::async_trait;
use uuid::Uuid;

use virs_error::{BotResult, VirsResult};

use crate::exchange::{ExchangePe, MarketType};
use crate::market::{Balance, Candle, KlineEvent, Timeframe};
use crate::position::{EngineCommand, EngineEvent, Position};
use crate::ws_types::OrderBookEvent;

use super::structs::MarketSnapshot;


/* 订单执行器 trait：定义发送交易命令和查询持仓的接口 */
#[async_trait]
pub trait OrderExecutor: Send + Sync {
    async fn send_command(&self, command: super::enums::OrderCommand) -> BotResult<()>;

    /* 查询单个交易对的开放持仓，默认实现从列表中取第一个 */
    async fn query_open_position(&self, symbol: &str) -> BotResult<Option<Position>> {
        Ok(self.query_open_positions(symbol).await?.into_iter().next())
    }

    async fn query_open_positions(&self, symbol: &str) -> BotResult<Vec<Position>>;
}


/* 凭证存储 trait：定义加载用户 API 凭证的接口 */
#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn load_credentials(
        &self,
        user_id: Uuid,
    ) -> BotResult<Vec<(String, String, Option<String>)>>;
}


/* 行情数据提供者 trait：定义获取市场快照和账户余额的接口 */
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    async fn get_market_snapshot(
        &self,
        exchange: &str,
        symbol: &str,
    ) -> VirsResult<MarketSnapshot>;
    async fn get_account_balance(&self, exchange: &str) -> VirsResult<Balance>;
}


/* LLM 提供者解析器 trait：根据用户凭证解析出 LLM 服务配置 */
pub trait LlmProviderResolver: Send + Sync {
    fn is_available(&self) -> bool;
    fn resolve(
        &self,
        user_credentials: &[(String, String, Option<String>)],
    ) -> BotResult<(String, String, String, String)>;
}


/* K 线事件源 trait：提供 K 线事件的广播订阅能力 */
pub trait KlineEventSource: Send + Sync {
    fn subscribe_kline_events(&self) -> tokio::sync::broadcast::Receiver<KlineEvent>;
}


/* K 线引擎句柄 trait：提供市场订阅、K 线查询和引擎停止能力 */
#[async_trait]
pub trait KlineEngineHandle: KlineEventSource {

    async fn subscribe_market(
        &self,
        exchange: &str,
        symbol: &str,
        market_type: MarketType,
    ) -> VirsResult<()>;

    async fn stop(&self);

    async fn get_klines(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: Timeframe,
    ) -> Option<Vec<Candle>>;
}


/* 订单簿引擎句柄 trait：提供订单簿事件订阅和市场订阅能力 */
#[async_trait]
pub trait OrderBookEngineHandle: Send + Sync {

    fn subscribe_orderbook_events(&self) -> tokio::sync::broadcast::Receiver<OrderBookEvent>;

    async fn subscribe_market(
        &self,
        exchange: &str,
        symbol: &str,
        market_type: MarketType,
    ) -> VirsResult<()>;

    async fn stop(&self);
}


/* 持仓引擎句柄 trait：提供命令发送、事件订阅、持仓查询和引擎停止能力 */
#[async_trait]
pub trait PositionEngineHandle: Send + Sync {

    fn command_sender(&self) -> tokio::sync::mpsc::Sender<EngineCommand>;

    fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<EngineEvent>;

    fn event_sender(&self) -> tokio::sync::broadcast::Sender<EngineEvent>;

    fn exchange(&self) -> std::sync::Arc<dyn ExchangePe>;

    fn get_all_positions(&self) -> Vec<Position>;

    fn get_open_positions_by_symbol(&self, symbol: &str) -> Vec<Position>;

    async fn stop(&self);
}
