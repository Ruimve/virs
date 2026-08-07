use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;


/// 自动交易 Bot DB 行映射（原 virs-models::AutoBot）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AutoBot {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub symbol: String,
    pub exchange: String,
    pub paper_mode: bool,
    pub status: String,
    pub leverage: i32,
    pub max_position_pct: f64,
    pub decide_interval_secs: i32,
    pub initial_capital: f64,
    pub position_id_long: Option<Uuid>,
    pub position_id_short: Option<Uuid>,
    pub market_regime: Option<String>,
    pub ai_analysis: Option<String>,
    pub system_prompt: Option<String>,
    pub user_prompt: Option<String>,
    pub total_pnl: f64,
    pub total_trades: i32,
    pub win_trades: i32,
    pub loss_trades: i32,
    pub last_decided_at: Option<DateTime<Utc>>,
    pub strategy_file: Option<String>,
    pub auto_optimize_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
}

impl AutoBot {
    pub fn win_rate(&self) -> f64 {
        if self.total_trades <= 0 {
            0.0
        } else {
            self.win_trades as f64 / self.total_trades as f64 * 100.0
        }
    }

    pub fn loss_rate(&self) -> f64 {
        if self.total_trades <= 0 {
            0.0
        } else {
            self.loss_trades as f64 / self.total_trades as f64 * 100.0
        }
    }

    pub fn total_return_pct(&self) -> f64 {
        if self.initial_capital == 0.0 {
            0.0
        } else {
            self.total_pnl / self.initial_capital * 100.0
        }
    }

    pub fn is_running(&self) -> bool {
        self.status == "running"
    }

    pub fn is_stopped(&self) -> bool {
        self.status == "stopped"
    }
}


/// Bot 运行时配置（从 DB 行投影，不含持久化专属字段）
#[derive(Debug, Clone)]
pub struct AutoBotConfig {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub symbol: String,
    pub exchange: String,
    pub paper_mode: bool,
    pub leverage: i32,
    pub max_position_pct: f64,
    pub decide_interval_secs: i32,
    pub position_id_long: Option<Uuid>,
    pub position_id_short: Option<Uuid>,
    pub market_regime: Option<String>,
    pub ai_analysis: Option<String>,
    pub system_prompt: Option<String>,
    pub user_prompt: Option<String>,
    pub total_pnl: f64,
    pub total_trades: i32,
    pub win_trades: i32,
    pub loss_trades: i32,
    pub last_decided_at: Option<DateTime<Utc>>,
    /// 策略 prompt 文件夹名。加载时查 `strategies/auto/{strategy_file}/`。
    /// 必填项，创建 bot 时由策略选择逻辑写入。worker 缺失时报错并跳过决策。
    pub strategy_file: Option<String>,
    /// 是否启用策略自动优化
    pub auto_optimize_enabled: bool,
}

impl From<&AutoBot> for AutoBotConfig {
    fn from(bot: &AutoBot) -> Self {
        Self {
            id: bot.id,
            user_id: bot.user_id,
            name: bot.name.clone(),
            symbol: bot.symbol.clone(),
            exchange: bot.exchange.clone(),
            paper_mode: bot.paper_mode,
            leverage: bot.leverage,
            max_position_pct: bot.max_position_pct,
            decide_interval_secs: bot.decide_interval_secs,
            position_id_long: bot.position_id_long,
            position_id_short: bot.position_id_short,
            market_regime: bot.market_regime.clone(),
            ai_analysis: bot.ai_analysis.clone(),
            system_prompt: bot.system_prompt.clone(),
            user_prompt: bot.user_prompt.clone(),
            total_pnl: bot.total_pnl,
            total_trades: bot.total_trades,
            win_trades: bot.win_trades,
            loss_trades: bot.loss_trades,
            last_decided_at: bot.last_decided_at,
            strategy_file: bot.strategy_file.clone(),
            auto_optimize_enabled: bot.auto_optimize_enabled,
        }
    }
}

/// 单笔历史交易记录（用于策略评估）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    /// 策略名称
    pub strategy_name: String,
    /// 交易对
    pub symbol: String,
    /// 方向
    pub side: String,
    /// 开仓时间
    pub opened_at: DateTime<Utc>,
    /// 平仓时间
    pub closed_at: DateTime<Utc>,
    /// 入场价
    pub entry_price: f64,
    /// 出场价
    pub exit_price: f64,
    /// 数量
    pub quantity: f64,
    /// 已实现盈亏（USDT）
    pub realized_pnl: f64,
}
