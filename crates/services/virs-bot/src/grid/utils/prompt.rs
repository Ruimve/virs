use crate::common::indicators::MarketIndicators;
use crate::grid::types::DEFAULT_USER_PROMPT_TEMPLATE;

pub fn render_user_prompt(
    indicators: &MarketIndicators,
    total_balance: f64,
    available_balance: f64,
    used_margin: f64,
    leverage: i32,
    grid_status: &str,
    last_adjust_time: &str,
    consecutive_losses: i32,
    current_grid_config: &str,
    position_info: &str,
    event_flag: bool,
    event_description: &str,
    trigger_reason: &str,
) -> String {
    let margin_usage_rate = if total_balance > 0.0 {
        used_margin / total_balance * 100.0
    } else {
        0.0
    };

    let mut prompt = DEFAULT_USER_PROMPT_TEMPLATE.to_string();

    prompt = prompt.replace("{timestamp}", &chrono::Utc::now().to_rfc3339());
    prompt = prompt.replace("{total_balance}", &format!("{:.2}", total_balance));
    prompt = prompt.replace("{available_balance}", &format!("{:.2}", available_balance));
    prompt = prompt.replace("{used_margin}", &format!("{:.2}", used_margin));
    prompt = prompt.replace("{margin_usage_rate}", &format!("{:.1}", margin_usage_rate));
    prompt = prompt.replace("{leverage}", &leverage.to_string());
    prompt = prompt.replace("{grid_status}", grid_status);
    prompt = prompt.replace("{last_adjust_time}", last_adjust_time);
    prompt = prompt.replace("{consecutive_losses}", &consecutive_losses.to_string());
    prompt = prompt.replace("{current_grid_config}", current_grid_config);
    prompt = prompt.replace("{position_info}", position_info);
    prompt = prompt.replace("{funding_rate}", &format!("{:.6}", indicators.funding_rate));
    prompt = prompt.replace("{funding_next_time}", &indicators.funding_next_time);
    prompt = prompt.replace("{event_flag}", if event_flag { "true" } else { "false" });
    prompt = prompt.replace("{event_description}", event_description);

    prompt = prompt.replace(
        "{h1_current_price}",
        &format!("{:.2}", indicators.current_price),
    );
    prompt = prompt.replace("{h1_bb_upper}", &format!("{:.2}", indicators.bb_upper));
    prompt = prompt.replace("{h1_bb_middle}", &format!("{:.2}", indicators.bb_middle));
    prompt = prompt.replace("{h1_bb_lower}", &format!("{:.2}", indicators.bb_lower));
    prompt = prompt.replace(
        "{h1_bb_width_pct}",
        &format!("{:.2}", indicators.bb_width * 100.0),
    );
    prompt = prompt.replace("{h1_ema20}", &format!("{:.2}", indicators.ema20));
    prompt = prompt.replace("{h1_ema50}", &format!("{:.2}", indicators.ema50));
    prompt = prompt.replace(
        "{h1_ema_distance_pct}",
        &format!("{:.2}", indicators.h1_ema_gap_pct),
    );
    prompt = prompt.replace("{h1_adx}", &format!("{:.1}", indicators.adx));
    prompt = prompt.replace("{h1_atr}", &format!("{:.4}", indicators.atr));
    prompt = prompt.replace("{h1_atr_sma20}", &format!("{:.4}", indicators.h1_atr_sma20));
    prompt = prompt.replace(
        "{h1_candle_body}",
        &format!("{:.4}", indicators.h1_candle_body),
    );
    prompt = prompt.replace(
        "{h1_bars_outside_band}",
        &format_bars_outside(indicators.h1_bars_outside_band),
    );
    prompt = prompt.replace(
        "{h1_bandwidth_5bars_ago}",
        &format!("{:.2}", indicators.h1_bandwidth_5bars_ago * 100.0),
    );
    prompt = prompt.replace("{h1_high_20}", &format!("{:.2}", indicators.h1_high_20));
    prompt = prompt.replace("{h1_low_20}", &format!("{:.2}", indicators.h1_low_20));
    prompt = prompt.replace(
        "{nearest_round_up}",
        &format!("{:.2}", indicators.nearest_round_up),
    );
    prompt = prompt.replace(
        "{nearest_round_down}",
        &format!("{:.2}", indicators.nearest_round_down),
    );

    prompt = prompt.replace(
        "{m15_current_price}",
        &format!("{:.2}", indicators.m15_current_price),
    );
    prompt = prompt.replace(
        "{m15_bb_width_pct}",
        &format!("{:.2}", indicators.m15_bb_width_pct * 100.0),
    );
    prompt = prompt.replace("{m15_atr}", &format!("{:.4}", indicators.m15_atr));
    prompt = prompt.replace(
        "{m15_atr_sma20}",
        &format!("{:.4}", indicators.m15_atr_sma20),
    );
    prompt = prompt.replace("{m15_adx}", &format!("{:.1}", indicators.m15_adx));
    prompt = prompt.replace(
        "{m15_bars_outside_band}",
        &format_bars_outside(indicators.m15_bars_outside_band),
    );
    prompt = prompt.replace("{m15_ema20}", &format!("{:.2}", indicators.m15_ema20));
    prompt = prompt.replace("{m15_ema50}", &format!("{:.2}", indicators.m15_ema50));

    prompt = prompt.replace("{h4_ema20}", &format!("{:.2}", indicators.h4_ema20));
    prompt = prompt.replace("{h4_ema50}", &format!("{:.2}", indicators.h4_ema50));
    prompt = prompt.replace("{h4_adx}", &format!("{:.1}", indicators.h4_adx));
    prompt = prompt.replace(
        "{h4_bb_width_pct}",
        &format!("{:.2}", indicators.h4_bb_width_pct * 100.0),
    );

    prompt = prompt.replace("{trigger_reason}", trigger_reason);

    prompt
}

pub fn format_bars_outside(count: i32) -> String {
    if count > 0 {
        format!("向上{}根", count)
    } else if count < 0 {
        format!("向下{}根", count.abs())
    } else {
        "无".to_string()
    }
}
