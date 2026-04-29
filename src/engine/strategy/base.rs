//! Base trait for market-specific strategy engines.

use async_trait::async_trait;
use crate::models::*;
use crate::exchange::Exchange;
use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Trait that each market-specific engine (Spot, Perpetual) must implement.
/// The dispatch layer (`StrategyEngine`) delegates to the appropriate engine
/// based on the strategy's market type.
#[async_trait]
pub trait MarketEngine: Send + Sync {
    /// Return the market type this engine handles.
    fn market_type(&self) -> MarketType;

    /// Return all running strategy IDs in this engine.
    fn running_strategy_ids(&self) -> Vec<Uuid>;

    /// Check if a strategy is currently running in this engine.
    fn is_running(&self, strategy_id: &Uuid) -> bool;

    /// Start a strategy in this engine.
    /// The exchange is looked up from the shared exchange_instances map by exchange_key.
    async fn start_strategy(
        &self,
        strategy: Strategy,
        exchange_key: String,
        exchange_instances: Arc<DashMap<String, Box<dyn Exchange>>>,
    ) -> anyhow::Result<()>;

    /// Stop a running strategy in this engine.
    fn stop_strategy(&self, strategy_id: &Uuid) -> bool;
}
