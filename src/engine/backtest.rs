use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::engine::indicators;
use crate::engine::map_raw_signal;
use crate::models::*;

/// High-performance backtesting engine.
/// Processes historical kline data and simulates strategy execution.
pub struct BacktestEngine {
    initial_balance: f64,
    commission_rate: f64,
    slippage: f64,
}

fn record_trade(
    trades: &mut Vec<BacktestTrade>,
    bt: BacktestTrade,
    profit_trades: &mut i64,
    loss_trades: &mut i64,
    total_profit: &mut f64,
    total_loss: &mut f64,
    current_consecutive_wins: &mut i64,
    current_consecutive_losses: &mut i64,
    max_consecutive_wins: &mut i64,
    max_consecutive_losses: &mut i64,
) {
    if bt.pnl >= 0.0 {
        *profit_trades += 1;
        *total_profit += bt.pnl;
        *current_consecutive_wins += 1;
        *current_consecutive_losses = 0;
        *max_consecutive_wins = (*max_consecutive_wins).max(*current_consecutive_wins);
    } else {
        *loss_trades += 1;
        *total_loss += bt.pnl.abs();
        *current_consecutive_losses += 1;
        *current_consecutive_wins = 0;
        *max_consecutive_losses = (*max_consecutive_losses).max(*current_consecutive_losses);
    }
    trades.push(bt);
}

impl BacktestEngine {
    pub fn new(initial_balance: f64, commission_rate: f64, slippage: f64) -> Self {
        Self {
            initial_balance,
            commission_rate,
            slippage,
        }
    }

    /// Run a backtest with the given klines and signal generator.
    /// The signal generator is a function that takes klines up to index i
    /// and returns a signal (1 = buy, -1 = sell, 0 = hold).
    pub fn run<F>(
        &self,
        klines: &[Kline],
        mut signal_fn: F,
        stop_loss_pct: Option<f64>,
        take_profit_pct: Option<f64>,
        position_pct: f64,
        trailing_stop_pct: Option<f64>,
        trailing_activation_pct: Option<f64>,
        trade_direction: &str,
    ) -> BacktestResult
    where
        F: FnMut(&[Kline], usize) -> i8,
    {
        let mut balance = self.initial_balance;
        let mut position: Option<PositionState> = None;
        let mut trades: Vec<BacktestTrade> = Vec::new();
        let mut open_trade: Option<(DateTime<Utc>, f64, f64, String)> = None;
        let mut equity_curve: Vec<(DateTime<Utc>, f64)> = Vec::new();
        let mut returns: Vec<f64> = Vec::new();

        let start_date = klines
            .first()
            .map(|k| {
                chrono::DateTime::from_timestamp_millis(k.open_time)
                    .unwrap_or_else(Utc::now)
            })
            .unwrap_or_else(Utc::now);
        let end_date = klines
            .last()
            .map(|k| {
                chrono::DateTime::from_timestamp_millis(k.open_time)
                    .unwrap_or_else(Utc::now)
            })
            .unwrap_or_else(Utc::now);

        let mut profit_trades = 0i64;
        let mut loss_trades = 0i64;
        let mut total_profit = 0.0;
        let mut total_loss = 0.0;
        let mut max_consecutive_wins = 0i64;
        let mut max_consecutive_losses = 0i64;
        let mut current_consecutive_wins = 0i64;
        let mut current_consecutive_losses = 0i64;

        // Track peak equity for drawdown
        let mut peak_equity = balance;

        let mut pending_signal: Option<i8> = None;

        for i in 0..klines.len() {
            let kline = &klines[i];
            
            if let Some(raw) = pending_signal.take() {
                let exec_price = kline.open;
                let timestamp = chrono::DateTime::from_timestamp_millis(kline.open_time)
                    .unwrap_or_else(Utc::now);

                let pos_side_str = position.as_ref().map(|p| match p.side {
                    PositionSide::Long => "long",
                    PositionSide::Short => "short",
                });

                if let Some(signal) = map_raw_signal(raw, trade_direction, pos_side_str) {
                    match signal {
                        SignalType::OpenLong if position.is_none() => {
                            let open_fee = self.open_position(&mut position, exec_price, &mut balance, PositionSide::Long, position_pct);
                            open_trade = Some((timestamp, exec_price, open_fee, "long".to_string()));
                        }
                        SignalType::CloseLong if position.as_ref().map_or(false, |p| p.side == PositionSide::Long) => {
                            if let Some(bt) = self.close_position(&mut position, exec_price, &mut balance, timestamp, &mut open_trade) {
                                record_trade(&mut trades, bt, &mut profit_trades, &mut loss_trades, &mut total_profit, &mut total_loss, &mut current_consecutive_wins, &mut current_consecutive_losses, &mut max_consecutive_wins, &mut max_consecutive_losses);
                            }
                        }
                        SignalType::OpenShort if position.is_none() => {
                            let open_fee = self.open_position(&mut position, exec_price, &mut balance, PositionSide::Short, position_pct);
                            open_trade = Some((timestamp, exec_price, open_fee, "short".to_string()));
                        }
                        SignalType::CloseShort if position.as_ref().map_or(false, |p| p.side == PositionSide::Short) => {
                            if let Some(bt) = self.close_position(&mut position, exec_price, &mut balance, timestamp, &mut open_trade) {
                                record_trade(&mut trades, bt, &mut profit_trades, &mut loss_trades, &mut total_profit, &mut total_loss, &mut current_consecutive_wins, &mut current_consecutive_losses, &mut max_consecutive_wins, &mut max_consecutive_losses);
                            }
                        }
                        _ => {}
                    }
                }
            }

            let price = kline.close;
            let timestamp = chrono::DateTime::from_timestamp_millis(kline.open_time)
                .unwrap_or_else(Utc::now);
            if let Some(ref mut pos) = position {
                let pnl_pct = if pos.side == PositionSide::Long {
                    (price - pos.entry_price) / pos.entry_price
                } else {
                    (pos.entry_price - price) / pos.entry_price
                };

                let should_close = match (stop_loss_pct, take_profit_pct) {
                    (Some(sl), _) if pnl_pct <= -sl => true,
                    (_, Some(tp)) if pnl_pct >= tp => true,
                    _ => false,
                };

                if should_close {
                    if let Some(bt) = self.close_position(&mut position, price, &mut balance, timestamp, &mut open_trade) {
                        record_trade(&mut trades, bt, &mut profit_trades, &mut loss_trades, &mut total_profit, &mut total_loss, &mut current_consecutive_wins, &mut current_consecutive_losses, &mut max_consecutive_wins, &mut max_consecutive_losses);
                    }
                }
            }

            if let Some(ref mut pos) = position {
                if price > pos.highest_price {
                    pos.highest_price = price;
                }
                if price < pos.lowest_price {
                    pos.lowest_price = price;
                }

                if let (Some(ts_pct), Some(activation_pct)) = (trailing_stop_pct, trailing_activation_pct) {
                    let (profit_pct, should_trigger) = match pos.side {
                        PositionSide::Long => {
                            let pct = (pos.highest_price - pos.entry_price) / pos.entry_price;
                            let trigger = pct >= activation_pct && price <= pos.highest_price * (1.0 - ts_pct);
                            (pct, trigger)
                        }
                        PositionSide::Short => {
                            let pct = (pos.entry_price - pos.lowest_price) / pos.entry_price;
                            let trigger = pct >= activation_pct && price >= pos.lowest_price * (1.0 + ts_pct);
                            (pct, trigger)
                        }
                    };

                    if should_trigger {
                        if let Some(bt) = self.close_position(&mut position, price, &mut balance, timestamp, &mut open_trade) {
                            record_trade(&mut trades, bt, &mut profit_trades, &mut loss_trades, &mut total_profit, &mut total_loss, &mut current_consecutive_wins, &mut current_consecutive_losses, &mut max_consecutive_wins, &mut max_consecutive_losses);
                        }
                    }
                }
            }

            // Generate signal at bar close
            // Signal will be executed at next bar's open (pending_signal)
            let signal = signal_fn(klines, i);
            if signal != 0 {
                pending_signal = Some(signal);
            }

            // Calculate equity
            let equity = if let Some(ref pos) = position {
                let unrealized = if pos.side == PositionSide::Long {
                    (price - pos.entry_price) * pos.size
                } else {
                    (pos.entry_price - price) * pos.size
                };
                balance + pos.size * pos.entry_price + unrealized
            } else {
                balance
            };

            peak_equity = peak_equity.max(equity);
            let _drawdown = (peak_equity - equity) / peak_equity;

            let timestamp = chrono::DateTime::from_timestamp_millis(kline.open_time)
                .unwrap_or_else(Utc::now);
            equity_curve.push((timestamp, equity));

            if i > 0 {
                let prev_equity = equity_curve[i - 1].1;
                if prev_equity > 0.0 {
                    returns.push((equity - prev_equity) / prev_equity);
                }
            }
        }

        // Close any open position at end
        if position.is_some() {
            let last_price = klines.last().map(|k| k.close).unwrap_or(0.0);
            let last_ts = klines.last()
                .map(|k| chrono::DateTime::from_timestamp_millis(k.open_time).unwrap_or_else(Utc::now))
                .unwrap_or_else(Utc::now);
            if let Some(bt) = self.close_position(&mut position, last_price, &mut balance, last_ts, &mut open_trade) {
                record_trade(&mut trades, bt, &mut profit_trades, &mut loss_trades, &mut total_profit, &mut total_loss, &mut current_consecutive_wins, &mut current_consecutive_losses, &mut max_consecutive_wins, &mut max_consecutive_losses);
            }
        }

        let total_trades = trades.len() as i64;
        let win_rate = if total_trades > 0 {
            profit_trades as f64 / total_trades as f64
        } else {
            0.0
        };
        let avg_profit = if profit_trades > 0 { total_profit / profit_trades as f64 } else { 0.0 };
        let avg_loss = if loss_trades > 0 { total_loss / loss_trades as f64 } else { 0.0 };
        let profit_factor = if total_loss > 0.0 { total_profit / total_loss } else { f64::INFINITY };

        let total_return_pct = (balance - self.initial_balance) / self.initial_balance * 100.0;

        // Max drawdown from equity curve
        let max_drawdown_pct = equity_curve
            .iter()
            .fold((0.0_f64, 0.0_f64), |(peak, max_dd), (_, eq)| {
                let new_peak = peak.max(*eq);
                let dd = if new_peak > 0.0 { (new_peak - eq) / new_peak * 100.0 } else { 0.0 };
                (new_peak, max_dd.max(dd))
            })
            .1;

        // Sharpe ratio (annualized, assuming 252 trading days)
        let sharpe_ratio = if returns.len() > 1 {
            let mean = returns.iter().sum::<f64>() / returns.len() as f64;
            let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;
            let std_dev = variance.sqrt();
            if std_dev > 0.0 { (mean / std_dev) * (252.0_f64).sqrt() } else { 0.0 }
        } else {
            0.0
        };

        // Sortino ratio (downside deviation only)
        let sortino_ratio = if returns.len() > 1 {
            let mean = returns.iter().sum::<f64>() / returns.len() as f64;
            let downside: Vec<f64> = returns.iter().filter(|r| **r < 0.0).cloned().collect();
            let downside_dev = if downside.len() > 0 {
                let ds_mean = downside.iter().sum::<f64>() / downside.len() as f64;
                (downside.iter().map(|r| (r - ds_mean).powi(2)).sum::<f64>() / downside.len() as f64).sqrt()
            } else {
                1.0
            };
            if downside_dev > 0.0 { (mean / downside_dev) * (252.0_f64).sqrt() } else { 0.0 }
        } else {
            0.0
        };

        BacktestResult {
            id: Uuid::new_v4(),
            user_id: Uuid::nil(), // set by caller
            strategy_name: String::new(),
            symbol: klines.first().map(|k| k.symbol.clone()).unwrap_or_default(),
            exchange: klines.first().map(|k| k.exchange.clone()).unwrap_or_default(),
            timeframe: klines.first().map(|k| k.interval.clone()).unwrap_or_default(),
            start_date,
            end_date,
            initial_balance: self.initial_balance,
            final_balance: balance,
            total_return_pct,
            max_drawdown_pct,
            sharpe_ratio,
            sortino_ratio,
            win_rate: win_rate * 100.0,
            total_trades,
            profit_trades,
            loss_trades,
            avg_profit,
            avg_loss,
            profit_factor,
            max_consecutive_wins,
            max_consecutive_losses,
            trades,
            equity_curve,
            created_at: Utc::now(),
        }
    }

    fn open_position(
        &self,
        position: &mut Option<PositionState>,
        price: f64,
        balance: &mut f64,
        side: PositionSide,
        position_pct: f64,
    ) -> f64 {
        let effective_price = price * (1.0 + self.slippage);
        let max_amount = *balance * 0.99 * position_pct;
        let size = max_amount / effective_price;
        let fee = size * effective_price * self.commission_rate;
        *balance -= size * effective_price + fee;

        *position = Some(PositionState {
            side,
            size,
            entry_price: effective_price,
            highest_price: effective_price,
            lowest_price: effective_price,
        });

        fee
    }

    fn close_position(
        &self,
        position: &mut Option<PositionState>,
        price: f64,
        balance: &mut f64,
        exit_time: DateTime<Utc>,
        open_trade: &mut Option<(DateTime<Utc>, f64, f64, String)>,
    ) -> Option<BacktestTrade> {
        let pos = position.take().unwrap();
        let effective_price = price * (1.0 - self.slippage);
        let revenue = pos.size * effective_price;
        let fee = revenue * self.commission_rate;
        let pnl = revenue - pos.size * pos.entry_price - fee;
        *balance += revenue - fee;

        let ot = open_trade.take()?;
        let entry_price = ot.1;
        let entry_time = ot.0;
        let side = ot.3;
        let pnl_pct = (effective_price - entry_price) / entry_price * 100.0;
        let total_commission = ot.2 + fee;

        Some(BacktestTrade {
            entry_time,
            exit_time,
            side,
            entry_price,
            exit_price: effective_price,
            quantity: pos.size,
            pnl,
            pnl_pct,
            commission: total_commission,
        })
    }
}

struct PositionState {
    side: PositionSide,
    size: f64,
    entry_price: f64,
    highest_price: f64,
    lowest_price: f64,
}

// ============================================================
// Built-in Technical Indicator Signal Generators
// ============================================================

/// Simple Moving Average Crossover signal generator (uses PrecomputedIndicators for O(1) lookup).
pub fn sma_crossover_signal_cached(
    cache: &indicators::PrecomputedIndicators,
    idx: usize,
    fast_period: usize,
    slow_period: usize,
) -> i8 {
    if idx < 1 || idx < slow_period - 1 {
        return 0;
    }
    let fast_sma = cache.sma_at(idx, fast_period);
    let prev_fast_sma = cache.sma_at(idx - 1, fast_period);
    let slow_sma = cache.sma_at(idx, slow_period);
    let prev_slow_sma = cache.sma_at(idx - 1, slow_period);

    if prev_fast_sma <= prev_slow_sma && fast_sma > slow_sma {
        1
    } else if prev_fast_sma >= prev_slow_sma && fast_sma < slow_sma {
        -1
    } else {
        0
    }
}

/// Simple Moving Average Crossover signal generator.
/// Buy when fast SMA crosses above slow SMA, sell when it crosses below.
pub fn sma_crossover_signal(klines: &[Kline], idx: usize, fast_period: usize, slow_period: usize) -> i8 {
    if idx < 1 || klines.len() < 2 || idx < slow_period - 1 { return 0; }
    let fast_sma = indicators::sma_at(klines, idx, fast_period);
    let prev_fast_sma = indicators::sma_at(klines, idx - 1, fast_period);
    let slow_sma = indicators::sma_at(klines, idx, slow_period);
    let prev_slow_sma = indicators::sma_at(klines, idx - 1, slow_period);

    if prev_fast_sma <= prev_slow_sma && fast_sma > slow_sma {
        1 // Buy signal
    } else if prev_fast_sma >= prev_slow_sma && fast_sma < slow_sma {
        -1 // Sell signal
    } else {
        0
    }
}

/// RSI-based signal generator (cached version).
pub fn rsi_signal_cached(
    cache: &indicators::PrecomputedIndicators,
    idx: usize,
    period: usize,
    oversold: f64,
    overbought: f64,
) -> i8 {
    if idx < 1 || idx < period {
        return 0;
    }
    let rsi = cache.rsi_at(idx, period);
    let prev_rsi = cache.rsi_at(idx - 1, period);

    if prev_rsi >= oversold && rsi < oversold {
        1 // Buy signal (RSI crossed below oversold)
    } else if prev_rsi <= overbought && rsi > overbought {
        -1 // Sell signal (RSI crossed above overbought)
    } else {
        0
    }
}

/// RSI-based signal generator.
/// Buy when RSI crosses below oversold, sell when RSI crosses above overbought.
pub fn rsi_signal(klines: &[Kline], idx: usize, period: usize, oversold: f64, overbought: f64) -> i8 {
    if idx < 1 || idx < period { return 0; }
    let rsi = indicators::rsi_at(klines, idx, period);
    let prev_rsi = indicators::rsi_at(klines, idx - 1, period);

    if prev_rsi >= oversold && rsi < oversold {
        1 // Buy signal (RSI crossed below oversold)
    } else if prev_rsi <= overbought && rsi > overbought {
        -1 // Sell signal (RSI crossed above overbought)
    } else {
        0
    }
}

/// MACD-based signal generator (cached version).
pub fn macd_signal_cached(
    cache: &indicators::PrecomputedIndicators,
    idx: usize,
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
) -> i8 {
    if idx < 1 || idx < slow_period + signal_period - 2 {
        return 0;
    }
    let macd = cache.macd_line_at(idx, fast_period, slow_period, signal_period);
    let signal = cache.macd_signal_at(idx, fast_period, slow_period, signal_period);
    let prev_macd = cache.macd_line_at(idx - 1, fast_period, slow_period, signal_period);
    let prev_signal = cache.macd_signal_at(idx - 1, fast_period, slow_period, signal_period);

    if prev_macd <= prev_signal && macd > signal {
        1
    } else if prev_macd >= prev_signal && macd < signal {
        -1
    } else {
        0
    }
}

/// MACD-based signal generator.
pub fn macd_signal(
    klines: &[Kline],
    idx: usize,
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
) -> i8 {
    if idx < 1 || idx < slow_period + signal_period - 2 { return 0; }
    let macd = indicators::macd_at(klines, idx, fast_period, slow_period);
    let signal = indicators::macd_signal_at(klines, idx, fast_period, slow_period, signal_period);
    let prev_macd = indicators::macd_at(klines, idx - 1, fast_period, slow_period);
    let prev_signal = indicators::macd_signal_at(klines, idx - 1, fast_period, slow_period, signal_period);

    if prev_macd <= prev_signal && macd > signal {
        1
    } else if prev_macd >= prev_signal && macd < signal {
        -1
    } else {
        0
    }
}

/// Bollinger Bands signal generator (cached version).
pub fn bollinger_bands_signal_cached(
    cache: &indicators::PrecomputedIndicators,
    klines: &[Kline],
    idx: usize,
    period: usize,
    std_dev_mult: f64,
) -> i8 {
    if idx < period - 1 {
        return 0;
    }
    let (upper, _middle, lower) = cache.bbands_at(idx, period, std_dev_mult);
    let price = klines[idx].close;

    if price <= lower {
        1 // Price at lower band - buy
    } else if price >= upper {
        -1 // Price at upper band - sell
    } else {
        0
    }
}

/// Bollinger Bands signal generator.
pub fn bollinger_bands_signal(klines: &[Kline], idx: usize, period: usize, std_dev_mult: f64) -> i8 {
    if idx < period - 1 { return 0; }
    let (upper, _middle, lower) = indicators::bbands_at(klines, idx, period, std_dev_mult);
    let price = klines[idx].close;

    if price <= lower {
        1 // Price at lower band - buy
    } else if price >= upper {
        -1 // Price at upper band - sell
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_klines(closes: &[f64]) -> Vec<Kline> {
        closes
            .iter()
            .enumerate()
            .map(|(i, &c)| Kline {
                open_time: (i as i64) * 60000,
                open: c,
                high: c * 1.01,
                low: c * 0.99,
                close: c,
                volume: 1000.0,
                close_time: ((i + 1) as i64) * 60000,
                quote_volume: 100000.0,
                trades: 10,
                symbol: "BTC/USDT".into(),
                exchange: "binance".into(),
                interval: "1m".into(),
            })
            .collect()
    }

    #[test]
    fn test_sma_crossover() {
        let closes: Vec<f64> = (0..50).map(|i| 100.0 + i as f64).collect();
        let klines = make_klines(&closes);
        // In uptrend, fast SMA should be above slow SMA
        let signal = sma_crossover_signal(&klines, 30, 5, 20);
        // Should eventually give a buy signal in a strong uptrend
        assert!(signal == 1 || signal == 0);
    }

    #[test]
    fn test_backtest_engine() {
        // Simulate a simple uptrend with some volatility
        let closes: Vec<f64> = (0..100)
            .map(|i| 100.0 + (i as f64) * 0.5 + (i as f64).sin() * 2.0)
            .collect();
        let klines = make_klines(&closes);

        let engine = BacktestEngine::new(10000.0, 0.001, 0.0005);

        let result = engine.run(&klines, |klines, idx| {
            sma_crossover_signal(klines, idx, 5, 20)
        }, Some(0.05), Some(0.10), 1.0, None, None, "long");

        assert!(result.total_trades >= 0);
        assert!(result.final_balance >= 0.0);
        assert!(result.max_drawdown_pct >= 0.0);
    }

    #[test]
    fn test_rsi_calculation() {
        let closes: Vec<f64> = (0..30).map(|i| 100.0 + (i as f64)).collect();
        let klines = make_klines(&closes);
        let rsi = indicators::rsi_at(&klines, 20, 14);
        // In a continuous uptrend, RSI should be high (> 50)
        assert!(rsi > 50.0);
    }

    #[test]
    fn test_bollinger_bands() {
        let closes: Vec<f64> = (0..30).map(|i| 100.0 + (i as f64).sin() * 5.0).collect();
        let klines = make_klines(&closes);
        let (upper, middle, lower) = indicators::bbands_at(&klines, 25, 20, 2.0);
        assert!(upper > middle);
        assert!(middle > lower);
    }
}
