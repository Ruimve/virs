use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use virs_error::VirsResult;


/// Grid 交易记录 — 从 pe_order_latest + pe_grid_order_context 联查派生
#[derive(Debug, Clone)]
pub struct GridTradeRecord {
    pub open_client_order_id: String,
    pub close_client_order_id: Option<String>,
    pub grid_level: i32,
    pub open_side: String,
    pub open_price: f64,
    pub open_quantity: f64,
    pub close_side: Option<String>,
    pub close_price: Option<f64>,
    pub close_quantity: Option<f64>,
    pub pnl: f64,
    pub opened_at: DateTime<Utc>,
}


#[derive(Debug, Clone)]
pub struct GridBotConfig {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub symbol: String,
    pub exchange: String,
    pub paper_mode: bool,
    pub grid_count: i32,
    pub upper_price: f64,
    pub lower_price: f64,
    pub grid_profit_pct: f64,
    pub quantity_per_grid: f64,
    pub leverage: i32,
    pub dynamic_adjust: bool,
    pub adjust_interval_secs: i32,
    pub market_regime: Option<String>,
    pub grid_levels_json: Option<serde_json::Value>,
    pub system_prompt: Option<String>,
    pub last_adjusted_at: Option<DateTime<Utc>>,
    /// 策略 prompt 文件夹名。加载时查 `strategies/grid/{strategy_file}/`。
    /// 必填项，创建 bot 时由策略选择逻辑写入。worker 缺失时报错并跳过决策。
    pub strategy_file: Option<String>,
}


#[async_trait]
pub trait GridStore: Send + Sync {
    async fn load_running_bots(&self) -> VirsResult<Vec<GridBotConfig>>;
    async fn load_bot(&self, bot_id: Uuid) -> VirsResult<Option<GridBotConfig>>;

    /// 加载交易记录 — JOIN pe_order_latest + pe_grid_order_context
    async fn load_trades(&self, bot_id: Uuid) -> VirsResult<Vec<GridTradeRecord>>;

    /// 记录开仓 context — INSERT pe_grid_order_context (order_role='open', status='open')
    async fn record_open_trade(
        &self,
        bot_id: Uuid,
        user_id: Uuid,
        symbol: &str,
        exchange: &str,
        grid_level: i32,
        client_order_id: &str,
        strategy_file: &Option<String>,
    ) -> VirsResult<()>;

    /// 记录平仓 context — UPDATE open row status='closed' + INSERT close row
    async fn close_trade(
        &self,
        open_client_order_id: &str,
        close_client_order_id: &str,
    ) -> VirsResult<()>;

    /// 查找 open 状态的开仓 client_order_id
    async fn find_open_trade(
        &self,
        bot_id: Uuid,
        grid_level: i32,
    ) -> VirsResult<Option<String>>;

    /// 记录孤儿平仓 — INSERT close context row, status='orphaned'
    async fn record_orphaned_close_trade(
        &self,
        bot_id: Uuid,
        user_id: Uuid,
        symbol: &str,
        exchange: &str,
        grid_level: i32,
        close_client_order_id: &str,
        strategy_file: &Option<String>,
    ) -> VirsResult<()>;

    async fn save_stats(
        &self,
        bot_id: Uuid,
        total_pnl: f64,
        unrealized_pnl: f64,
        total_trades: i32,
        grid_filled_count: i32,
        levels_json: Option<&serde_json::Value>,
    ) -> VirsResult<()>;
    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> VirsResult<()>;
    async fn update_last_adjusted(&self, bot_id: Uuid) -> VirsResult<()>;
    async fn update_grid_params(
        &self,
        bot_id: Uuid,
        upper_price: f64,
        lower_price: f64,
    ) -> VirsResult<()>;
    async fn update_quantity_per_grid(&self, bot_id: Uuid, quantity: f64) -> VirsResult<()>;
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
    ) -> VirsResult<()>;
    async fn save_analysis_log(
        &self,
        bot_id: Uuid,
        analysis_type: &str,
        system_prompt: &str,
        user_prompt: &str,
        result: &serde_json::Value,
        error: Option<&str>,
        llm_model: &str,
        strategy_file: &Option<String>,
    ) -> VirsResult<()>;
    async fn delete_bot(&self, bot_id: Uuid) -> VirsResult<()>;
}
