//! 策略评估器：从历史交易记录计算绩效指标。

use async_trait::async_trait;
use chrono::Utc;

use super::types::{StrategyMetrics, TradeRecord};

/// 交易历史数据源 trait。
/// 由应用层实现（从数据库查询已平仓交易记录）。
#[async_trait]
pub trait TradeHistoryProvider: Send + Sync {
    /// 查询指定策略在时间窗口内的已平仓交易记录。
    async fn query_trades(
        &self,
        strategy_name: &str,
        since: chrono::DateTime<chrono::Utc>,
    ) -> Vec<TradeRecord>;
}

/// 策略评估器。
pub struct StrategyEvaluator {
    history: Box<dyn TradeHistoryProvider>,
}

impl StrategyEvaluator {
    pub fn new(history: Box<dyn TradeHistoryProvider>) -> Self {
        Self { history }
    }

    /// 评估单个策略的绩效。
    pub async fn evaluate(
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

        // 盈亏比 = 平均盈利 / 平均亏损绝对值
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

        // 最大回撤：累计 P&L 曲线的最大峰值到谷值跌幅
        let max_drawdown = compute_max_drawdown(&trades);

        // 平均持仓时长
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

/// 计算累计 P&L 曲线的最大回撤。
fn compute_max_drawdown(trades: &[TradeRecord]) -> f64 {
    // 按平仓时间排序
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    struct MockHistory {
        trades: Vec<TradeRecord>,
    }

    #[async_trait]
    impl TradeHistoryProvider for MockHistory {
        async fn query_trades(
            &self,
            strategy_name: &str,
            _since: chrono::DateTime<chrono::Utc>,
        ) -> Vec<TradeRecord> {
            self.trades
                .iter()
                .filter(|t| t.strategy_name == strategy_name)
                .cloned()
                .collect()
        }
    }

    fn make_trade(pnl: f64, mins_ago: i64) -> TradeRecord {
        let now = Utc::now();
        TradeRecord {
            strategy_name: "test".into(),
            symbol: "BTC/USDT".into(),
            side: "long".into(),
            opened_at: now - chrono::Duration::minutes(mins_ago + 10),
            closed_at: now - chrono::Duration::minutes(mins_ago),
            entry_price: 50000.0,
            exit_price: 50000.0 + pnl,
            quantity: 0.1,
            realized_pnl: pnl,
        }
    }

    #[tokio::test]
    async fn t_eval_1_basic_metrics() {
        let trades = vec![
            make_trade(100.0, 60),
            make_trade(-50.0, 50),
            make_trade(80.0, 40),
            make_trade(-30.0, 30),
        ];
        let evaluator = StrategyEvaluator::new(Box::new(MockHistory { trades }));
        let metrics = evaluator.evaluate("test", 3600).await.unwrap();

        assert_eq!(metrics.total_trades, 4);
        assert_eq!(metrics.winning_trades, 2);
        assert_eq!(metrics.losing_trades, 2);
        assert_eq!(metrics.win_rate, 0.5);
        assert_eq!(metrics.total_pnl, 100.0);
    }

    #[tokio::test]
    async fn t_eval_2_profit_factor() {
        let trades = vec![
            make_trade(100.0, 60),
            make_trade(-50.0, 50),
        ];
        let evaluator = StrategyEvaluator::new(Box::new(MockHistory { trades }));
        let metrics = evaluator.evaluate("test", 3600).await.unwrap();

        // avg_win=100, avg_loss=50, pf=2.0
        assert!((metrics.profit_factor - 2.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn t_eval_3_no_trades_returns_none() {
        let evaluator = StrategyEvaluator::new(Box::new(MockHistory { trades: vec![] }));
        let result = evaluator.evaluate("nonexistent", 3600).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn t_eval_4_max_drawdown() {
        // 累计 P&L: +100, +50, -200, +30 → peak=150, trough=-50, dd=200
        let trades = vec![
            make_trade(100.0, 60),
            make_trade(50.0, 50),
            make_trade(-200.0, 40),
            make_trade(30.0, 30),
        ];
        let evaluator = StrategyEvaluator::new(Box::new(MockHistory { trades }));
        let metrics = evaluator.evaluate("test", 3600).await.unwrap();
        assert!((metrics.max_drawdown - 200.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn t_eval_5_composite_score() {
        let metrics = StrategyMetrics {
            strategy_name: "test".into(),
            total_trades: 10,
            winning_trades: 6,
            losing_trades: 4,
            win_rate: 0.6,
            total_pnl: 200.0,
            max_drawdown: 50.0,
            avg_holding_secs: 3600.0,
            avg_trade_pnl: 20.0,
            profit_factor: 2.0,
            window_start: Utc::now(),
            window_end: Utc::now(),
            evaluated_at: Utc::now(),
        };
        let score = metrics.composite_score();
        assert!(score > 0.0 && score <= 1.0, "score should be in (0, 1], got {}", score);
    }

    #[tokio::test]
    async fn t_eval_6_needs_optimization() {
        let good = StrategyMetrics {
            strategy_name: "good".into(),
            total_trades: 20,
            winning_trades: 14,
            losing_trades: 6,
            win_rate: 0.7,
            total_pnl: 500.0,
            max_drawdown: 30.0,
            avg_holding_secs: 3600.0,
            avg_trade_pnl: 25.0,
            profit_factor: 3.0,
            window_start: Utc::now(),
            window_end: Utc::now(),
            evaluated_at: Utc::now(),
        };
        assert!(!good.needs_optimization(10, 0.45), "good strategy should not need optimization");

        let bad = StrategyMetrics {
            strategy_name: "bad".into(),
            total_trades: 15,
            winning_trades: 3,
            losing_trades: 12,
            win_rate: 0.2,
            total_pnl: -300.0,
            max_drawdown: 150.0,
            avg_holding_secs: 3600.0,
            avg_trade_pnl: -20.0,
            profit_factor: 0.3,
            window_start: Utc::now(),
            window_end: Utc::now(),
            evaluated_at: Utc::now(),
        };
        assert!(bad.needs_optimization(10, 0.45), "bad strategy should need optimization");
    }
}
