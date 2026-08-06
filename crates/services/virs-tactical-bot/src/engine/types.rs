//! 策略引擎类型定义。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 单个策略的绩效指标。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyMetrics {
    /// 策略名称
    pub strategy_name: String,
    /// 总交易笔数
    pub total_trades: u32,
    /// 盈利交易笔数
    pub winning_trades: u32,
    /// 亏损交易笔数
    pub losing_trades: u32,
    /// 胜率（0.0 ~ 1.0）
    pub win_rate: f64,
    /// 累计已实现盈亏（USDT）
    pub total_pnl: f64,
    /// 最大回撤（USDT，正数）
    pub max_drawdown: f64,
    /// 平均持仓时长（秒）
    pub avg_holding_secs: f64,
    /// 平均每笔盈亏（USDT）
    pub avg_trade_pnl: f64,
    /// 盈亏比（平均盈利 / 平均亏损）
    pub profit_factor: f64,
    /// 评估时间窗口起点
    pub window_start: DateTime<Utc>,
    /// 评估时间窗口终点
    pub window_end: DateTime<Utc>,
    /// 评估时间
    pub evaluated_at: DateTime<Utc>,
}

impl StrategyMetrics {
    /// 综合评分：胜率 × 0.3 + 盈亏比 × 0.3 + 归一化累计 P&L × 0.4
    /// 用于决定是否需要优化。
    pub fn composite_score(&self) -> f64 {
        let win_score = self.win_rate * 0.3;
        let pf_score = if self.profit_factor.is_finite() && self.profit_factor > 0.0 {
            (self.profit_factor / (1.0 + self.profit_factor)).min(1.0) * 0.3
        } else {
            0.0
        };
        let pnl_score = if self.total_pnl > 0.0 {
            (self.total_pnl / (1.0 + self.total_pnl.abs())).max(0.0) * 0.4
        } else {
            0.0
        };
        win_score + pf_score + pnl_score
    }

    /// 是否需要优化：综合评分低于阈值且交易数足够。
    pub fn needs_optimization(&self, min_trades: u32, score_threshold: f64) -> bool {
        self.total_trades >= min_trades && self.composite_score() < score_threshold
    }
}

/// 策略热切换通知。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyUpdate {
    /// 策略名称
    pub strategy_name: String,
    /// 旧版本号
    pub old_version: i32,
    /// 新版本号
    pub new_version: i32,
    /// 触发优化的评估指标
    pub metrics: StrategyMetrics,
    /// 优化时间
    pub optimized_at: DateTime<Utc>,
}

/// 策略引擎配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyEngineConfig {
    /// 分析周期（秒）
    pub analysis_interval_secs: u64,
    /// 最少交易笔数（低于此数不优化）
    pub min_trades_for_optimization: u32,
    /// 综合评分阈值（低于此值触发优化）
    pub optimization_score_threshold: f64,
    /// 评估回看窗口（秒）
    pub evaluation_window_secs: u64,
    /// 最大版本号（防止无限优化）
    pub max_version: i32,
    /// LLM API key
    pub llm_api_key: String,
    /// LLM base URL
    pub llm_base_url: String,
    /// LLM 模型名
    pub llm_model: String,
}

impl Default for StrategyEngineConfig {
    fn default() -> Self {
        Self {
            analysis_interval_secs: 3600,       // 1 小时
            min_trades_for_optimization: 10,
            optimization_score_threshold: 0.45,
            evaluation_window_secs: 86400 * 7,  // 7 天
            max_version: 20,
            llm_api_key: String::new(),
            llm_base_url: "https://api.openai.com/v1".to_string(),
            llm_model: "gpt-4o".to_string(),
        }
    }
}

/// 单笔历史交易记录（用于评估）。
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
