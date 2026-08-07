

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StrategyMetrics {

    pub(crate) strategy_name: String,

    pub(crate) total_trades: u32,

    pub(crate) winning_trades: u32,

    pub(crate) losing_trades: u32,

    pub(crate) win_rate: f64,

    pub(crate) total_pnl: f64,

    pub(crate) max_drawdown: f64,

    pub(crate) avg_holding_secs: f64,

    pub(crate) avg_trade_pnl: f64,

    pub(crate) profit_factor: f64,

    pub(crate) window_start: DateTime<Utc>,

    pub(crate) window_end: DateTime<Utc>,

    pub(crate) evaluated_at: DateTime<Utc>,
}

impl StrategyMetrics {


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


    pub(crate) fn needs_optimization(&self, min_trades: u32, score_threshold: f64) -> bool {
        self.total_trades >= min_trades && self.composite_score() < score_threshold
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyEngineConfig {

    pub analysis_interval_secs: u64,

    pub min_trades_for_optimization: u32,

    pub optimization_score_threshold: f64,

    pub evaluation_window_secs: u64,

    pub max_version: i32,

    pub llm_api_key: String,

    pub llm_base_url: String,

    pub llm_model: String,
}

impl Default for StrategyEngineConfig {
    fn default() -> Self {
        Self {
            analysis_interval_secs: 3600,
            min_trades_for_optimization: 10,
            optimization_score_threshold: 0.45,
            evaluation_window_secs: 86400 * 7,
            max_version: 20,
            llm_api_key: String::new(),
            llm_base_url: "https://api.openai.com/v1".to_string(),
            llm_model: "gpt-4o".to_string(),
        }
    }
}
