

pub fn format_position_info(
    position: &virs_type::Position,
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

/* 计算止损价：基于ATR的1.5倍波动幅度，ATR无效时回退到入场价3%止损 */
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

/* 计算止盈价：基于ATR的3.0倍波动幅度，ATR无效时回退到入场价6%止盈 */
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

/* 计算仓位百分比：ADX越高仓位越大（趋势强），连续亏损或高资金费率时减半，最终限制在10%-100% */
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

/* 计算冷却时间：止损后同方向冷却30分钟，止盈后同方向冷却15分钟，反方向不冷却 */
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
