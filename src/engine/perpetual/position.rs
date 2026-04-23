use crate::models::*;
use crate::exchange::Exchange;
use tracing::{info, warn};

pub struct PositionManager {
    strategy_id: uuid::Uuid,
    symbol: String,
    exchange_name: String,
    long_position: Option<PositionData>,
    short_position: Option<PositionData>,
    config: PositionConfig,
    cached_quote_balance: f64,
}

#[derive(Debug, Clone)]
struct PositionData {
    size: f64,
    entry_price: f64,
    peak_price: f64,      // 用于追踪止损（多头）
    trough_price: f64,    // 用于追踪止损（空头）
}

#[derive(Debug, Clone)]
struct PositionConfig {
    fixed_amount: f64,
    max_position_size: f64,
    balance_pct: f64,
    stop_loss_pct: Option<f64>,
    take_profit_pct: Option<f64>,
    trailing_stop_pct: Option<f64>,
    trailing_activation_pct: Option<f64>,
    trade_direction: String,
    leverage: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RiskReason {
    StopLoss,
    TakeProfit,
    TrailingStop,
}

impl RiskReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskReason::StopLoss => "stop_loss",
            RiskReason::TakeProfit => "take_profit",
            RiskReason::TrailingStop => "trailing_stop",
        }
    }
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
            leverage: trading_config
                .get("leverage")
                .and_then(|v| v.as_u64())
                .unwrap_or(5) as u32,
        };

        Self {
            strategy_id,
            symbol,
            exchange_name,
            long_position: None,
            short_position: None,
            config,
            cached_quote_balance: 0.0,
        }
    }

    /// Return the configured leverage multiplier.
    pub fn leverage(&self) -> u32 {
        self.config.leverage
    }

    /// Prepare an order based on a signal WITHOUT mutating position state.
    /// Returns (Side, base_currency_amount).
    /// `current_price` is used to convert quote amount -> base amount for open orders.
    /// For close orders, returns the tracked base-currency position size.
    /// Open orders multiply the quote amount by leverage.
    ///
    /// Call `apply_signal` ONLY after the exchange confirms the order.
    pub fn prepare_order(&self, signal: &SignalType, current_price: f64) -> (Side, f64) {
        if current_price <= 0.0 {
            warn!(
                "[Strategy {}] Invalid current_price={}, cannot prepare order",
                self.strategy_id, current_price
            );
            return match signal {
                SignalType::OpenLong | SignalType::CloseLong => (Side::Buy, 0.0),
                SignalType::OpenShort | SignalType::CloseShort => (Side::Sell, 0.0),
            };
        }

        match signal {
            SignalType::OpenLong => {
                if self.long_position.is_some() {
                    warn!(
                        "[Strategy {}] OpenLong signal but already in long position, skipping",
                        self.strategy_id
                    );
                    return (Side::Buy, 0.0);
                }
                let quote_amount = self.determine_amount();
                if quote_amount <= 0.0 {
                    return (Side::Buy, 0.0);
                }
                let base_amount = (quote_amount * self.config.leverage as f64) / current_price;
                info!(
                    "[Strategy {}] Prepare OpenLong: quote={:.2}, leverage={}x, base={:.6} @ price={:.2}",
                    self.strategy_id, quote_amount, self.config.leverage, base_amount, current_price
                );
                (Side::Buy, base_amount)
            }
            SignalType::CloseLong => {
                match &self.long_position {
                    Some(pos) => {
                        info!(
                            "[Strategy {}] Prepare CloseLong: base_size={:.6}",
                            self.strategy_id, pos.size
                        );
                        (Side::Sell, pos.size)
                    }
                    None => {
                        warn!(
                            "[Strategy {}] CloseLong signal but no long position, skipping",
                            self.strategy_id
                        );
                        (Side::Sell, 0.0)
                    }
                }
            }
            SignalType::OpenShort => {
                if self.short_position.is_some() {
                    warn!(
                        "[Strategy {}] OpenShort signal but already in short position, skipping",
                        self.strategy_id
                    );
                    return (Side::Sell, 0.0);
                }
                let quote_amount = self.determine_amount();
                if quote_amount <= 0.0 {
                    return (Side::Sell, 0.0);
                }
                let base_amount = (quote_amount * self.config.leverage as f64) / current_price;
                info!(
                    "[Strategy {}] Prepare OpenShort: quote={:.2}, leverage={}x, base={:.6} @ price={:.2}",
                    self.strategy_id, quote_amount, self.config.leverage, base_amount, current_price
                );
                (Side::Sell, base_amount)
            }
            SignalType::CloseShort => {
                match &self.short_position {
                    Some(pos) => {
                        info!(
                            "[Strategy {}] Prepare CloseShort: base_size={:.6}",
                            self.strategy_id, pos.size
                        );
                        (Side::Buy, pos.size)
                    }
                    None => {
                        warn!(
                            "[Strategy {}] CloseShort signal but no short position, skipping",
                            self.strategy_id
                        );
                        (Side::Buy, 0.0)
                    }
                }
            }
        }
    }

    /// Async version of prepare_order -- queries exchange for balance when using balance_pct mode.
    pub async fn prepare_order_async(
        &mut self,
        signal: &SignalType,
        exchange: &dyn Exchange,
        current_price: f64,
    ) -> (Side, f64) {
        if current_price <= 0.0 {
            warn!(
                "[Strategy {}] Invalid current_price={}, cannot prepare order",
                self.strategy_id, current_price
            );
            return match signal {
                SignalType::OpenLong | SignalType::CloseLong => (Side::Buy, 0.0),
                SignalType::OpenShort | SignalType::CloseShort => (Side::Sell, 0.0),
            };
        }

        match signal {
            SignalType::OpenLong => {
                if self.long_position.is_some() {
                    warn!(
                        "[Strategy {}] OpenLong signal but already in long position, skipping",
                        self.strategy_id
                    );
                    return (Side::Buy, 0.0);
                }
                let quote_amount = self.determine_amount_async(exchange).await;
                if quote_amount <= 0.0 {
                    return (Side::Buy, 0.0);
                }
                let base_amount = (quote_amount * self.config.leverage as f64) / current_price;
                info!(
                    "[Strategy {}] Prepare OpenLong: quote={:.2}, leverage={}x, base={:.6} @ price={:.2}",
                    self.strategy_id, quote_amount, self.config.leverage, base_amount, current_price
                );
                (Side::Buy, base_amount)
            }
            SignalType::CloseLong => {
                match &self.long_position {
                    Some(pos) => {
                        info!(
                            "[Strategy {}] Prepare CloseLong: base_size={:.6}",
                            self.strategy_id, pos.size
                        );
                        (Side::Sell, pos.size)
                    }
                    None => {
                        warn!(
                            "[Strategy {}] CloseLong signal but no long position, skipping",
                            self.strategy_id
                        );
                        (Side::Sell, 0.0)
                    }
                }
            }
            SignalType::OpenShort => {
                if self.short_position.is_some() {
                    warn!(
                        "[Strategy {}] OpenShort signal but already in short position, skipping",
                        self.strategy_id
                    );
                    return (Side::Sell, 0.0);
                }
                let quote_amount = self.determine_amount_async(exchange).await;
                if quote_amount <= 0.0 {
                    return (Side::Sell, 0.0);
                }
                let base_amount = (quote_amount * self.config.leverage as f64) / current_price;
                info!(
                    "[Strategy {}] Prepare OpenShort: quote={:.2}, leverage={}x, base={:.6} @ price={:.2}",
                    self.strategy_id, quote_amount, self.config.leverage, base_amount, current_price
                );
                (Side::Sell, base_amount)
            }
            SignalType::CloseShort => {
                match &self.short_position {
                    Some(pos) => {
                        info!(
                            "[Strategy {}] Prepare CloseShort: base_size={:.6}",
                            self.strategy_id, pos.size
                        );
                        (Side::Buy, pos.size)
                    }
                    None => {
                        warn!(
                            "[Strategy {}] CloseShort signal but no short position, skipping",
                            self.strategy_id
                        );
                        (Side::Buy, 0.0)
                    }
                }
            }
        }
    }

    /// Apply a confirmed signal to position state.
    /// Call this ONLY after the exchange has confirmed the order.
    /// `base_amount` is the filled amount in base currency.
    /// `fill_price` is the actual execution price from the exchange.
    pub fn apply_signal(&mut self, signal: &SignalType, base_amount: f64, fill_price: f64) {
        match signal {
            SignalType::OpenLong => {
                self.long_position = Some(PositionData {
                    size: base_amount,
                    entry_price: fill_price,
                    peak_price: fill_price,
                    trough_price: f64::MAX,
                });
                info!(
                    "[Strategy {}] Applied OpenLong: size={:.6}, entry={:.2}",
                    self.strategy_id, base_amount, fill_price
                );
            }
            SignalType::CloseLong => {
                if let Some(ref pos) = self.long_position {
                    info!(
                        "[Strategy {}] Applied CloseLong: size={:.6}, exit={:.2}",
                        self.strategy_id, pos.size, fill_price
                    );
                }
                self.long_position = None;
            }
            SignalType::OpenShort => {
                self.short_position = Some(PositionData {
                    size: base_amount,
                    entry_price: fill_price,
                    peak_price: 0.0,
                    trough_price: fill_price,
                });
                info!(
                    "[Strategy {}] Applied OpenShort: size={:.6}, entry={:.2}",
                    self.strategy_id, base_amount, fill_price
                );
            }
            SignalType::CloseShort => {
                if let Some(ref pos) = self.short_position {
                    info!(
                        "[Strategy {}] Applied CloseShort: size={:.6}, exit={:.2}",
                        self.strategy_id, pos.size, fill_price
                    );
                }
                self.short_position = None;
            }
        }
    }

    /// Update peak/trough price tracking for trailing stop calculation.
    /// Should be called on every cycle with the latest market price,
    /// BEFORE calling `check_risk`.
    pub fn update_price_tracking(&mut self, current_price: f64) {
        if current_price <= 0.0 {
            return;
        }
        if let Some(ref mut pos) = self.long_position {
            if current_price > pos.peak_price {
                pos.peak_price = current_price;
            }
        }
        if let Some(ref mut pos) = self.short_position {
            if current_price < pos.trough_price {
                pos.trough_price = current_price;
            }
        }
    }

    pub fn can_go_long(&self) -> bool {
        self.config.trade_direction == "long" || self.config.trade_direction == "both"
    }

    pub fn can_go_short(&self) -> bool {
        self.config.trade_direction == "short" || self.config.trade_direction == "both"
    }

    /// Check if any position should be closed due to risk management.
    /// Returns Some((Side, base_amount, reason)) if a position should be closed.
    /// Does NOT mutate position state -- call `apply_risk_close` after order confirmation.
    /// If both long and short trigger risk, returns the one with larger loss first.
    /// `current_price` should be a real-time price (e.g., from ticker), not a stale kline close.
    pub fn check_risk(&self, current_price: f64) -> Option<(Side, f64, RiskReason)> {
        let long_risk = self.check_single_risk(
            &self.long_position,
            PositionSide::Long,
            current_price,
        );
        let short_risk = self.check_single_risk(
            &self.short_position,
            PositionSide::Short,
            current_price,
        );

        match (long_risk, short_risk) {
            (Some(l), Some(s)) => {
                // Both triggered: return the one with larger loss (more negative pnl_pct)
                let l_pnl = self.calc_pnl_pct(self.long_position.as_ref().unwrap(), &PositionSide::Long, current_price);
                let s_pnl = self.calc_pnl_pct(self.short_position.as_ref().unwrap(), &PositionSide::Short, current_price);
                if l_pnl <= s_pnl {
                    Some(l)
                } else {
                    Some(s)
                }
            }
            (Some(l), None) => Some(l),
            (None, Some(s)) => Some(s),
            (None, None) => None,
        }
    }

    /// Check risk for all positions (both long and short).
    /// Returns a Vec of all triggered risks, so callers can close each one.
    /// Does NOT mutate position state -- call `apply_risk_close` after each order confirmation.
    pub fn check_all_risks(&self, current_price: f64) -> Vec<(Side, f64, RiskReason)> {
        let mut risks = Vec::new();
        if let Some(r) = self.check_single_risk(&self.long_position, PositionSide::Long, current_price) {
            risks.push(r);
        }
        if let Some(r) = self.check_single_risk(&self.short_position, PositionSide::Short, current_price) {
            risks.push(r);
        }
        risks
    }

    /// Check risk for a single position direction.
    fn check_single_risk(
        &self,
        position: &Option<PositionData>,
        side: PositionSide,
        current_price: f64,
    ) -> Option<(Side, f64, RiskReason)> {
        let pos = match position {
            Some(p) => p,
            None => return None,
        };

        if pos.entry_price <= 0.0 {
            return None;
        }

        let pnl_pct = self.calc_pnl_pct(pos, &side, current_price);

        // Stop loss
        if let Some(sl) = self.config.stop_loss_pct {
            if pnl_pct <= -sl {
                info!(
                    "[Strategy {}] Stop-loss triggered ({:?}): pnl_pct={:.4}% <= -{}%",
                    self.strategy_id, side, pnl_pct * 100.0, sl * 100.0
                );
                let close_side = match side {
                    PositionSide::Long => Side::Sell,
                    PositionSide::Short => Side::Buy,
                };
                return Some((close_side, pos.size, RiskReason::StopLoss));
            }
        }

        // Take profit
        if let Some(tp) = self.config.take_profit_pct {
            if pnl_pct >= tp {
                info!(
                    "[Strategy {}] Take-profit triggered ({:?}): pnl_pct={:.4}% >= {}%",
                    self.strategy_id, side, pnl_pct * 100.0, tp * 100.0
                );
                let close_side = match side {
                    PositionSide::Long => Side::Sell,
                    PositionSide::Short => Side::Buy,
                };
                return Some((close_side, pos.size, RiskReason::TakeProfit));
            }
        }

        // Trailing stop
        if let (Some(ts_pct), Some(activation_pct)) =
            (self.config.trailing_stop_pct, self.config.trailing_activation_pct)
        {
            if pnl_pct >= activation_pct {
                match side {
                    PositionSide::Long => {
                        let trailing_stop_price = pos.peak_price * (1.0 - ts_pct);
                        if current_price <= trailing_stop_price {
                            info!(
                                "[Strategy {}] Trailing stop triggered (long): price={}, trailing_stop={:.2}",
                                self.strategy_id, current_price, trailing_stop_price
                            );
                            return Some((Side::Sell, pos.size, RiskReason::TrailingStop));
                        }
                    }
                    PositionSide::Short => {
                        let trailing_stop_price = pos.trough_price * (1.0 + ts_pct);
                        if current_price >= trailing_stop_price {
                            info!(
                                "[Strategy {}] Trailing stop triggered (short): price={}, trailing_stop={:.2}",
                                self.strategy_id, current_price, trailing_stop_price
                            );
                            return Some((Side::Buy, pos.size, RiskReason::TrailingStop));
                        }
                    }
                }
            }
        }

        None
    }

    /// Calculate PnL percentage for a single position.
    fn calc_pnl_pct(&self, pos: &PositionData, side: &PositionSide, current_price: f64) -> f64 {
        match side {
            PositionSide::Long => (current_price - pos.entry_price) / pos.entry_price,
            PositionSide::Short => (pos.entry_price - current_price) / pos.entry_price,
        }
    }

    /// Apply risk close to a specific side's position state after the close order is confirmed.
    pub fn apply_risk_close(&mut self, side: Side) {
        match side {
            Side::Sell => {
                // Close long
                if let Some(ref pos) = self.long_position {
                    info!(
                        "[Strategy {}] Risk close applied: long position closed, size={:.6}",
                        self.strategy_id, pos.size
                    );
                }
                self.long_position = None;
            }
            Side::Buy => {
                // Close short
                if let Some(ref pos) = self.short_position {
                    info!(
                        "[Strategy {}] Risk close applied: short position closed, size={:.6}",
                        self.strategy_id, pos.size
                    );
                }
                self.short_position = None;
            }
        }
    }

    /// Determine order amount in quote currency based on config.
    /// Priority: fixed_amount > balance_pct > 0 (no trade)
    pub async fn determine_amount_async(&mut self, exchange: &dyn Exchange) -> f64 {
        if self.config.fixed_amount > 0.0 {
            return self.config.fixed_amount.min(self.config.max_position_size);
        }

        if self.config.balance_pct > 0.0 {
            let quote_currency = self.extract_quote_currency();

            match exchange.get_balances().await {
                Ok(balances) => {
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
                        self.strategy_id,
                        quote_currency,
                        self.config.balance_pct * 100.0,
                        amount,
                        capped
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

    fn extract_quote_currency(&self) -> String {
        let sym = &self.symbol;

        if let Some(pos) = sym.find('/') {
            return sym[pos + 1..].to_uppercase();
        }

        if let Some(pos) = sym.find('_') {
            return sym[pos + 1..].to_uppercase();
        }

        let common_quotes = [
            "USDC", "USDT", "BUSD", "TUSD", "DAI", "USD", "EUR", "BTC", "ETH", "BNB",
        ];

        let upper = sym.to_uppercase();
        for quote in &common_quotes {
            if upper.ends_with(quote) {
                return quote.to_string();
            }
        }

        if upper.len() >= 7 {
            upper[upper.len() - 4..].to_string()
        } else if upper.len() >= 4 {
            upper[upper.len() - 3..].to_string()
        } else {
            "USDT".to_string()
        }
    }

    /// Return position info for all currently held positions.
    /// In hedge mode, can return both long and short simultaneously.
    pub fn position_info(&self) -> Vec<PositionInfo> {
        let mut result = Vec::new();

        if let Some(ref pos) = self.long_position {
            result.push(PositionInfo {
                side: PositionSide::Long,
                size: pos.size,
                entry_price: pos.entry_price,
            });
        }
        if let Some(ref pos) = self.short_position {
            result.push(PositionInfo {
                side: PositionSide::Short,
                size: pos.size,
                entry_price: pos.entry_price,
            });
        }

        result
    }

    pub fn cached_quote_balance(&self) -> f64 {
        self.cached_quote_balance
    }

    /// Restore a position for a specific side (hedge mode).
    /// Can be called for both Long and Short independently.
    pub fn restore_position(
        &mut self,
        side: PositionSide,
        size: f64,
        entry_price: f64,
    ) {
        if size <= 0.0 || entry_price <= 0.0 {
            warn!(
                "[Strategy {}] Invalid restore_position params: side={:?}, size={}, entry_price={}. Skipping.",
                self.strategy_id, side, size, entry_price
            );
            return;
        }
        info!(
            "[Strategy {}] Restoring position: side={:?}, size={:.6}, entry={:.2}",
            self.strategy_id, side, size, entry_price
        );
        let pos_data = PositionData {
            size,
            entry_price,
            peak_price: entry_price,
            trough_price: entry_price,
        };
        match side {
            PositionSide::Long => self.long_position = Some(pos_data),
            PositionSide::Short => self.short_position = Some(pos_data),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PositionInfo {
    pub side: PositionSide,
    pub size: f64,
    pub entry_price: f64,
}
