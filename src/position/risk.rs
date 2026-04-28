use super::config::RiskConfig;
use super::error::{PositionEngineError, Result};
use super::types::Position;

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
// RiskChecker
// ============================================================================

/// 风控检查器
pub struct RiskChecker {
    config: RiskConfig,
    consecutive_losses: u32,
}

impl RiskChecker {
    pub fn new(config: RiskConfig) -> Self {
        Self {
            config,
            consecutive_losses: 0,
        }
    }

    // -----------------------------------------------------------------------
    // 开仓前风控检查
    // -----------------------------------------------------------------------

    /// 开仓前风控检查，返回 `Ok(())` 或 `Err(RiskCheckFailed)`。
    ///
    /// 检查项：
    /// 1. 杠杆上限
    /// 2. 单品种仓位上限
    /// 3. 总仓位上限
    /// 4. 可用保证金（总仓位 + 新仓位 <= 上限）
    pub fn check_open_position(
        &self,
        current_positions: &[&Position],
        new_symbol: &str,
        new_amount: f64,
        new_leverage: u32,
        total_equity: f64,
    ) -> Result<()> {
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
        let symbol_exposure: f64 = current_positions
            .iter()
            .filter(|p| p.symbol == new_symbol)
            .map(|p| p.size * p.entry_price / p.leverage as f64)
            .sum::<f64>();

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
        let total_exposure: f64 = current_positions
            .iter()
            .map(|p| p.size * p.entry_price / p.leverage as f64)
            .sum::<f64>();

        let total_limit = total_equity * self.config.max_total_position_pct;

        if total_exposure + new_margin > total_limit {
            return Err(PositionEngineError::RiskCheckFailed {
                reason: format!(
                    "总仓位 {} + 新仓 {} 超过总仓位上限 {}",
                    total_exposure, new_margin, total_limit
                ),
            });
        }

        // 4. 检查可用保证金（简单判断：总仓位 + 新仓位 <= 上限）
        //    与第 3 步逻辑一致，但此处额外考虑连续亏损减仓
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
        current_positions: &[&Position],
        symbol: &str,
        amount: f64,
        total_equity: f64,
    ) -> Result<()> {
        let max_amount = total_equity * self.config.max_order_amount_pct;

        if amount > max_amount {
            return Err(PositionEngineError::RiskCheckFailed {
                reason: format!(
                    "品种 {} 下单金额 {} 超过单笔上限 {}",
                    symbol, amount, max_amount
                ),
            });
        }

        // 如果该品种没有对应仓位，则不允许 reduce_only 之外的下单
        // （此处仅做金额检查，仓位存在性由上层逻辑保证）

        let _ = current_positions; // 预留扩展
        Ok(())
    }

    // -----------------------------------------------------------------------
    // 资金费率检查
    // -----------------------------------------------------------------------

    /// 检查资金费率（同步循环中调用）。
    ///
    /// 费率超过阈值返回 `Some(RiskAlertInfo)`，否则返回 `None`。
    pub fn check_funding_rate(&self, symbol: &str, rate: f64) -> Option<RiskAlertInfo> {
        if rate.abs() > self.config.funding_rate_threshold {
            let severity = if rate.abs() > self.config.funding_rate_threshold * 2.0 {
                "critical"
            } else {
                "warning"
            };

            Some(RiskAlertInfo {
                symbol: symbol.to_string(),
                alert_type: "funding_rate".to_string(),
                severity: severity.to_string(),
                message: format!(
                    "品种 {} 资金费率 {} 超过阈值 {}",
                    symbol, rate, self.config.funding_rate_threshold
                ),
            })
        } else {
            None
        }
    }

    // -----------------------------------------------------------------------
    // 回撤检查
    // -----------------------------------------------------------------------

    /// 检查回撤（同步循环中调用）。
    ///
    /// 根据回撤比例返回对应的动作：
    /// - `None` / `Normal`：正常
    /// - `Warning`：回撤超过 50% 的最大回撤阈值
    /// - `Pause`：回撤超过 75% 的最大回撤阈值
    /// - `CloseAll`：回撤超过最大回撤阈值
    pub fn check_drawdown(&self, peak_equity: f64, current_equity: f64) -> Option<DrawdownAction> {
        if peak_equity <= 0.0 {
            return None;
        }

        let drawdown = (peak_equity - current_equity) / peak_equity;
        let max_dd = self.config.max_drawdown_pct;

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

    // -----------------------------------------------------------------------
    // 强平预警
    // -----------------------------------------------------------------------

    /// 检查强平预警。
    ///
    /// 返回 `Some(距离强平价百分比)` 表示接近强平，`None` 表示安全。
    pub fn check_liquidation(&self, position: &Position) -> Option<f64> {
        let liq_price = match position.liquidation_price {
            Some(p) => p,
            None => return None,
        };

        let current = position.current_price;
        if current <= 0.0 || liq_price <= 0.0 {
            return None;
        }

        let distance_pct = (current - liq_price).abs() / current;

        if distance_pct <= self.config.liquidation_buffer_pct {
            Some(distance_pct)
        } else {
            None
        }
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

    /// 获取当前连续亏损次数。
    pub fn consecutive_losses(&self) -> u32 {
        self.consecutive_losses
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::config::RiskConfig;
    use crate::position::types::{Position, PositionSide, PositionStatus};
    use chrono::Utc;
    use uuid::Uuid;

    fn make_position(symbol: &str, size: f64, entry_price: f64, leverage: u32) -> Position {
        Position {
            id: Uuid::new_v4(),
            engine_id: "test".to_string(),
            strategy_id: None,
            exchange: "test".to_string(),
            symbol: symbol.to_string(),
            side: PositionSide::Long,
            status: PositionStatus::Open,
            size,
            entry_price,
            current_price: entry_price,
            leverage,
            margin: size * entry_price / leverage as f64,
            unrealized_pnl: 0.0,
            realized_pnl: 0.0,
            stop_loss: None,
            take_profit: None,
            liquidation_price: None,
            opened_at: Utc::now(),
            updated_at: Utc::now(),
            closed_at: None,
            metadata: serde_json::Value::Null,
        }
    }

    #[test]
    fn test_leverage_check() {
        let config = RiskConfig::default();
        let checker = RiskChecker::new(config);
        let result = checker.check_open_position(&[], "BTCUSDT", 1.0, 50, 10000.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_normal_open() {
        let config = RiskConfig::default();
        let checker = RiskChecker::new(config);
        let result = checker.check_open_position(&[], "BTCUSDT", 0.1, 5, 10000.0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_consecutive_losses() {
        let config = RiskConfig::default();
        let mut checker = RiskChecker::new(config);
        assert!(!checker.should_reduce_position());

        for _ in 0..5 {
            checker.record_trade_result(-1.0);
        }
        assert!(checker.should_reduce_position());

        checker.record_trade_result(1.0);
        assert!(!checker.should_reduce_position());
    }

    #[test]
    fn test_funding_rate_check() {
        let config = RiskConfig::default();
        let checker = RiskChecker::new(config);

        // 低于阈值
        assert!(checker.check_funding_rate("BTCUSDT", 0.0005).is_none());

        // 超过阈值
        assert!(checker.check_funding_rate("BTCUSDT", 0.002).is_some());
    }

    #[test]
    fn test_drawdown_check() {
        let config = RiskConfig::default();
        let checker = RiskChecker::new(config);

        // 无回撤
        assert!(checker.check_drawdown(10000.0, 10000.0).is_none());

        // 轻微回撤
        assert!(checker.check_drawdown(10000.0, 9500.0).is_none());

        // 超过最大回撤
        let action = checker.check_drawdown(10000.0, 8000.0);
        assert_eq!(action, Some(DrawdownAction::CloseAll));
    }

    #[test]
    fn test_liquidation_check() {
        let config = RiskConfig::default();
        let checker = RiskChecker::new(config);

        let mut pos = make_position("BTCUSDT", 1.0, 50000.0, 10);
        pos.liquidation_price = Some(45000.0);
        pos.current_price = 46000.0;

        // 距离强平 (50000-45000)/50000 = 10%，超过 buffer 20%，安全
        assert!(checker.check_liquidation(&pos).is_none());

        pos.current_price = 45500.0;
        // 距离强平 (45500-45000)/45500 ≈ 1.1%，低于 buffer 20%，预警
        assert!(checker.check_liquidation(&pos).is_some());
    }
}
