//! Auto 策略交易数学计算。
//!
//! Prompt 渲染已统一到 [`virs_tactical_bot::prompt::render`]，
//! 本模块仅保留 Auto 专属的交易数学函数（止损止盈、移动止损、仓位百分比、冷却时间）
//! 和持仓格式化辅助函数。

pub fn format_position_info(
    position: &virs_type::position::Position,
    current_side: Option<&str>,
    current_price: f64,
) -> String {
    match current_side {
        Some(side) if !side.is_empty() && side != "none" => {
            let unrealized_pnl = position.unrealized_pnl_at(current_price);
            let pnl_pct = if position.entry_price > 0.0 {
                unrealized_pnl / (position.entry_price * position.quantity) * 100.0
            } else {
                0.0
            };
            format!(
                "- 方向：{}\n- 入场价：{:.2}\n- 持仓量：{:.6}\n- 当前价：{:.2}\n- 未实现盈亏：{:.4} USDT ({:+.2}%)",
                side, position.entry_price, position.quantity, current_price, unrealized_pnl, pnl_pct
            )
        }
        _ => "无仓位".to_string(),
    }
}

pub fn format_stop_take_profit(stop_loss: f64, take_profit: f64) -> String {
    if stop_loss <= 0.0 && take_profit <= 0.0 {
        return "未设置".to_string();
    }
    let mut s = String::new();
    if stop_loss > 0.0 {
        s.push_str(&format!("- 止损价：{:.2}", stop_loss));
    }
    if take_profit > 0.0 {
        if !s.is_empty() {
            s.push('\n');
        }
        s.push_str(&format!("- 止盈价：{:.2}", take_profit));
    }
    s
}

pub fn compute_stop_loss(entry_price: f64, side: &str, atr: f64) -> f64 {
    if atr <= 0.0 || entry_price <= 0.0 {
        return entry_price * 0.97;
    }
    match side {
        "long" => entry_price - 1.5 * atr,
        "short" => entry_price + 1.5 * atr,
        _ => entry_price * 0.97,
    }
}

pub fn compute_take_profit(entry_price: f64, side: &str, atr: f64) -> f64 {
    if atr <= 0.0 || entry_price <= 0.0 {
        return entry_price * 1.06;
    }
    match side {
        "long" => entry_price + 3.0 * atr,
        "short" => entry_price - 3.0 * atr,
        _ => entry_price * 1.06,
    }
}

pub fn compute_trailing_stop(
    entry_price: f64,
    current_price: f64,
    side: &str,
    atr: f64,
    current_stop: f64,
) -> f64 {
    if atr <= 0.0 || entry_price <= 0.0 {
        return current_stop;
    }
    match side {
        "long" => {
            let profit_atr = (current_price - entry_price) / atr;
            let new_stop = if profit_atr >= 2.0 {
                current_price - 1.0 * atr
            } else if profit_atr >= 1.0 {
                entry_price
            } else {
                return current_stop;
            };
            if new_stop > current_stop {
                new_stop
            } else {
                current_stop
            }
        }
        "short" => {
            let profit_atr = (entry_price - current_price) / atr;
            let new_stop = if profit_atr >= 2.0 {
                current_price + 1.0 * atr
            } else if profit_atr >= 1.0 {
                entry_price
            } else {
                return current_stop;
            };
            if new_stop < current_stop || current_stop <= 0.0 {
                new_stop
            } else {
                current_stop
            }
        }
        _ => current_stop,
    }
}

pub fn compute_position_pct(adx: f64, consecutive_losses: i32, funding_rate: f64) -> f64 {
    let base: f64 = if adx >= 25.0 {
        80.0
    } else if adx >= 20.0 {
        60.0
    } else {
        40.0
    };
    let after_loss: f64 = if consecutive_losses >= 2 {
        base * 0.5
    } else {
        base
    };
    let after_funding: f64 = if funding_rate.abs() > 0.001 {
        after_loss * 0.5
    } else {
        after_loss
    };
    after_funding.clamp(10.0, 100.0)
}

pub fn compute_cooldown_secs(closed_side: &str, reason: &str, new_side: &str) -> i64 {
    match reason {
        "stop_loss" => {
            if closed_side == new_side {
                30 * 60
            } else {
                0
            }
        }
        "take_profit" => {
            if closed_side == new_side {
                15 * 60
            } else {
                0
            }
        }
        _ => 15 * 60,
    }
}
