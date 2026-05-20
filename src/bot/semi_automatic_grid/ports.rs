use async_trait::async_trait;
use uuid::Uuid;

pub use crate::trading::ports::{OrderSide, OrderInfo, OrderCommand, OrderEvent, OrderExecutor, PositionSide};

/** AI 分析日志持久化记录 */
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnalysisLogEntry {
    pub id: Uuid,
    pub bot_id: Uuid,
/** 分析类型："initial" 或 "periodic" */
    pub analysis_type: String,
    pub system_prompt: String,
    pub user_prompt: String,
/** LLM 返回的 JSON 结果 */
    pub result: serde_json::Value,
/** 错误信息 */
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/** 网格交易记录

每次成交后记录的流水，用于 PnL 统计和历史回溯 */
#[derive(Debug, Clone)]
pub struct GridTradeRecord {
/** 网格层级编号 */
    pub grid_level: i32,
/** 成交方向："buy" 或 "sell" */
    pub side: String,
/** 成交价格 */
    pub price: f64,
/** 成交数量 */
    pub quantity: f64,
/** 本次成交已实现盈亏 */
    pub pnl: f64,
}

/** 网格 Bot 配置

从数据库加载的 bot 参数，包含网格结构、交易参数和 AI 配置 */
#[derive(Debug, Clone)]
pub struct GridBotConfig {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
/** 交易对（如 "BTC/USDT"） */
    pub symbol: String,
/** 交易所名称 */
    pub exchange: String,
/** 网格层数 */
    pub grid_count: i32,
/** 网格上界价格 */
    pub upper_price: f64,
/** 网格下界价格 */
    pub lower_price: f64,
/** 每格利润百分比 */
    pub grid_profit_pct: f64,
/** 每格投入金额（USDT） */
    pub quantity_per_grid: f64,
/** 杠杆倍数 */
    pub leverage: i32,
/** 是否启用 AI 动态调整 */
    pub dynamic_adjust: bool,
/** AI 调整间隔（秒） */
    pub adjust_interval_secs: i32,
/** AI 判定的市场状态 */
    pub market_regime: Option<String>,
/** LLM 返回的层级配置 JSON */
    pub grid_levels_json: Option<serde_json::Value>,
/** 自定义系统提示词 */
    pub system_prompt: Option<String>,
    pub last_adjusted_at: Option<chrono::DateTime<chrono::Utc>>,
}

/** 价格提供者端口

获取指定交易对的当前价格 */
#[async_trait]
pub trait PriceProvider: Send + Sync {
    async fn get_price(&self, exchange: &str, symbol: &str) -> Option<f64>;
}

/** 网格数据存储端口

定义所有与数据库交互的方法，包括 bot 配置、交易记录、统计和 AI 分析日志 */
#[async_trait]
pub trait GridStore: Send + Sync {
/** 加载所有运行中的 bot 配置 */
    async fn load_running_bots(&self) -> anyhow::Result<Vec<GridBotConfig>>;
/** 按 ID 加载单个 bot 配置 */
    async fn load_bot(&self, bot_id: Uuid) -> anyhow::Result<Option<GridBotConfig>>;
/** 加载指定 bot 的历史交易记录 */
    async fn load_trades(&self, bot_id: Uuid) -> anyhow::Result<Vec<GridTradeRecord>>;
/** 记录一笔交易 */
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
/** 保存运行统计（已实现 PnL、未实现 PnL、成交次数等） */
    async fn save_stats(
        &self,
        bot_id: Uuid,
        total_pnl: f64,
        unrealized_pnl: f64,
        total_trades: i32,
        grid_filled_count: i32,
        levels_json: Option<&serde_json::Value>,
    ) -> anyhow::Result<()>;
/** 更新 bot 运行状态 */
    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> anyhow::Result<()>;
/** 更新最后调整时间 */
    async fn update_last_adjusted(&self, bot_id: Uuid) -> anyhow::Result<()>;
/** 更新网格上下界价格 */
    async fn update_grid_params(
        &self,
        bot_id: Uuid,
        upper_price: f64,
        lower_price: f64,
    ) -> anyhow::Result<()>;
/** 更新每格投入金额 */
    async fn update_quantity_per_grid(
        &self,
        bot_id: Uuid,
        quantity: f64,
    ) -> anyhow::Result<()>;
/** 保存 AI 分析结果并更新 bot 参数 */
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
        grid_levels_json: Option<&serde_json::Value>,
    ) -> anyhow::Result<()>;
/** 保存 AI 分析日志 */
    async fn save_analysis_log(
        &self,
        bot_id: Uuid,
        analysis_type: &str,
        system_prompt: &str,
        user_prompt: &str,
        result: &serde_json::Value,
        error: Option<&str>,
    ) -> anyhow::Result<()>;
/** 加载 AI 分析日志 */
    async fn load_analysis_logs(&self, bot_id: Uuid) -> anyhow::Result<Vec<AnalysisLogEntry>>;
/** 删除 bot 及其关联数据 */
    async fn delete_bot(&self, bot_id: Uuid) -> anyhow::Result<()>;
}

/** 用户凭证存储端口

加载用户自定义的 LLM API 凭证 */
#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn load_credentials(&self, user_id: Uuid) -> anyhow::Result<Vec<(String, String)>>;
}

/** LLM 提供商解析器端口

根据用户凭证解析出 API key、base URL、模型名称和提供商标识 */
pub trait LlmProviderResolver: Send + Sync {
/** 检查默认 LLM 服务是否可用 */
    fn is_available(&self) -> bool;
/** 从用户凭证中解析 LLM 连接参数 */
    fn resolve(
        &self,
        user_credentials: &[(String, String)],
    ) -> anyhow::Result<(String, String, String, String)>;
}

/** 市场快照

包含当前价格和多周期技术指标数据，由 adapters 层从交易所获取 K 线后计算 */
#[derive(Debug, Clone, Default)]
pub struct MarketSnapshot {
/** 当前价格（独立于指标，用于快速判断数据可用性） */
    pub current_price: f64,
/** 资金费率（独立于指标，用于 PromptContext 直接引用） */
    pub funding_rate: f64,
/** 多周期技术指标（由 utils::compute_market_indicators 计算） */
    pub indicators: crate::bot::semi_automatic_grid::utils::indicators::MarketIndicators,
}

/** 账户余额信息 */
#[derive(Debug, Clone, Default)]
pub struct AccountBalance {
/** 总资产（可用 + 占用） */
    pub total: f64,
/** 可用余额 */
    pub free: f64,
/** 已用保证金 */
    pub used: f64,
}

/** 市场数据提供者端口

获取市场快照和账户余额 */
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
/** 获取指定交易对的市场快照（含多周期技术指标） */
    async fn get_market_snapshot(&self, exchange: &str, symbol: &str) -> MarketSnapshot;
/** 获取指定交易所的账户余额 */
    async fn get_account_balance(&self, exchange: &str) -> AccountBalance;
}
