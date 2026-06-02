use crate::bot::common::indicators::MarketIndicators;
use crate::bot::auto_trade::types::DEFAULT_USER_PROMPT_TEMPLATE;

pub struct PromptContext {
    pub timestamp: String,
    pub symbol: String,
    pub exchange: String,
    pub market_type: String,
    pub total_balance: f64,
    pub available_balance: f64,
    pub used_margin: f64,
    pub margin_usage_rate: f64,
    pub leverage: i32,
    pub position_info: String,
    pub position_duration: String,
    pub stop_take_profit_info: String,
    pub funding_rate: f64,
    pub funding_next_time: String,
    pub total_trades: i32,
    pub win_trades: i32,
    pub loss_trades: i32,
    pub total_pnl: f64,
    pub consecutive_losses: i32,
    pub trigger_reason: String,
    pub ind: MarketIndicators,
}

pub fn render_prompt(template: &str, ctx: &PromptContext) -> String {
    let h1_ema_cross = if ctx.ind.ema20 > ctx.ind.ema50 { "金叉(多头)" } else { "死叉(空头)" };
    let m15_ema_cross = if ctx.ind.m15_ema20 > ctx.ind.m15_ema50 { "金叉(多头)" } else { "死叉(空头)" };
    let h1_ema_cross_bars = if ctx.ind.h1_ema_cross_bars_ago >= 0 {
        ctx.ind.h1_ema_cross_bars_ago.to_string()
    } else {
        "无近期交叉".to_string()
    };
    let m15_ema_cross_bars = if ctx.ind.m15_ema_cross_bars_ago >= 0 {
        ctx.ind.m15_ema_cross_bars_ago.to_string()
    } else {
        "无近期交叉".to_string()
    };

    template
        .replace("{timestamp}", &ctx.timestamp)
        .replace("{symbol}", &ctx.symbol)
        .replace("{exchange}", &ctx.exchange)
        .replace("{market_type}", &ctx.market_type)
        .replace("{total_balance}", &format!("{:.2}", ctx.total_balance))
        .replace("{available_balance}", &format!("{:.2}", ctx.available_balance))
        .replace("{used_margin}", &format!("{:.2}", ctx.used_margin))
        .replace("{margin_usage_rate}", &format!("{:.1}", ctx.margin_usage_rate * 100.0))
        .replace("{leverage}", &ctx.leverage.to_string())
        .replace("{position_info}", &ctx.position_info)
        .replace("{position_duration}", &ctx.position_duration)
        .replace("{stop_take_profit_info}", &ctx.stop_take_profit_info)
        .replace("{funding_rate}", &format!("{:.4}%", ctx.funding_rate * 100.0))
        .replace("{funding_next_time}", &ctx.funding_next_time)
        .replace("{h4_ema20}", &format!("{:.2}", ctx.ind.h4_ema20))
        .replace("{h4_ema50}", &format!("{:.2}", ctx.ind.h4_ema50))
        .replace("{h4_rsi}", &format!("{:.2}", ctx.ind.h4_rsi))
        .replace("{h4_macd_histogram}", &format!("{:.4}", ctx.ind.h4_macd_histogram))
        .replace("{h4_adx}", &format!("{:.2}", ctx.ind.h4_adx))
        .replace("{h1_current_price}", &format!("{:.2}", ctx.ind.current_price))
        .replace("{h1_ema20}", &format!("{:.2}", ctx.ind.ema20))
        .replace("{h1_ema50}", &format!("{:.2}", ctx.ind.ema50))
        .replace("{h1_ema_cross}", h1_ema_cross)
        .replace("{h1_ema_cross_bars_ago}", &h1_ema_cross_bars)
        .replace("{h1_ema_gap_pct}", &format!("{:.2}", ctx.ind.h1_ema_gap_pct))
        .replace("{h1_ema_gap_trend}", &ctx.ind.h1_ema_gap_trend)
        .replace("{h1_rsi}", &format!("{:.2}", ctx.ind.rsi))
        .replace("{h1_macd}", &format!("{:.4}", ctx.ind.macd))
        .replace("{h1_macd_signal}", &format!("{:.4}", ctx.ind.macd_signal))
        .replace("{h1_macd_histogram}", &format!("{:.4}", ctx.ind.macd_histogram))
        .replace("{h1_adx}", &format!("{:.2}", ctx.ind.adx))
        .replace("{h1_atr}", &format!("{:.4}", ctx.ind.atr))
        .replace("{h1_bb_upper}", &format!("{:.2}", ctx.ind.bb_upper))
        .replace("{h1_bb_middle}", &format!("{:.2}", ctx.ind.bb_middle))
        .replace("{h1_bb_lower}", &format!("{:.2}", ctx.ind.bb_lower))
        .replace("{h1_bb_width_pct}", &format!("{:.2}", ctx.ind.bb_width * 100.0))
        .replace("{h1_change}", &format!("{:+.2}", ctx.ind.change_1h))
        .replace("{h1_volume}", &format!("{:.2}", ctx.ind.h1_volume))
        .replace("{h1_volume_sma20}", &format!("{:.2}", ctx.ind.h1_volume_sma20))
        .replace("{h1_high_50}", &format!("{:.2}", ctx.ind.h1_high_50))
        .replace("{h1_low_50}", &format!("{:.2}", ctx.ind.h1_low_50))
        .replace("{m15_current_price}", &format!("{:.2}", ctx.ind.m15_current_price))
        .replace("{m15_ema20}", &format!("{:.2}", ctx.ind.m15_ema20))
        .replace("{m15_ema50}", &format!("{:.2}", ctx.ind.m15_ema50))
        .replace("{m15_ema_cross}", m15_ema_cross)
        .replace("{m15_ema_cross_bars_ago}", &m15_ema_cross_bars)
        .replace("{m15_rsi}", &format!("{:.2}", ctx.ind.m15_rsi))
        .replace("{m15_macd}", &format!("{:.4}", ctx.ind.m15_macd))
        .replace("{m15_macd_signal}", &format!("{:.4}", ctx.ind.m15_macd_signal))
        .replace("{m15_macd_histogram}", &format!("{:.4}", ctx.ind.m15_macd_histogram))
        .replace("{m15_atr}", &format!("{:.4}", ctx.ind.m15_atr))
        .replace("{m15_adx}", &format!("{:.2}", ctx.ind.m15_adx))
        .replace("{m15_volume}", &format!("{:.2}", ctx.ind.m15_volume))
        .replace("{m15_volume_sma20}", &format!("{:.2}", ctx.ind.m15_volume_sma20))
        .replace("{m15_high_50}", &format!("{:.2}", ctx.ind.m15_high_50))
        .replace("{m15_low_50}", &format!("{:.2}", ctx.ind.m15_low_50))
        .replace("{total_trades}", &ctx.total_trades.to_string())
        .replace("{win_trades}", &ctx.win_trades.to_string())
        .replace("{loss_trades}", &ctx.loss_trades.to_string())
        .replace("{total_pnl}", &format!("{:.2}", ctx.total_pnl))
        .replace("{consecutive_losses}", &ctx.consecutive_losses.to_string())
        .replace("{trigger_reason}", &ctx.trigger_reason)
}

pub fn format_position_info(
    current_side: Option<&str>,
    entry_price: f64,
    position_size: f64,
    current_price: f64,
) -> String {
    match current_side {
        Some(side) if !side.is_empty() && side != "none" => {
            let unrealized_pnl = if side == "long" {
                (current_price - entry_price) * position_size
            } else {
                (entry_price - current_price) * position_size
            };
            let pnl_pct = if entry_price > 0.0 {
                unrealized_pnl / (entry_price * position_size) * 100.0
            } else {
                0.0
            };
            format!(
                "- 方向：{}\n- 入场价：{:.2}\n- 持仓量：{:.6}\n- 当前价：{:.2}\n- 未实现盈亏：{:.4} USDT ({:+.2}%)",
                side, entry_price, position_size, current_price, unrealized_pnl, pnl_pct
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
        if !s.is_empty() { s.push('\n'); }
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
            if new_stop > current_stop { new_stop } else { current_stop }
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
            if new_stop < current_stop || current_stop <= 0.0 { new_stop } else { current_stop }
        }
        _ => current_stop,
    }
}

pub fn compute_leverage(adx: f64, is_spot: bool) -> i32 {
    if is_spot { return 1; }
    if adx >= 30.0 { 5 }
    else if adx >= 20.0 { 3 }
    else { 2 }
}

pub fn compute_position_pct(adx: f64, consecutive_losses: i32, funding_rate: f64) -> f64 {
    let base: f64 = if adx >= 25.0 { 80.0 } else if adx >= 20.0 { 60.0 } else { 40.0 };
    let after_loss: f64 = if consecutive_losses >= 2 { base * 0.5 } else { base };
    let after_funding: f64 = if funding_rate.abs() > 0.001 { after_loss * 0.5 } else { after_loss };
    after_funding.clamp(10.0, 100.0)
}

pub fn default_template() -> &'static str {
    DEFAULT_USER_PROMPT_TEMPLATE
}
