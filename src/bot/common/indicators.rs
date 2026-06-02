use crate::indicators;
use crate::models::Kline;

#[derive(Debug, Clone, Default)]
pub struct MarketIndicators {
    pub current_price: f64,
    pub rsi: f64,
    pub atr: f64,
    pub atr_pct: f64,
    pub bb_width: f64,
    pub bb_upper: f64,
    pub bb_middle: f64,
    pub bb_lower: f64,
    pub ema12: f64,
    pub ema20: f64,
    pub ema26: f64,
    pub ema50: f64,
    pub ema12_trend: String,
    pub ema20_trend: String,
    pub ema26_trend: String,
    pub ema50_trend: String,
    pub price_high: f64,
    pub price_low: f64,
    pub volatility: f64,
    pub change_1h: f64,
    pub change_4h: f64,
    pub change_24h: f64,
    pub macd: f64,
    pub macd_signal: f64,
    pub macd_histogram: f64,
    pub adx: f64,
    pub funding_rate: f64,
    pub funding_next_time: String,
    pub h1_atr_sma20: f64,
    pub h1_candle_body: f64,
    pub h1_bars_outside_band: i32,
    pub h1_bandwidth_5bars_ago: f64,
    pub h1_high_20: f64,
    pub h1_low_20: f64,
    pub nearest_round_up: f64,
    pub nearest_round_down: f64,
    pub h1_volume: f64,
    pub h1_volume_sma20: f64,
    pub h1_ema_cross_bars_ago: i32,
    pub h1_ema_gap_pct: f64,
    pub h1_ema_gap_trend: String,
    pub h1_high_50: f64,
    pub h1_low_50: f64,
    pub m15_current_price: f64,
    pub m15_rsi: f64,
    pub m15_macd: f64,
    pub m15_macd_signal: f64,
    pub m15_macd_histogram: f64,
    pub m15_bb_width_pct: f64,
    pub m15_atr: f64,
    pub m15_atr_sma20: f64,
    pub m15_adx: f64,
    pub m15_bars_outside_band: i32,
    pub m15_ema20: f64,
    pub m15_ema50: f64,
    pub m15_volume: f64,
    pub m15_volume_sma20: f64,
    pub m15_ema_cross_bars_ago: i32,
    pub m15_high_50: f64,
    pub m15_low_50: f64,
    pub h4_ema20: f64,
    pub h4_ema50: f64,
    pub h4_adx: f64,
    pub h4_bb_width_pct: f64,
    pub h4_rsi: f64,
    pub h4_macd: f64,
    pub h4_macd_signal: f64,
    pub h4_macd_histogram: f64,
}

fn ema_trend(current: f64, previous: f64) -> &'static str {
    if current > previous {
        "上升"
    } else if current < previous {
        "下降"
    } else {
        "横盘"
    }
}

pub fn compute_market_indicators(
    klines_1h: &[Kline],
    klines_4h: &[Kline],
    klines_15m: &[Kline],
    funding_rate: f64,
    funding_next_time: String,
) -> MarketIndicators {
    let last_idx = klines_1h.len().saturating_sub(1);
    let current_price = klines_1h.last().map(|k| k.close).unwrap_or(0.0);

    let rsi = indicators::rsi_at(klines_1h, last_idx, 14);
    let atr = indicators::atr_at(klines_1h, last_idx, 14);
    let atr_pct = if current_price > 0.0 { atr / current_price * 100.0 } else { 0.0 };
    let bb_width = indicators::bbands_width_at(klines_1h, last_idx, 20, 2.0);
    let (bb_upper, bb_middle, bb_lower) = indicators::bbands_at(klines_1h, last_idx, 20, 2.0);

    let ema12 = indicators::ema_at(klines_1h, last_idx, 12);
    let ema20 = indicators::ema_at(klines_1h, last_idx, 20);
    let ema26 = indicators::ema_at(klines_1h, last_idx, 26);
    let ema50 = if klines_1h.len() >= 50 {
        indicators::ema_at(klines_1h, last_idx, 50)
    } else {
        0.0
    };

    let lookback = 5.min(last_idx);
    let ema12_prev = indicators::ema_at(klines_1h, last_idx.saturating_sub(lookback), 12);
    let ema20_prev = indicators::ema_at(klines_1h, last_idx.saturating_sub(lookback), 20);
    let ema26_prev = indicators::ema_at(klines_1h, last_idx.saturating_sub(lookback), 26);
    let ema50_prev = if klines_1h.len() >= 50 + lookback {
        indicators::ema_at(klines_1h, last_idx.saturating_sub(lookback), 50)
    } else {
        ema50
    };

    let price_high: f64 = klines_1h.iter().map(|k| k.high).fold(f64::NEG_INFINITY, f64::max);
    let price_low: f64 = klines_1h.iter().map(|k| k.low).fold(f64::INFINITY, f64::min);

    let change_1h = if last_idx >= 1 && klines_1h[last_idx.saturating_sub(1)].close > 0.0 {
        (current_price - klines_1h[last_idx.saturating_sub(1)].close)
            / klines_1h[last_idx.saturating_sub(1)].close
            * 100.0
    } else {
        0.0
    };

    let change_4h = if last_idx >= 4 && klines_1h[last_idx.saturating_sub(4)].close > 0.0 {
        (current_price - klines_1h[last_idx.saturating_sub(4)].close)
            / klines_1h[last_idx.saturating_sub(4)].close
            * 100.0
    } else {
        0.0
    };

    let last_24: &[Kline] = if klines_1h.len() >= 24 {
        &klines_1h[klines_1h.len() - 24..]
    } else {
        klines_1h
    };
    let high_24: f64 = last_24.iter().map(|k| k.high).fold(f64::NEG_INFINITY, f64::max);
    let low_24: f64 = last_24.iter().map(|k| k.low).fold(f64::INFINITY, f64::min);
    let volatility = if low_24 > 0.0 {
        (high_24 - low_24) / low_24 * 100.0
    } else {
        0.0
    };
    let change_24h = if last_24.first().map(|k| k.close).unwrap_or(0.0) > 0.0 {
        (current_price - last_24.first().unwrap().close) / last_24.first().unwrap().close * 100.0
    } else {
        0.0
    };

    let macd = indicators::macd_at(klines_1h, last_idx, 12, 26);
    let macd_signal = indicators::macd_signal_at(klines_1h, last_idx, 12, 26, 9);
    let macd_histogram = indicators::macd_histogram_at(klines_1h, last_idx, 12, 26, 9);
    let adx = indicators::adx_at(klines_1h, last_idx, 14);

    let h1_atr_sma20 = if klines_1h.len() >= 20 {
        let atr_series = indicators::atr(klines_1h, 14);
        indicators::sma_at_from(&atr_series, last_idx, 20)
    } else {
        0.0
    };

    let h1_candle_body = klines_1h.last().map(|k| k.close - k.open).unwrap_or(0.0);

    let h1_bars_outside_band = indicators::compute_bars_outside_band(klines_1h, bb_upper, bb_lower);

    let h1_bandwidth_5bars_ago = if last_idx >= 5 {
        indicators::bbands_width_at(klines_1h, last_idx.saturating_sub(5), 20, 2.0)
    } else {
        0.0
    };

    let h1_high_20 = indicators::highest_at(klines_1h, last_idx, 20);
    let h1_low_20 = indicators::lowest_at(klines_1h, last_idx, 20);

    let nearest_round_up = indicators::find_round_number(current_price, true);
    let nearest_round_down = indicators::find_round_number(current_price, false);

    let h1_volume = klines_1h.last().map(|k| k.volume).unwrap_or(0.0);
    let h1_volume_sma20 = indicators::volume_sma_at(klines_1h, last_idx, 20);
    let h1_high_50 = indicators::highest_at(klines_1h, last_idx, 50);
    let h1_low_50 = indicators::lowest_at(klines_1h, last_idx, 50);

    let h1_ema_cross_bars_ago = compute_ema_cross_bars_ago(klines_1h, 20, 50, last_idx);

    let h1_ema_gap_pct = if ema50 != 0.0 { (ema20 - ema50) / ema50 * 100.0 } else { 0.0 };
    let h1_ema_gap_trend = {
        let curr_gap_abs = (ema20 - ema50).abs();
        let prev_gap_abs = (ema20_prev - ema50_prev).abs();
        if curr_gap_abs > prev_gap_abs * 1.01 {
            "扩大"
        } else if curr_gap_abs < prev_gap_abs * 0.99 {
            "缩小"
        } else {
            "持平"
        }
    };

    let h4_last = klines_4h.len().saturating_sub(1);
    let h4_ema20 = if !klines_4h.is_empty() { indicators::ema_at(klines_4h, h4_last, 20) } else { 0.0 };
    let h4_ema50 = if klines_4h.len() >= 50 { indicators::ema_at(klines_4h, h4_last, 50) } else { 0.0 };
    let h4_adx = if !klines_4h.is_empty() { indicators::adx_at(klines_4h, h4_last, 14) } else { 0.0 };
    let h4_bb_width_pct = if !klines_4h.is_empty() { indicators::bbands_width_at(klines_4h, h4_last, 20, 2.0) } else { 0.0 };
    let h4_rsi = if !klines_4h.is_empty() { indicators::rsi_at(klines_4h, h4_last, 14) } else { 0.0 };
    let h4_macd = if !klines_4h.is_empty() { indicators::macd_at(klines_4h, h4_last, 12, 26) } else { 0.0 };
    let h4_macd_signal = if !klines_4h.is_empty() { indicators::macd_signal_at(klines_4h, h4_last, 12, 26, 9) } else { 0.0 };
    let h4_macd_histogram = if !klines_4h.is_empty() { indicators::macd_histogram_at(klines_4h, h4_last, 12, 26, 9) } else { 0.0 };

    let m15_last = klines_15m.len().saturating_sub(1);
    let m15_current_price = klines_15m.last().map(|k| k.close).unwrap_or(current_price);
    let m15_rsi = if !klines_15m.is_empty() { indicators::rsi_at(klines_15m, m15_last, 14) } else { 0.0 };
    let m15_macd = if !klines_15m.is_empty() { indicators::macd_at(klines_15m, m15_last, 12, 26) } else { 0.0 };
    let m15_macd_signal = if !klines_15m.is_empty() { indicators::macd_signal_at(klines_15m, m15_last, 12, 26, 9) } else { 0.0 };
    let m15_macd_histogram = if !klines_15m.is_empty() { indicators::macd_histogram_at(klines_15m, m15_last, 12, 26, 9) } else { 0.0 };
    let m15_bb_width_pct = if !klines_15m.is_empty() { indicators::bbands_width_at(klines_15m, m15_last, 20, 2.0) } else { 0.0 };
    let m15_atr = if !klines_15m.is_empty() { indicators::atr_at(klines_15m, m15_last, 14) } else { 0.0 };
    let m15_atr_sma20 = if klines_15m.len() >= 20 {
        let atr_series = indicators::atr(klines_15m, 14);
        indicators::sma_at_from(&atr_series, m15_last, 20)
    } else {
        0.0
    };
    let m15_adx = if !klines_15m.is_empty() { indicators::adx_at(klines_15m, m15_last, 14) } else { 0.0 };
    let (m15_bb_upper, _, m15_bb_lower) = if !klines_15m.is_empty() {
        indicators::bbands_at(klines_15m, m15_last, 20, 2.0)
    } else {
        (0.0, 0.0, 0.0)
    };
    let m15_bars_outside_band = indicators::compute_bars_outside_band(klines_15m, m15_bb_upper, m15_bb_lower);
    let m15_ema20 = if !klines_15m.is_empty() { indicators::ema_at(klines_15m, m15_last, 20) } else { 0.0 };
    let m15_ema50 = if klines_15m.len() >= 50 { indicators::ema_at(klines_15m, m15_last, 50) } else { 0.0 };
    let m15_volume = klines_15m.last().map(|k| k.volume).unwrap_or(0.0);
    let m15_volume_sma20 = if !klines_15m.is_empty() { indicators::volume_sma_at(klines_15m, m15_last, 20) } else { 0.0 };
    let m15_high_50 = if !klines_15m.is_empty() { indicators::highest_at(klines_15m, m15_last, 50) } else { 0.0 };
    let m15_low_50 = if !klines_15m.is_empty() { indicators::lowest_at(klines_15m, m15_last, 50) } else { 0.0 };
    let m15_ema_cross_bars_ago = compute_ema_cross_bars_ago(klines_15m, 20, 50, m15_last);

    MarketIndicators {
        current_price,
        rsi,
        atr,
        atr_pct,
        bb_width,
        bb_upper,
        bb_middle,
        bb_lower,
        ema12,
        ema20,
        ema26,
        ema50,
        ema12_trend: ema_trend(ema12, ema12_prev).to_string(),
        ema20_trend: ema_trend(ema20, ema20_prev).to_string(),
        ema26_trend: ema_trend(ema26, ema26_prev).to_string(),
        ema50_trend: ema_trend(ema50, ema50_prev).to_string(),
        price_high,
        price_low,
        volatility,
        change_1h,
        change_4h,
        change_24h,
        macd,
        macd_signal,
        macd_histogram,
        adx,
        funding_rate,
        funding_next_time,
        h1_atr_sma20,
        h1_candle_body,
        h1_bars_outside_band,
        h1_bandwidth_5bars_ago,
        h1_high_20,
        h1_low_20,
        nearest_round_up,
        nearest_round_down,
        h1_volume,
        h1_volume_sma20,
        h1_ema_cross_bars_ago,
        h1_ema_gap_pct,
        h1_ema_gap_trend: h1_ema_gap_trend.to_string(),
        h1_high_50,
        h1_low_50,
        m15_current_price,
        m15_rsi,
        m15_macd,
        m15_macd_signal,
        m15_macd_histogram,
        m15_bb_width_pct,
        m15_atr,
        m15_atr_sma20,
        m15_adx,
        m15_bars_outside_band,
        m15_ema20,
        m15_ema50,
        m15_volume,
        m15_volume_sma20,
        m15_ema_cross_bars_ago,
        m15_high_50,
        m15_low_50,
        h4_ema20,
        h4_ema50,
        h4_adx,
        h4_bb_width_pct,
        h4_rsi,
        h4_macd,
        h4_macd_signal,
        h4_macd_histogram,
    }
}

fn compute_ema_cross_bars_ago(klines: &[Kline], fast_period: usize, slow_period: usize, last_idx: usize) -> i32 {
    if klines.len() < slow_period + 5 {
        return -1;
    }
    let lookback = 20.min(last_idx);
    for i in 0..lookback {
        let idx = last_idx - i;
        if idx < 1 { break; }
        let fast_curr = indicators::ema_at(klines, idx, fast_period);
        let slow_curr = indicators::ema_at(klines, idx, slow_period);
        let fast_prev = indicators::ema_at(klines, idx - 1, fast_period);
        let slow_prev = indicators::ema_at(klines, idx - 1, slow_period);
        if (fast_prev <= slow_prev && fast_curr > slow_curr)
            || (fast_prev >= slow_prev && fast_curr < slow_curr)
        {
            return i as i32;
        }
    }
    -1
}
