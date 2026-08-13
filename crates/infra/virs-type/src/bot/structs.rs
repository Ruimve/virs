use crate::order::Side;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;


#[derive(Debug, Clone)]
pub struct OrderInfo {
    pub id: uuid::Uuid,
    pub position_id: Option<uuid::Uuid>,
    pub symbol: String,
    pub side: Side,
    pub fill_price: Option<f64>,
    pub request_price: Option<f64>,
    pub filled: f64,
    pub client_order_id: Option<String>,

    pub fee: f64,
}


/* 市场快照：包含当前价格、资金费率、最小交易量和指标 JSON，用于策略决策 */
#[derive(Debug, Clone)]
pub struct MarketSnapshot {
    pub current_price: f64,
    pub funding_rate: f64,
    pub funding_next_time: String,
    pub min_qty: f64,

    /* 指标 JSON：技术指标的序列化结果，结构由策略层定义 */
    pub indicators_json: serde_json::Value,
}


#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Bot {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub symbol: String,
    pub exchange: String,
    pub paper_mode: bool,
    pub status: String,
    pub bot_type: String,
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

impl Bot {
    /* 胜率计算：盈利交易数 / 总交易数 * 100，无交易时返回 0 */
    pub fn win_rate(&self) -> f64 {
        if self.total_trades <= 0 {
            0.0
        } else {
            self.win_trades as f64 / self.total_trades as f64 * 100.0
        }
    }

    /* 亏损率计算：亏损交易数 / 总交易数 * 100，无交易时返回 0 */
    pub fn loss_rate(&self) -> f64 {
        if self.total_trades <= 0 {
            0.0
        } else {
            self.loss_trades as f64 / self.total_trades as f64 * 100.0
        }
    }

    /* 总收益率计算：总盈亏 / 初始资金 * 100，初始资金为 0 时返回 0 */
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


#[derive(Debug, Clone)]
pub struct BotConfig {
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

    pub strategy_file: Option<String>,

    pub auto_optimize_enabled: bool,
}

/* 从 Bot 转换为 BotConfig：提取运行时所需的配置字段，去掉时间戳等管理字段 */
impl From<&Bot> for BotConfig {
    fn from(bot: &Bot) -> Self {
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


/* 交易记录：用于策略回测和历史交易统计，记录开仓/平仓价格和已实现盈亏 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {

    pub strategy_name: String,

    pub symbol: String,

    pub side: String,

    pub opened_at: DateTime<Utc>,

    pub closed_at: DateTime<Utc>,

    pub entry_price: f64,

    pub exit_price: f64,

    pub quantity: f64,

    pub realized_pnl: f64,
}
