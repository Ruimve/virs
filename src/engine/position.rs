//! Position Manager — calculates order amounts and tracks positions.
//!
//! Responsible for:
//! - Calculating order size based on strategy config and available balance
//! - Tracking current position state (long/short/flat)
//! - Determining close vs open signals
//! - Applying risk management rules (max position size, stop loss, take profit)
//! - Querying exchange balance for balance_pct mode

use crate::models::*;
use crate::exchange::Exchange;
use tracing::{info, warn};

/// Position state tracker for a single strategy.
pub struct PositionManager {
    strategy_id: uuid::Uuid,
    symbol: String,
    exchange_name: String,
    /// Current position side (None = flat)
    current_side: Option<PositionSide>,
    /// Current position size
    current_size: f64,
    /// Entry price of current position
    entry_price: f64,
    /// Trading configuration
    config: PositionConfig,
    /// Cached quote currency balance (updated on each determine_amount call)
    cached_quote_balance: f64,
    /// Highest price since entry (for trailing stop on long)
    peak_price: f64,
    /// Lowest price since entry (for trailing stop on short)
    trough_price: f64,
}

#[derive(Debug, Clone)]
struct PositionConfig {
    /// Fixed order amount in quote currency (e.g., 100 USDT)
    fixed_amount: f64,
    /// Max position size in base currency
    max_position_size: f64,
    /// Percentage of balance to use per trade (0.0 - 1.0)
    balance_pct: f64,
    /// Stop loss percentage (e.g., 0.02 = 2%)
    stop_loss_pct: Option<f64>,
    /// Take profit percentage (e.g., 0.05 = 5%)
    take_profit_pct: Option<f64>,
    /// Trailing stop percentage (e.g., 0.015 = 1.5%)
    trailing_stop_pct: Option<f64>,
    /// Trailing stop activation percentage (e.g., 0.03 = 3% profit to activate)
    trailing_activation_pct: Option<f64>,
    /// Trade direction: "long", "short", or "both"
    trade_direction: String,
}

impl PositionManager {
    pub fn new(
        strategy_id: uuid::Uuid,
        symbol: String,
        exchange_name: String,
        trading_config: &serde_json::Value,
    ) -> Self {
        let config = PositionConfig {
            fixed_amount: trading_config
                .get("fixed_amount")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            max_position_size: trading_config
                .get("max_position_size")
                .and_then(|v| v.as_f64())
                .unwrap_or(1000.0),
            balance_pct: trading_config
                .get("balance_pct")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.1),
            stop_loss_pct: trading_config
                .get("stop_loss_pct")
                .and_then(|v| v.as_f64()),
            take_profit_pct: trading_config
                .get("take_profit_pct")
                .and_then(|v| v.as_f64()),
            trailing_stop_pct: trading_config
                .get("trailing_stop_pct")
                .and_then(|v| v.as_f64()),
            trailing_activation_pct: trading_config
                .get("trailing_activation_pct")
                .and_then(|v| v.as_f64()),
            trade_direction: trading_config
                .get("trade_direction")
                .and_then(|v| v.as_str())
                .unwrap_or("long")
                .to_string(),
        };

        Self {
            strategy_id,
            symbol,
            exchange_name,
            current_side: None,
            current_size: 0.0,
            entry_price: 0.0,
            config,
            cached_quote_balance: 0.0,
            peak_price: 0.0,
            trough_price: f64::MAX,
        }
    }

    /// Calculate the order side and amount based on a signal.
    /// Returns (Side, amount) or skips if invalid.
    pub fn calculate_order(&mut self, signal: &SignalType) -> (Side, f64) {
        match signal {
            SignalType::OpenLong => {
                if self.current_side.is_some() {
                    warn!(
                        "[Strategy {}] OpenLong signal but already in position {:?}, skipping",
                        self.strategy_id, self.current_side
                    );
                    return (Side::Buy, 0.0);
                }
                let amount = self.determine_amount();
                if amount <= 0.0 {
                    return (Side::Buy, 0.0);
                }
                self.current_side = Some(PositionSide::Long);
                self.current_size = amount;
                self.peak_price = 0.0;
                self.trough_price = f64::MAX;
                (Side::Buy, amount)
            }
            SignalType::CloseLong => {
                if self.current_side != Some(PositionSide::Long) {
                    warn!(
                        "[Strategy {}] CloseLong signal but no long position, skipping",
                        self.strategy_id
                    );
                    return (Side::Sell, 0.0);
                }
                let amount = self.current_size;
                info!(
                    "[Strategy {}] Closing long position: size={}",
                    self.strategy_id, amount
                );
                self.current_side = None;
                self.current_size = 0.0;
                self.entry_price = 0.0;
                (Side::Sell, amount)
            }
            SignalType::OpenShort => {
                if self.current_side.is_some() {
                    warn!(
                        "[Strategy {}] OpenShort signal but already in position, skipping",
                        self.strategy_id
                    );
                    return (Side::Sell, 0.0);
                }
                let amount = self.determine_amount();
                if amount <= 0.0 {
                    return (Side::Sell, 0.0);
                }
                self.current_side = Some(PositionSide::Short);
                self.current_size = amount;
                self.peak_price = 0.0;
                self.trough_price = f64::MAX;
                (Side::Sell, amount)
            }
            SignalType::CloseShort => {
                if self.current_side != Some(PositionSide::Short) {
                    warn!(
                        "[Strategy {}] CloseShort signal but no short position, skipping",
                        self.strategy_id
                    );
                    return (Side::Buy, 0.0);
                }
                let amount = self.current_size;
                info!(
                    "[Strategy {}] Closing short position: size={}",
                    self.strategy_id, amount
                );
                self.current_side = None;
                self.current_size = 0.0;
                self.entry_price = 0.0;
                (Side::Buy, amount)
            }
        }
    }

    /// Whether this strategy is allowed to open long positions.
    pub fn can_go_long(&self) -> bool {
        self.config.trade_direction == "long" || self.config.trade_direction == "both"
    }

    /// Whether this strategy is allowed to open short positions.
    pub fn can_go_short(&self) -> bool {
        self.config.trade_direction == "short" || self.config.trade_direction == "both"
    }

    pub fn set_entry_price(&mut self, price: f64) {
        self.entry_price = price;
    }

    /// Check if current position should be closed due to risk management.
    /// Returns Some((Side, amount)) with the close side and position size if position should be closed, None otherwise.
    /// `current_price` is the latest market price.
    pub fn check_risk(&mut self, current_price: f64) -> Option<(Side, f64)> {
        if self.current_side.is_none() || self.entry_price <= 0.0 {
            return None;
        }

        let pnl_pct = if self.current_side == Some(PositionSide::Long) {
            (current_price - self.entry_price) / self.entry_price
        } else {
            (self.entry_price - current_price) / self.entry_price
        };

        // Check fixed stop-loss
        if let Some(sl) = self.config.stop_loss_pct {
            if pnl_pct <= -sl {
                info!(
                    "[Strategy {}] Stop-loss triggered: pnl_pct={:.4}% <= -{}%",
                    self.strategy_id, pnl_pct * 100.0, sl * 100.0
                );
                return self.close_position();
            }
        }

        // Check fixed take-profit
        if let Some(tp) = self.config.take_profit_pct {
            if pnl_pct >= tp {
                info!(
                    "[Strategy {}] Take-profit triggered: pnl_pct={:.4}% >= {}%",
                    self.strategy_id, pnl_pct * 100.0, tp * 100.0
                );
                return self.close_position();
            }
        }

        // Check trailing stop
        if let (Some(ts_pct), Some(activation_pct)) = (self.config.trailing_stop_pct, self.config.trailing_activation_pct) {
            if pnl_pct >= activation_pct {
                let is_long = self.current_side == Some(PositionSide::Long);

                if is_long {
                    if current_price > self.peak_price {
                        self.peak_price = current_price;
                    }
                    let trailing_stop_price = self.peak_price * (1.0 - ts_pct);
                    if current_price <= trailing_stop_price {
                        info!(
                            "[Strategy {}] Trailing stop triggered (long): price={}, trailing_stop={:.2}",
                            self.strategy_id, current_price, trailing_stop_price
                        );
                        return self.close_position();
                    }
                } else {
                    if current_price < self.trough_price {
                        self.trough_price = current_price;
                    }
                    let trailing_stop_price = self.trough_price * (1.0 + ts_pct);
                    if current_price >= trailing_stop_price {
                        info!(
                            "[Strategy {}] Trailing stop triggered (short): price={}, trailing_stop={:.2}",
                            self.strategy_id, current_price, trailing_stop_price
                        );
                        return self.close_position();
                    }
                }
            }
        }

        None
    }

    /// Close current position and return the close side and size.
    fn close_position(&mut self) -> Option<(Side, f64)> {
        let side = self.current_side.as_ref()?.clone();
        let size = self.current_size;
        info!(
            "[Strategy {}] Risk management closing {:?} position: size={}",
            self.strategy_id, side, size
        );
        self.current_side = None;
        self.current_size = 0.0;
        self.entry_price = 0.0;
        self.peak_price = 0.0;
        self.trough_price = f64::MAX;
        Some((if side == PositionSide::Long { Side::Sell } else { Side::Buy }, size))
    }

    /// Determine order amount based on config.
    /// Priority: fixed_amount > balance_pct > 0 (no trade)
    ///
    /// When balance_pct is configured, queries the exchange for the quote currency
    /// balance and calculates the amount as `free_balance * balance_pct`.
    pub async fn determine_amount_async(&mut self, exchange: &dyn Exchange) -> f64 {
        if self.config.fixed_amount > 0.0 {
            return self.config.fixed_amount.min(self.config.max_position_size);
        }

        if self.config.balance_pct > 0.0 {
            // Extract quote currency from symbol (e.g., "BTCUSDT" -> "USDT")
            let quote_currency = self.extract_quote_currency();

            // Query exchange balance
            match exchange.get_balances().await {
                Ok(balances) => {
                    // Find the quote currency balance
                    let quote_balance = balances
                        .iter()
                        .find(|b| b.asset.eq_ignore_ascii_case(&quote_currency))
                        .map(|b| b.free)
                        .unwrap_or(0.0);

                    self.cached_quote_balance = quote_balance;

                    if quote_balance <= 0.0 {
                        warn!(
                            "[Strategy {}] Insufficient {} balance for balance_pct mode (free={})",
                            self.strategy_id, quote_currency, quote_balance
                        );
                        return 0.0;
                    }

                    let amount = quote_balance * self.config.balance_pct;
                    let capped = amount.min(self.config.max_position_size);

                    info!(
                        "[Strategy {}] balance_pct: {} free * {}% = {} (capped at {})",
                        self.strategy_id, quote_currency, self.config.balance_pct * 100.0,
                        amount, capped
                    );

                    return capped;
                }
                Err(e) => {
                    warn!(
                        "[Strategy {}] Failed to query exchange balance: {}. Cannot use balance_pct mode.",
                        self.strategy_id, e
                    );
                    return 0.0;
                }
            }
        }

        0.0
    }

    /// Synchronous fallback — uses cached balance or returns 0.
    /// Used when async exchange query is not available.
    pub fn determine_amount(&self) -> f64 {
        if self.config.fixed_amount > 0.0 {
            return self.config.fixed_amount.min(self.config.max_position_size);
        }

        if self.config.balance_pct > 0.0 && self.cached_quote_balance > 0.0 {
            let amount = self.cached_quote_balance * self.config.balance_pct;
            return amount.min(self.config.max_position_size);
        }

        0.0
    }

    /// Extract quote currency from symbol.
    /// Handles formats: "BTCUSDT" -> "USDT", "BTC/USDT" -> "USDT", "BTC_USDT" -> "USDT"
    fn extract_quote_currency(&self) -> String {
        let sym = &self.symbol;

        // Try slash separator first: "BTC/USDT"
        if let Some(pos) = sym.find('/') {
            return sym[pos + 1..].to_uppercase();
        }

        // Try underscore: "BTC_USDT"
        if let Some(pos) = sym.find('_') {
            return sym[pos + 1..].to_uppercase();
        }

        // Try common quote currencies (longest match first to avoid "BUSD" matching "USDT")
        let common_quotes = [
            "USDC", "USDT", "BUSD", "TUSD", "DAI", "USD", "EUR", "BTC", "ETH", "BNB",
        ];

        let upper = sym.to_uppercase();
        for quote in &common_quotes {
            if upper.ends_with(quote) {
                return quote.to_string();
            }
        }

        // Fallback: assume last 3-4 chars are quote
        if upper.len() >= 7 {
            upper[upper.len() - 4..].to_string()
        } else if upper.len() >= 4 {
            upper[upper.len() - 3..].to_string()
        } else {
            "USDT".to_string()
        }
    }

    /// Get current position info.
    pub fn position_info(&self) -> Option<PositionInfo> {
        self.current_side.as_ref().map(|side| PositionInfo {
            side: side.clone(),
            size: self.current_size,
            entry_price: self.entry_price,
        })
    }

    /// Get the cached quote currency balance.
    pub fn cached_quote_balance(&self) -> f64 {
        self.cached_quote_balance
    }
}

/// Current position information.
#[derive(Debug, Clone)]
pub struct PositionInfo {
    pub side: PositionSide,
    pub size: f64,
    pub entry_price: f64,
}
