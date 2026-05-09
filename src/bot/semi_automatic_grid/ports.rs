use async_trait::async_trait;
use uuid::Uuid;

pub use crate::trading::ports::{GridSide, GridOrderInfo, GridOrderCommand, GridOrderEvent, OrderExecutor};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnalysisLogEntry {
    pub id: Uuid,
    pub bot_id: Uuid,
    pub analysis_type: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub result: serde_json::Value,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct GridTradeRecord {
    pub grid_level: i32,
    pub side: String,
    pub quantity: f64,
    pub pnl: f64,
}

#[derive(Debug, Clone)]
pub struct GridBotConfig {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub symbol: String,
    pub exchange: String,
    pub grid_count: i32,
    pub upper_price: f64,
    pub lower_price: f64,
    pub grid_profit_pct: f64,
    pub quantity_per_grid: f64,
    pub leverage: i32,
    pub dynamic_adjust: bool,
    pub adjust_interval_secs: i32,
    pub market_regime: Option<String>,
    pub system_prompt: Option<String>,
}

#[async_trait]
pub trait PriceProvider: Send + Sync {
    async fn get_price(&self, exchange: &str, symbol: &str) -> Option<f64>;
}

#[async_trait]
pub trait GridStore: Send + Sync {
    async fn load_running_bots(&self) -> anyhow::Result<Vec<GridBotConfig>>;
    async fn load_bot(&self, bot_id: Uuid) -> anyhow::Result<Option<GridBotConfig>>;
    async fn load_trades(&self, bot_id: Uuid) -> anyhow::Result<Vec<GridTradeRecord>>;
    async fn record_trade(
        &self,
        bot_id: Uuid,
        user_id: Uuid,
        symbol: &str,
        exchange: &str,
        side: &str,
        grid_level: i32,
        price: f64,
        quantity: f64,
        pnl: f64,
        pnl_pct: f64,
    ) -> anyhow::Result<()>;
    async fn save_stats(
        &self,
        bot_id: Uuid,
        total_pnl: f64,
        total_trades: i32,
        grid_filled_count: i32,
    ) -> anyhow::Result<()>;
    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> anyhow::Result<()>;
    async fn update_last_adjusted(&self, bot_id: Uuid) -> anyhow::Result<()>;
    async fn update_grid_params(
        &self,
        bot_id: Uuid,
        upper_price: f64,
        lower_price: f64,
    ) -> anyhow::Result<()>;
    async fn update_quantity_per_grid(
        &self,
        bot_id: Uuid,
        quantity: f64,
    ) -> anyhow::Result<()>;
    async fn update_ai_analysis(
        &self,
        bot_id: Uuid,
        market_regime: &str,
        upper_price: f64,
        lower_price: f64,
        grid_count: i32,
        grid_profit_pct: f64,
        quantity_per_grid: f64,
        leverage: i32,
        ai_analysis: &str,
    ) -> anyhow::Result<()>;
    async fn save_analysis_log(
        &self,
        bot_id: Uuid,
        analysis_type: &str,
        system_prompt: &str,
        user_prompt: &str,
        result: &serde_json::Value,
        error: Option<&str>,
    ) -> anyhow::Result<()>;
    async fn load_analysis_logs(&self, bot_id: Uuid) -> anyhow::Result<Vec<AnalysisLogEntry>>;
    async fn delete_bot(&self, bot_id: Uuid) -> anyhow::Result<()>;
}

#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn load_credentials(&self, user_id: Uuid) -> anyhow::Result<Vec<(String, String)>>;
}

pub trait LlmProviderResolver: Send + Sync {
    fn is_available(&self) -> bool;
    fn resolve(
        &self,
        user_credentials: &[(String, String)],
    ) -> anyhow::Result<(String, String, String, String)>;
}

#[derive(Debug, Clone, Default)]
pub struct MarketSnapshot {
    pub current_price: f64,
    pub rsi: f64,
    pub atr: f64,
    pub atr_pct: f64,
    pub bb_width: f64,
    pub bb_upper: f64,
    pub bb_middle: f64,
    pub bb_lower: f64,
    pub ema12: f64,
    pub ema12_trend: String,
    pub ema20: f64,
    pub ema26: f64,
    pub ema26_trend: String,
    pub ema50: f64,
    pub ema_4h: f64,
    pub volatility: f64,
    pub change_1h: f64,
    pub change_4h: f64,
    pub change_24h: f64,
    pub funding_rate: f64,
    pub macd: f64,
    pub macd_signal: f64,
    pub adx: f64,
    pub price_high: f64,
    pub price_low: f64,
    pub h1_atr_sma20: f64,
    pub h1_candle_body: f64,
    pub h1_bars_outside_band: i32,
    pub h1_bandwidth_5bars_ago: f64,
    pub h1_high_20: f64,
    pub h1_low_20: f64,
    pub nearest_round_up: f64,
    pub nearest_round_down: f64,
    pub m15_current_price: f64,
    pub m15_bb_width_pct: f64,
    pub m15_atr: f64,
    pub m15_atr_sma20: f64,
    pub m15_adx: f64,
    pub m15_bars_outside_band: i32,
    pub m15_ema20: f64,
    pub m15_ema50: f64,
    pub h4_ema20: f64,
    pub h4_ema50: f64,
    pub h4_adx: f64,
    pub h4_bb_width_pct: f64,
}

#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    async fn get_market_snapshot(&self, exchange: &str, symbol: &str) -> MarketSnapshot;
    async fn get_account_balance(&self, exchange: &str) -> f64;
}
