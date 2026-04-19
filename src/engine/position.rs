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
