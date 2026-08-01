use async_trait::async_trait;
use uuid::Uuid;

use virs_error::{BotResult, VirsResult};

use crate::market::Balance;
use crate::position::Position;

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
