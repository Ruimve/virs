//! Risk checker for position engine.

use virs_types::position::{Position, PositionEngineError, PositionResult, RiskConfig};

// ============================================================================
// RiskAlertInfo
// ============================================================================

/// 风控告警信息
#[derive(Debug, Clone)]
pub struct RiskAlertInfo {
    /// 品种
    pub symbol: String,
    /// 告警类型: "funding_rate" / "drawdown" / "liquidation" / "consecutive_losses"
    pub alert_type: String,
    /// 严重程度: "warning" / "critical"
    pub severity: String,
    /// 告警消息
    pub message: String,
}

// ============================================================================
// DrawdownAction
// ============================================================================

/// 回撤处理动作
#[derive(Debug, Clone, PartialEq)]
pub enum DrawdownAction {
    /// 正常，无需处理
    Normal,
    /// 发出警告
    Warning,
    /// 暂停开新仓
    Pause,
    /// 平掉所有仓位
    CloseAll,
}

// ============================================================================
// Idempotent functions
// ============================================================================

/// 计算指定品种的仓位暴露（保证金占用）。
pub fn calc_symbol_exposure(positions: &[&Position], symbol: &str) -> f64 {
    positions
        .iter()
        .filter(|p| p.symbol == symbol)
        .map(|p| p.size * p.entry_price / p.leverage as f64)
        .sum()
}

/// 计算所有仓位的总暴露（保证金占用）。
pub fn calc_total_exposure(positions: &[&Position]) -> f64 {
    positions
        .iter()
        .map(|p| p.size * p.entry_price / p.leverage as f64)
        .sum()
}

/// 检查资金费率是否超过阈值，返回告警信息。
pub fn check_funding_rate(
    config: &RiskConfig,
    symbol: &str,
    rate: f64,
) -> Option<RiskAlertInfo> {
    if rate.abs() > config.funding_rate_threshold {
        let severity = if rate.abs() >= config.funding_rate_threshold * 2.0 {
            "critical"
        } else {
            "warning"
        };

        Some(RiskAlertInfo {
            symbol: symbol.to_string(),
            alert_type: "funding_rate".to_string(),
            severity: severity.to_string(),
            message: format!(
                "品种 {} 资金费率 {:.4}% 超过阈值 {:.4}%",
                symbol,
                rate * 100.0,
                config.funding_rate_threshold * 100.0
            ),
        })
    } else {
        None
    }
}

/// 检查回撤级别，返回处理动作。
pub fn check_drawdown(
    config: &RiskConfig,
    peak_equity: f64,
    current_equity: f64,
) -> Option<DrawdownAction> {
    if peak_equity <= 0.0 {
        return None;
    }

    let drawdown = (peak_equity - current_equity) / peak_equity;
    let max_dd = config.max_drawdown_pct;

    if drawdown >= max_dd {
        Some(DrawdownAction::CloseAll)
    } else if drawdown >= max_dd * 0.75 {
        Some(DrawdownAction::Pause)
    } else if drawdown >= max_dd * 0.5 {
        Some(DrawdownAction::Warning)
    } else {
        None
    }
}

/// 检查强平预警，返回到强平价的距离百分比。
pub fn check_liquidation(config: &RiskConfig, position: &Position) -> Option<f64> {
    let liq_price = match position.liquidation_price {
        Some(p) => p,
        None => return None,
    };

    let current = position.current_price;
    if current <= 0.0 || liq_price <= 0.0 {
        return None;
    }

    let distance_pct = (current - liq_price).abs() / current;

    if distance_pct <= config.liquidation_buffer_pct {
        Some(distance_pct)
    } else {
        None
    }
}

// ============================================================================
// RiskChecker
// ============================================================================

/// 风控检查器
pub struct RiskChecker {
    config: RiskConfig,
    consecutive_losses: u32,
}

impl RiskChecker {
    pub fn new(config: RiskConfig) -> Self {
        if let Err(msg) = config.validate() {
            tracing::warn!("RiskConfig validation failed: {}", msg);
        }
        Self {
            config,
            consecutive_losses: 0,
        }
    }

    // -----------------------------------------------------------------------
    // 开仓前风控检查
    // -----------------------------------------------------------------------

    /// 开仓前风控检查，返回 `Ok(())` 或 `Err(RiskCheckFailed)`。
    pub fn check_open_position(
        &self,
        current_positions: &[&Position],
        new_symbol: &str,
        new_amount: f64,
        new_leverage: u32,
        total_equity: f64,
    ) -> PositionResult<()> {
        // 1. 检查杠杆上限
        if new_leverage > self.config.max_leverage {
            return Err(PositionEngineError::RiskCheckFailed {
                reason: format!(
                    "杠杆 {} 超过上限 {}",
                    new_leverage, self.config.max_leverage
                ),
            });
        }

        // 2. 检查单品种仓位上限
        let symbol_exposure = calc_symbol_exposure(current_positions, new_symbol);

        let new_margin = new_amount / new_leverage as f64;
        let symbol_limit = total_equity * self.config.max_position_per_symbol_pct;

        if symbol_exposure + new_margin > symbol_limit {
            return Err(PositionEngineError::RiskCheckFailed {
                reason: format!(
                    "品种 {} 仓位 {} + 新仓 {} 超过单品种上限 {}",
                    new_symbol, symbol_exposure, new_margin, symbol_limit
                ),
            });
        }

        // 3. 检查总仓位上限
        let total_exposure = calc_total_exposure(current_positions);

        let total_limit = total_equity * self.config.max_total_position_pct;

        if total_exposure + new_margin > total_limit {
            return Err(PositionEngineError::RiskCheckFailed {
                reason: format!(
                    "总仓位 {} + 新仓 {} 超过总仓位上限 {}",
                    total_exposure, new_margin, total_limit
                ),
            });
        }

        // 4. 检查连续亏损
        if self.should_reduce_position() {
            return Err(PositionEngineError::RiskCheckFailed {
                reason: format!(
                    "连续亏损 {} 次达到阈值 {}，暂停开仓",
                    self.consecutive_losses, self.config.max_consecutive_losses
                ),
            });
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // 下单前风控检查
    // -----------------------------------------------------------------------

    /// 下单前风控检查：检查单笔下单金额上限。
    pub fn check_place_order(
        &self,
        _current_positions: &[&Position],
        symbol: &str,
        amount: f64,
        total_equity: f64,
    ) -> PositionResult<()> {
        let max_amount = total_equity * self.config.max_order_amount_pct;

        if amount > max_amount {
            return Err(PositionEngineError::RiskCheckFailed {
                reason: format!(
                    "品种 {} 下单金额 {} 超过单笔上限 {}",
                    symbol, amount, max_amount
                ),
            });
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // 资金费率检查
    // -----------------------------------------------------------------------

    /// 检查资金费率（同步循环中调用）。
    pub fn check_funding_rate(&self, symbol: &str, rate: f64) -> Option<RiskAlertInfo> {
        check_funding_rate(&self.config, symbol, rate)
    }

    // -----------------------------------------------------------------------
    // 回撤检查
    // -----------------------------------------------------------------------

    /// 检查回撤（同步循环中调用）。
    pub fn check_drawdown(&self, peak_equity: f64, current_equity: f64) -> Option<DrawdownAction> {
        check_drawdown(&self.config, peak_equity, current_equity)
    }

    // -----------------------------------------------------------------------
    // 强平预警
    // -----------------------------------------------------------------------

    /// 检查强平预警。
    pub fn check_liquidation(&self, position: &Position) -> Option<f64> {
        check_liquidation(&self.config, position)
    }

    // -----------------------------------------------------------------------
    // 连续亏损
    // -----------------------------------------------------------------------

    /// 记录一笔交易结果（用于连续亏损计数）。
    pub fn record_trade_result(&mut self, pnl: f64) {
        if pnl >= 0.0 {
            self.consecutive_losses = 0;
        } else {
            self.consecutive_losses += 1;
        }
    }

    /// 检查连续亏损是否需要减仓。
    pub fn should_reduce_position(&self) -> bool {
        self.consecutive_losses >= self.config.max_consecutive_losses
    }
}
