

use chrono::Utc;

use super::types::StrategyMetrics;
use virs_type::{TradeHistoryProvider, TradeRecord};


pub(crate) struct StrategyEvaluator {
    history: Box<dyn TradeHistoryProvider>,
}

impl StrategyEvaluator {
    pub(crate) fn new(history: Box<dyn TradeHistoryProvider>) -> Self {
        Self { history }
    }


    pub(crate) async fn evaluate(
        &self,
        strategy_name: &str,
        window_secs: u64,
    ) -> Option<StrategyMetrics> {
        let window_end = Utc::now();
        let window_start = window_end - chrono::Duration::seconds(window_secs as i64);

        let trades = self.history.query_trades(strategy_name, window_start).await;

        if trades.is_empty() {
            return None;
        }

        let total_trades = trades.len() as u32;
        let winning: Vec<&TradeRecord> = trades.iter().filter(|t| t.realized_pnl > 0.0).collect();
        let losing: Vec<&TradeRecord> = trades.iter().filter(|t| t.realized_pnl < 0.0).collect();

        let winning_trades = winning.len() as u32;
        let losing_trades = losing.len() as u32;
        let win_rate = if total_trades > 0 {
            winning_trades as f64 / total_trades as f64
        } else {
            0.0
        };

        let total_pnl: f64 = trades.iter().map(|t| t.realized_pnl).sum();
        let avg_trade_pnl = if total_trades > 0 {
            total_pnl / total_trades as f64
        } else {
            0.0
        };


        let avg_win = if !winning.is_empty() {
            winning.iter().map(|t| t.realized_pnl).sum::<f64>() / winning.len() as f64
        } else {
            0.0
        };
        let avg_loss = if !losing.is_empty() {
            losing.iter().map(|t| t.realized_pnl.abs()).sum::<f64>() / losing.len() as f64
        } else {
            0.0
        };
        let profit_factor = if avg_loss > 0.0 {
            avg_win / avg_loss
        } else if avg_win > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };


        let max_drawdown = compute_max_drawdown(&trades);


        let avg_holding_secs = trades
            .iter()
            .map(|t| (t.closed_at - t.opened_at).num_seconds() as f64)
            .sum::<f64>()
            / total_trades as f64;

        Some(StrategyMetrics {
            strategy_name: strategy_name.to_string(),
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            total_pnl,
            max_drawdown,
            avg_holding_secs,
            avg_trade_pnl,
            profit_factor,
            window_start,
            window_end,
            evaluated_at: Utc::now(),
        })
    }
}


fn compute_max_drawdown(trades: &[TradeRecord]) -> f64 {

    let mut sorted: Vec<&TradeRecord> = trades.iter().collect();
    sorted.sort_by_key(|t| t.closed_at);

    let mut cumulative = 0.0_f64;
    let mut peak = 0.0_f64;
    let mut max_dd = 0.0_f64;

    for t in &sorted {
        cumulative += t.realized_pnl;
        if cumulative > peak {
            peak = cumulative;
        }
        let dd = peak - cumulative;
        if dd > max_dd {
            max_dd = dd;
        }
    }

    max_dd
}


