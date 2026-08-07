//! 策略引擎类型定义。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 单个策略的绩效指标。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StrategyMetrics {
    /// 策略名称
    pub(crate) strategy_name: String,
    /// 总交易笔数
    pub(crate) total_trades: u32,
    /// 盈利交易笔数
    pub(crate) winning_trades: u32,
    /// 亏损交易笔数
    pub(crate) losing_trades: u32,
    /// 胜率（0.0 ~ 1.0）
    pub(crate) win_rate: f64,
    /// 累计已实现盈亏（USDT）
    pub(crate) total_pnl: f64,
    /// 最大回撤（USDT，正数）
    pub(crate) max_drawdown: f64,
    /// 平均持仓时长（秒）
    pub(crate) avg_holding_secs: f64,
    /// 平均每笔盈亏（USDT）
    pub(crate) avg_trade_pnl: f64,
    /// 盈亏比（平均盈利 / 平均亏损）
    pub(crate) profit_factor: f64,
    /// 评估时间窗口起点
    pub(crate) window_start: DateTime<Utc>,
    /// 评估时间窗口终点
    pub(crate) window_end: DateTime<Utc>,
    /// 评估时间
    pub(crate) evaluated_at: DateTime<Utc>,
}

impl StrategyMetrics {
    /// 综合评分：胜率 × 0.3 + 盈亏比 × 0.3 + 归一化累计 P&L × 0.4
    /// 用于决定是否需要优化。
    pub(crate) fn composite_score(&self) -> f64 {
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
    pub(crate) fn needs_optimization(&self, min_trades: u32, score_threshold: f64) -> bool {
        self.total_trades >= min_trades && self.composite_score() < score_threshold
    }
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
