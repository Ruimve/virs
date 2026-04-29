//! 端口定义（Ports & Adapters 模式的 Ports 层）
//!
//! 定义网格机器人对外部依赖的抽象接口，实现业务隔离。
//! 所有 trait 由外部模块（main.rs 或 adapter 层）实现并注入。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── 本地 DTO（替代 crate::engine::position::types 中的类型）──

/// 订单方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GridSide {
    Buy,
    Sell,
}

impl GridSide {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }
}

/// 网格内部使用的订单信息（从外部事件转换而来）
#[derive(Debug, Clone)]
pub struct GridOrderInfo {
    pub id: Uuid,
    pub side: GridSide,
    pub fill_price: Option<f64>,
    pub request_price: Option<f64>,
    pub filled: f64,
}

/// 网格发送给外部的订单命令
#[derive(Debug, Clone)]
pub enum GridOrderCommand {
    PlaceOrder {
        symbol: String,
        side: GridSide,
        amount: f64,
        price: Option<f64>,
        reduce_only: bool,
    },
    CancelAllOrders {
        symbol: Option<String>,
    },
}

/// 外部事件（从 Position Engine 事件转换而来）
#[derive(Debug, Clone)]
pub enum GridOrderEvent {
    OrderPlaced { order: GridOrderInfo },
    OrderFilled { order: GridOrderInfo },
    OrderCanceled { order_id: Uuid },
    OrderFailed { order_id: Uuid, reason: String },
    RiskAlert { level: String, message: String },
    LiquidationWarning {
        symbol: String,
        liquidation_price: f64,
        current_price: f64,
    },
}

/// 网格历史成交记录
#[derive(Debug, Clone)]
pub struct GridTradeRecord {
    pub grid_level: i32,
    pub side: String,
    pub quantity: f64,
    pub pnl: f64,
}

/// 网格 Bot 配置（从外部注入，替代直接访问 models::GridBot）
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
    pub dynamic_adjust: bool,
    pub adjust_interval_secs: i32,
    pub market_regime: Option<String>,
    pub system_prompt: Option<String>,
}

// ── Trait 定义 ──

/// 价格提供者（替代直接依赖 StrategyEngine）
#[async_trait]
pub trait PriceProvider: Send + Sync {
    async fn get_price(&self, exchange: &str, symbol: &str) -> Option<f64>;
}

/// 订单执行器（替代直接发送 EngineCommand）
#[async_trait]
pub trait OrderExecutor: Send + Sync {
    async fn send_command(&self, command: GridOrderCommand) -> anyhow::Result<()>;
}

/// 网格数据存储（替代直接 SQL 查询）
#[async_trait]
pub trait GridStore: Send + Sync {
    /// 加载运行中的 bot 列表
    async fn load_running_bots(&self) -> anyhow::Result<Vec<GridBotConfig>>;
    /// 按 ID 加载 bot
    async fn load_bot(&self, bot_id: Uuid) -> anyhow::Result<Option<GridBotConfig>>;
    /// 加载历史成交
    async fn load_trades(&self, bot_id: Uuid) -> anyhow::Result<Vec<GridTradeRecord>>;
    /// 记录成交
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
    /// 保存统计
    async fn save_stats(
        &self,
        bot_id: Uuid,
        total_pnl: f64,
        total_trades: i32,
        grid_filled_count: i32,
    ) -> anyhow::Result<()>;
    /// 更新 bot 状态
    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> anyhow::Result<()>;
    /// 更新调整时间
    async fn update_last_adjusted(&self, bot_id: Uuid) -> anyhow::Result<()>;
    /// 更新网格参数
    async fn update_grid_params(
        &self,
        bot_id: Uuid,
        upper_price: f64,
        lower_price: f64,
    ) -> anyhow::Result<()>;
    /// 更新每层数量
    async fn update_quantity_per_grid(
        &self,
        bot_id: Uuid,
        quantity: f64,
    ) -> anyhow::Result<()>;
    /// 删除 bot
    async fn delete_bot(&self, bot_id: Uuid) -> anyhow::Result<()>;
}

/// LLM 凭证存储（替代直接查询 qd_ai_credentials + crypto::decrypt）
#[async_trait]
pub trait CredentialStore: Send + Sync {
    /// 加载用户的 AI 凭证，返回 (provider_name, api_key)
    async fn load_credentials(&self, user_id: Uuid) -> anyhow::Result<Vec<(String, String)>>;
}

/// LLM 提供者解析（替代直接依赖 AiService）
pub trait LlmProviderResolver: Send + Sync {
    /// 检查是否有可用 provider
    fn is_available(&self) -> bool;
    /// 解析为 (api_key, base_url, model)
    fn resolve(
        &self,
        user_credentials: &[(String, String)],
    ) -> anyhow::Result<(String, String, String, String)>;
}

