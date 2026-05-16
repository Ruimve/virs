use crate::bot::semi_automatic_grid::types::DEFAULT_USER_PROMPT_TEMPLATE;
use crate::bot::semi_automatic_grid::utils::indicators::MarketIndicators;

/// Prompt 模板渲染所需的上下文数据
///
/// 包含所有占位符的值，用于将模板字符串渲染为最终 prompt
pub struct PromptContext {
    pub timestamp: String,
    pub symbol: String,
    pub total_balance: f64,
    pub available_balance: f64,
    pub used_margin: f64,
    pub margin_usage_rate: f64,
    pub leverage: i32,
    pub grid_status: String,
    pub last_adjust_time: String,
    pub consecutive_losses: i32,
    pub current_grid_config: String,
    pub position_base: f64,
    pub position_side: String,
    pub entry_price: f64,
    pub unrealized_pnl: f64,
    pub open_orders: String,
    pub funding_rate: f64,
    pub funding_next_time: String,
    pub event_flag: bool,
    pub event_description: String,
    pub trigger_reason: String,
    pub ind: MarketIndicators,
}

/// 将 PromptContext 中的值替换到模板字符串中
///
/// 遍历所有占位符 {xxx} 并用 context 中对应的值替换
pub fn render_prompt(template: &str, ctx: &PromptContext) -> String {
    let h1_atr_sma20_str = if ctx.ind.h1_atr_sma20.is_nan() { "N/A".to_string() } else { format!("{:.4}", ctx.ind.h1_atr_sma20) };
    let m15_atr_sma20_str = if ctx.ind.m15_atr_sma20.is_nan() { "N/A".to_string() } else { format!("{:.4}", ctx.ind.m15_atr_sma20) };
    let ema_distance_pct = if ctx.ind.ema50 > 0.0 {
        (ctx.ind.ema20 - ctx.ind.ema50) / ctx.ind.ema50
    } else {
        0.0
    };

    template
        .replace("{timestamp}", &ctx.timestamp)
        .replace("{symbol}", &ctx.symbol)
        .replace("{total_balance}", &format!("{:.2}", ctx.total_balance))
        .replace("{available_balance}", &format!("{:.2}", ctx.available_balance))
        .replace("{used_margin}", &format!("{:.2}", ctx.used_margin))
        .replace("{margin_usage_rate}", &format!("{:.1}", ctx.margin_usage_rate * 100.0))
        .replace("{leverage}", &ctx.leverage.to_string())
        .replace("{grid_status}", &ctx.grid_status)
        .replace("{last_adjust_time}", &ctx.last_adjust_time)
        .replace("{consecutive_losses}", &ctx.consecutive_losses.to_string())
        .replace("{current_grid_config}", &ctx.current_grid_config)
        .replace("{position_base}", &format!("{:.6}", ctx.position_base))
        .replace("{position_side}", &ctx.position_side)
        .replace("{entry_price}", &format!("{:.2}", ctx.entry_price))
        .replace("{unrealized_pnl}", &format!("{:.2}", ctx.unrealized_pnl))
        .replace("{open_orders}", &ctx.open_orders)
        .replace("{funding_rate}", &format!("{:.6}", ctx.funding_rate))
        .replace("{funding_next_time}", &ctx.funding_next_time)
        .replace("{event_flag}", &ctx.event_flag.to_string())
        .replace("{event_description}", &ctx.event_description)
        .replace("{h1_current_price}", &format!("{:.2}", ctx.ind.current_price))
        .replace("{h1_bb_upper}", &format!("{:.2}", ctx.ind.bb_upper))
        .replace("{h1_bb_middle}", &format!("{:.2}", ctx.ind.bb_middle))
        .replace("{h1_bb_lower}", &format!("{:.2}", ctx.ind.bb_lower))
        .replace("{h1_bb_width_pct}", &format!("{:.2}", ctx.ind.bb_width * 100.0))
        .replace("{h1_ema20}", &format!("{:.2}", ctx.ind.ema20))
        .replace("{h1_ema50}", &format!("{:.2}", ctx.ind.ema50))
        .replace("{h1_ema_distance_pct}", &format!("{:+.2}", ema_distance_pct * 100.0))
        .replace("{h1_adx}", &format!("{:.2}", ctx.ind.adx))
        .replace("{h1_atr}", &format!("{:.4}", ctx.ind.atr))
        .replace("{h1_atr_sma20}", &h1_atr_sma20_str)
        .replace("{h1_candle_body}", &format!("{:+.4}", ctx.ind.h1_candle_body))
        .replace("{h1_bars_outside_band}", &ctx.ind.h1_bars_outside_band.to_string())
        .replace("{h1_bandwidth_5bars_ago}", &format!("{:.2}", ctx.ind.h1_bandwidth_5bars_ago * 100.0))
        .replace("{h1_high_20}", &format!("{:.2}", ctx.ind.h1_high_20))
        .replace("{h1_low_20}", &format!("{:.2}", ctx.ind.h1_low_20))
        .replace("{nearest_round_up}", &format!("{:.2}", ctx.ind.nearest_round_up))
        .replace("{nearest_round_down}", &format!("{:.2}", ctx.ind.nearest_round_down))
        .replace("{m15_current_price}", &format!("{:.2}", ctx.ind.m15_current_price))
        .replace("{m15_bb_width_pct}", &format!("{:.2}", ctx.ind.m15_bb_width_pct * 100.0))
        .replace("{m15_atr}", &format!("{:.4}", ctx.ind.m15_atr))
        .replace("{m15_atr_sma20}", &m15_atr_sma20_str)
        .replace("{m15_adx}", &format!("{:.2}", ctx.ind.m15_adx))
        .replace("{m15_bars_outside_band}", &ctx.ind.m15_bars_outside_band.to_string())
        .replace("{m15_ema20}", &format!("{:.2}", ctx.ind.m15_ema20))
        .replace("{m15_ema50}", &format!("{:.2}", ctx.ind.m15_ema50))
        .replace("{h4_ema20}", &format!("{:.2}", ctx.ind.h4_ema20))
        .replace("{h4_ema50}", &format!("{:.2}", ctx.ind.h4_ema50))
        .replace("{h4_adx}", &format!("{:.2}", ctx.ind.h4_adx))
        .replace("{h4_bb_width_pct}", &format!("{:.2}", ctx.ind.h4_bb_width_pct * 100.0))
        .replace("{trigger_reason}", &ctx.trigger_reason)
}

/// 构建网格配置的文本描述，用于 prompt 中 {current_grid_config} 占位符
///
/// 当网格状态为 empty 时返回 "none"，否则返回格式化的配置详情和层级表格
pub fn format_grid_config(
    grid_status: &str,
    upper_price: f64,
    lower_price: f64,
    grid_count: i32,
    grid_profit_pct: f64,
    quantity_per_grid: f64,
    levels: &[crate::bot::semi_automatic_grid::types::GridLevel],
) -> String {
    if grid_status == "empty" {
        return "none".to_string();
    }

    let mut md = String::new();
    md.push_str(&format!("- 上界价格：{:.2}\n", upper_price));
    md.push_str(&format!("- 下界价格：{:.2}\n", lower_price));
    md.push_str(&format!("- 网格数量：{}\n", grid_count));
    md.push_str(&format!("- 网格利润：{:.2}%\n", grid_profit_pct));
    md.push_str(&format!("- 每格金额：{:.2} USDT\n\n", quantity_per_grid));
    md.push_str("| 层级 | 价格 | 方向 | 状态 | 金额(USDT) | 持仓量 | 均价 |\n");
    md.push_str("|------|------|------|------|------------|--------|------|\n");
    for l in levels {
        let status = if l.side == "buy" {
            if l.buy_filled && l.sell_filled { "sold" } else if l.buy_filled && l.hold_quantity > 0.0 { "hold" } else { "buy" }
        } else {
            if l.sell_filled && l.buy_filled { "bought" } else if l.sell_filled && l.hold_quantity < 0.0 { "hold" } else { "sell" }
        };
        let avg_price = if l.avg_buy_price > 0.0 { l.avg_buy_price } else { l.buy_price };
        md.push_str(&format!(
            "| {} | {:.2} | {} | {} | {:.2} | {:.6} | {:.2} |\n",
            l.level, l.price, l.side, status, l.quantity * l.price, l.hold_quantity, avg_price
        ));
    }
    md
}

/// 构建简化版网格配置文本（仅含参数，不含层级表格）
///
/// 用于 API 层初始分析时，此时尚无运行时层级数据
pub fn format_grid_config_simple(
    grid_status: &str,
    upper_price: f64,
    lower_price: f64,
    grid_count: i32,
    grid_profit_pct: f64,
    quantity_per_grid: f64,
) -> String {
    if grid_status == "empty" {
        return "none".to_string();
    }
    format!(
        "- 上界价格：{:.2}\n- 下界价格：{:.2}\n- 网格数量：{}\n- 网格利润：{:.2}%\n- 每格金额：{:.2} USDT",
        upper_price, lower_price, grid_count, grid_profit_pct, quantity_per_grid
    )
}

/// 获取默认用户 prompt 模板
pub fn default_template() -> &'static str {
    DEFAULT_USER_PROMPT_TEMPLATE
}
