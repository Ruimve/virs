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


    async fn query_open_position(&self, symbol: &str) -> BotResult<Option<Position>> {
        Ok(self.query_open_positions(symbol).await?.into_iter().next())
    }


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


pub trait KlineEventSource: Send + Sync {
    fn subscribe_kline_events(&self) -> tokio::sync::broadcast::Receiver<KlineEvent>;
}


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
