use talib_rs::{ma_type::MaType, math_operator, momentum, overlap, volatility};
use tracing::warn;
use virs_models::Kline;

pub fn closes(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.close).collect()
}

pub fn highs(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.high).collect()
}

pub fn lows(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.low).collect()
}

pub fn atr(klines: &[Kline], period: usize) -> Vec<f64> {
    volatility::atr(&highs(klines), &lows(klines), &closes(klines), period).unwrap_or_else(|e| {
        warn!(indicator = "atr", error = %e, "TA-Lib ATR calculation failed — returning empty series");
        Vec::new()
    })
}

pub fn sma_at_from(series: &[f64], idx: usize, period: usize) -> f64 {
    if series.is_empty() || period == 0 {
        return 0.0;
    }
    let nan_count = series.iter().take(idx + 1).filter(|v| v.is_nan()).count();
    let valid: Vec<f64> = series.iter().filter(|v| !v.is_nan()).copied().collect();
    if valid.len() < period {
        if valid.is_empty() {
            return 0.0;
        }
        return valid
            .iter()
            .rev()
            .take(period.min(valid.len()))
            .sum::<f64>()
            / period.min(valid.len()) as f64;
    }
    let mapped_idx = idx.saturating_sub(nan_count);
    let result = overlap::sma(&valid, period).unwrap_or_else(|e| {
        warn!(indicator = "sma_at_from", error = %e, "TA-Lib SMA calculation failed — returning empty series");
        Vec::new()
    });
    result.get(mapped_idx).copied().unwrap_or_else(|| {
        result.last().copied().unwrap_or_else(|| {
            warn!(
                indicator = "sma_at_from",
                idx, "Insufficient data for indicator calculation — defaulting to 0.0"
            );
            0.0
        })
    })
}

#[inline(always)]
pub fn ema_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if klines.is_empty() || idx < period - 1 || period == 0 {
        return 0.0;
    }
    let result = overlap::ema(&closes(klines), period).unwrap_or_else(|e| {
        warn!(indicator = "ema_at", error = %e, "TA-Lib EMA calculation failed — returning empty series");
        Vec::new()
    });
    result.get(idx).copied().unwrap_or_else(|| {
        warn!(
            indicator = "ema_at",
            idx, "Insufficient data for indicator calculation — defaulting to 0.0"
        );
        0.0
    })
}

#[inline(always)]
pub fn rsi_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if klines.is_empty() || idx < period || period == 0 {
        return 50.0;
    }
    let result = momentum::rsi(&closes(klines), period).unwrap_or_else(|e| {
        warn!(indicator = "rsi_at", error = %e, "TA-Lib RSI calculation failed — returning empty series");
        Vec::new()
    });
    result.get(idx).copied().unwrap_or(50.0)
}

#[inline(always)]
pub fn macd_at(klines: &[Kline], idx: usize, fast: usize, slow: usize) -> f64 {
    if klines.is_empty() || idx < slow - 1 {
        return 0.0;
    }
    let (macd, _, _) = momentum::macd(&closes(klines), fast, slow, 9).unwrap_or_else(|e| {
        warn!(indicator = "macd_at", error = %e, "TA-Lib MACD calculation failed — returning empty series");
        (Vec::new(), Vec::new(), Vec::new())
    });
    macd.get(idx).copied().unwrap_or_else(|| {
        warn!(
            indicator = "macd_at",
            idx, "Insufficient data for indicator calculation — defaulting to 0.0"
        );
        0.0
    })
}

#[inline(always)]
pub fn macd_signal_at(
    klines: &[Kline],
    idx: usize,
    fast: usize,
    slow: usize,
    signal: usize,
) -> f64 {
    if klines.is_empty() || idx < slow + signal - 2 {
        return 0.0;
    }
    let (_, sig, _) = momentum::macd(&closes(klines), fast, slow, signal).unwrap_or_else(|e| {
        warn!(indicator = "macd_signal_at", error = %e, "TA-Lib MACD calculation failed — returning empty series");
        (Vec::new(), Vec::new(), Vec::new())
    });
    sig.get(idx).copied().unwrap_or_else(|| {
        warn!(
            indicator = "macd_signal_at",
            idx, "Insufficient data for indicator calculation — defaulting to 0.0"
        );
        0.0
    })
}

#[inline(always)]
pub fn bbands_at(klines: &[Kline], idx: usize, period: usize, std_dev: f64) -> (f64, f64, f64) {
    if klines.is_empty() || idx < period - 1 {
        return (0.0, 0.0, 0.0);
    }
    let (upper, middle, lower) =
        overlap::bbands(&closes(klines), period, std_dev, std_dev, MaType::Sma).unwrap_or_else(|e| {
            warn!(indicator = "bbands_at", error = %e, "TA-Lib BBands calculation failed — returning empty series");
            (Vec::new(), Vec::new(), Vec::new())
        });
    (
        upper.get(idx).copied().unwrap_or_else(|| {
            warn!(
                indicator = "bbands_at.upper",
                idx, "Insufficient data for indicator calculation — defaulting to 0.0"
            );
            0.0
        }),
        middle.get(idx).copied().unwrap_or_else(|| {
            warn!(
                indicator = "bbands_at.middle",
                idx, "Insufficient data for indicator calculation — defaulting to 0.0"
            );
            0.0
        }),
        lower.get(idx).copied().unwrap_or_else(|| {
            warn!(
                indicator = "bbands_at.lower",
                idx, "Insufficient data for indicator calculation — defaulting to 0.0"
            );
            0.0
        }),
    )
}

#[inline(always)]
pub fn atr_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if klines.is_empty() || idx < period {
        return 0.0;
    }
    let result =
        volatility::atr(&highs(klines), &lows(klines), &closes(klines), period).unwrap_or_else(|e| {
            warn!(indicator = "atr_at", error = %e, "TA-Lib ATR calculation failed — returning empty series");
            Vec::new()
        });
    result.get(idx).copied().unwrap_or_else(|| {
        warn!(
            indicator = "atr_at",
            idx, "Insufficient data for indicator calculation — defaulting to 0.0"
        );
        0.0
    })
}

#[inline(always)]
pub fn adx_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if klines.is_empty() || idx < period * 2 {
        return 0.0;
    }
    let result =
        momentum::adx(&highs(klines), &lows(klines), &closes(klines), period).unwrap_or_else(|e| {
            warn!(indicator = "adx_at", error = %e, "TA-Lib ADX calculation failed — returning empty series");
            Vec::new()
        });
    result.get(idx).copied().unwrap_or_else(|| {
        warn!(
            indicator = "adx_at",
            idx, "Insufficient data for indicator calculation — defaulting to 0.0"
        );
        0.0
    })
}

#[inline(always)]
pub fn macd_histogram_at(
    klines: &[Kline],
    idx: usize,
    fast: usize,
    slow: usize,
    signal: usize,
) -> f64 {
    if klines.is_empty() || idx < slow + signal - 2 {
        return 0.0;
    }
    let (macd, sig, _) = momentum::macd(&closes(klines), fast, slow, signal).unwrap_or_else(|e| {
        warn!(indicator = "macd_histogram_at", error = %e, "TA-Lib MACD calculation failed — returning empty series");
        (Vec::new(), Vec::new(), Vec::new())
    });
    let m = macd.get(idx).copied().unwrap_or_else(|| {
        warn!(
            indicator = "macd_histogram_at.macd",
            idx, "Insufficient data for indicator calculation — defaulting to 0.0"
        );
        0.0
    });
    let s = sig.get(idx).copied().unwrap_or_else(|| {
        warn!(
            indicator = "macd_histogram_at.signal",
            idx, "Insufficient data for indicator calculation — defaulting to 0.0"
        );
        0.0
    });
    m - s
}

#[inline(always)]
pub fn bbands_width_at(klines: &[Kline], idx: usize, period: usize, std_dev: f64) -> f64 {
    let (upper, middle, lower) = bbands_at(klines, idx, period, std_dev);
    if middle == 0.0 {
        return 0.0;
    }
    (upper - lower) / middle
}

#[inline(always)]
pub fn highest_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if klines.is_empty() || idx < period - 1 || period == 0 {
        return 0.0;
    }
    let result = math_operator::max(&highs(klines), period).unwrap_or_else(|e| {
        warn!(indicator = "highest_at", error = %e, "TA-Lib MAX calculation failed — returning empty series");
        Vec::new()
    });
    result.get(idx).copied().unwrap_or_else(|| {
        warn!(
            indicator = "highest_at",
            idx, "Insufficient data for indicator calculation — defaulting to 0.0"
        );
        0.0
    })
}

#[inline(always)]
pub fn lowest_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if klines.is_empty() || idx < period - 1 || period == 0 {
        return 0.0;
    }
    let result = math_operator::min(&lows(klines), period).unwrap_or_else(|e| {
        warn!(indicator = "lowest_at", error = %e, "TA-Lib MIN calculation failed — returning empty series");
        Vec::new()
    });
    result.get(idx).copied().unwrap_or_else(|| {
        warn!(
            indicator = "lowest_at",
            idx, "Insufficient data for indicator calculation — defaulting to 0.0"
        );
        0.0
    })
}

pub fn volume_sma_at(klines: &[Kline], idx: usize, period: usize) -> f64 {
    if idx < period - 1 || klines.is_empty() {
        return 0.0;
    }
    let start = idx + 1 - period;
    let sum: f64 = (start..=idx).map(|i| klines[i].volume).sum();
    sum / period as f64
}

pub fn compute_bars_outside_band(klines: &[Kline], bb_upper: f64, bb_lower: f64) -> i32 {
    let mut count: i32 = 0;
    for k in klines.iter().rev() {
        if k.close > bb_upper {
            count += 1;
        } else if k.close < bb_lower {
            count -= 1;
        } else {
            break;
        }
    }
    count
}

pub fn find_round_number(price: f64, upward: bool) -> f64 {
    if price <= 0.0 {
        return 0.0;
    }
    let magnitude = 10_f64.powf(price.log10().floor());
    let step = if magnitude >= 10000.0 {
        1000.0
    } else if magnitude >= 1000.0 {
        100.0
    } else if magnitude >= 100.0 {
        10.0
    } else if magnitude >= 10.0 {
        5.0
    } else {
        1.0
    };
    if upward {
        (price / step).ceil() * step
    } else {
        (price / step).floor() * step
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
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
    pub macd: f64,
    pub macd_signal: f64,
    pub macd_histogram: f64,
    pub adx: f64,
    pub change_1h: f64,
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

    pub funding_rate: f64,
    pub funding_next_time: String,
}

pub fn compute_market_indicators(
    klines_1h: &[Kline],
    klines_4h: &[Kline],
    klines_15m: &[Kline],
    funding_rate: f64,
    funding_next_time: String,
) -> MarketIndicators {
    let last_idx = klines_1h.len().saturating_sub(1);
    let current_price = klines_1h.last().map(|k| k.close).unwrap_or_else(|| {
        warn!(
            indicator = "current_price",
            len = klines_1h.len(),
            "Insufficient data for indicator calculation — defaulting to 0.0"
        );
        0.0
    });

    let rsi = rsi_at(klines_1h, last_idx, 14);
    let atr_val = atr_at(klines_1h, last_idx, 14);
    let atr_pct = if current_price > 0.0 {
        atr_val / current_price * 100.0
    } else {
        0.0
    };
    let bb_width = bbands_width_at(klines_1h, last_idx, 20, 2.0);
    let (bb_upper, bb_middle, bb_lower) = bbands_at(klines_1h, last_idx, 20, 2.0);

    let ema12 = ema_at(klines_1h, last_idx, 12);
    let ema20 = ema_at(klines_1h, last_idx, 20);
    let ema26 = ema_at(klines_1h, last_idx, 26);
    let ema50 = if klines_1h.len() >= 50 {
        ema_at(klines_1h, last_idx, 50)
    } else {
        0.0
    };

    let change_1h = if last_idx >= 1 && klines_1h[last_idx.saturating_sub(1)].close > 0.0 {
        (current_price - klines_1h[last_idx.saturating_sub(1)].close)
            / klines_1h[last_idx.saturating_sub(1)].close
            * 100.0
    } else {
        0.0
    };

    let macd = macd_at(klines_1h, last_idx, 12, 26);
    let macd_signal = macd_signal_at(klines_1h, last_idx, 12, 26, 9);
    let macd_histogram = macd_histogram_at(klines_1h, last_idx, 12, 26, 9);
    let adx = adx_at(klines_1h, last_idx, 14);

    let h1_atr_sma20 = if klines_1h.len() >= 20 {
        let atr_series = atr(klines_1h, 14);
        sma_at_from(&atr_series, last_idx, 20)
    } else {
        0.0
    };

    let h1_candle_body = klines_1h
        .last()
        .map(|k| k.close - k.open)
        .unwrap_or_else(|| {
            warn!(
                indicator = "h1_candle_body",
                len = klines_1h.len(),
                "Insufficient data for indicator calculation — defaulting to 0.0"
            );
            0.0
        });
    let h1_bars_outside_band = compute_bars_outside_band(klines_1h, bb_upper, bb_lower);
    let h1_bandwidth_5bars_ago = if last_idx >= 5 {
        bbands_width_at(klines_1h, last_idx.saturating_sub(5), 20, 2.0)
    } else {
        0.0
    };
    let h1_high_20 = highest_at(klines_1h, last_idx, 20);
    let h1_low_20 = lowest_at(klines_1h, last_idx, 20);
    let nearest_round_up = find_round_number(current_price, true);
    let nearest_round_down = find_round_number(current_price, false);

    let h1_last_completed = klines_1h.len().saturating_sub(2);
    let h1_volume = klines_1h
        .get(h1_last_completed)
        .map(|k| k.volume)
        .unwrap_or_else(|| {
            warn!(
                indicator = "h1_volume",
                idx = h1_last_completed,
                "Insufficient data for indicator calculation — defaulting to 0.0"
            );
            0.0
        });
    let h1_volume_sma20 = if h1_last_completed >= 19 {
        volume_sma_at(klines_1h, h1_last_completed, 20)
    } else {
        0.0
    };
    let h1_high_50 = highest_at(klines_1h, last_idx, 50);
    let h1_low_50 = lowest_at(klines_1h, last_idx, 50);
    let h1_ema_cross_bars_ago = compute_ema_cross_bars_ago(klines_1h, 20, 50, last_idx);
    let h1_ema_gap_pct = if ema50 != 0.0 {
        (ema20 - ema50) / ema50 * 100.0
    } else {
        0.0
    };

    let lookback = 5.min(last_idx);
    let ema20_prev = ema_at(klines_1h, last_idx.saturating_sub(lookback), 20);
    let ema50_prev = if klines_1h.len() >= 50 + lookback {
        ema_at(klines_1h, last_idx.saturating_sub(lookback), 50)
    } else {
        ema50
    };
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
    }
    .to_string();

    let h4_last = klines_4h.len().saturating_sub(1);
    let h4_ema20 = if !klines_4h.is_empty() {
        ema_at(klines_4h, h4_last, 20)
    } else {
        0.0
    };
    let h4_ema50 = if klines_4h.len() >= 50 {
        ema_at(klines_4h, h4_last, 50)
    } else {
        0.0
    };
    let h4_adx = if !klines_4h.is_empty() {
        adx_at(klines_4h, h4_last, 14)
    } else {
        0.0
    };
    let h4_bb_width_pct = if !klines_4h.is_empty() {
        bbands_width_at(klines_4h, h4_last, 20, 2.0)
    } else {
        0.0
    };
    let h4_rsi = if !klines_4h.is_empty() {
        rsi_at(klines_4h, h4_last, 14)
    } else {
        0.0
    };
    let h4_macd = if !klines_4h.is_empty() {
        macd_at(klines_4h, h4_last, 12, 26)
    } else {
        0.0
    };
    let h4_macd_signal = if !klines_4h.is_empty() {
        macd_signal_at(klines_4h, h4_last, 12, 26, 9)
    } else {
        0.0
    };
    let h4_macd_histogram = if !klines_4h.is_empty() {
        macd_histogram_at(klines_4h, h4_last, 12, 26, 9)
    } else {
        0.0
    };

    let m15_last = klines_15m.len().saturating_sub(1);
    let m15_current_price = klines_15m.last().map(|k| k.close).unwrap_or(current_price);
    let m15_rsi = if !klines_15m.is_empty() {
        rsi_at(klines_15m, m15_last, 14)
    } else {
        0.0
    };
    let m15_macd = if !klines_15m.is_empty() {
        macd_at(klines_15m, m15_last, 12, 26)
    } else {
        0.0
    };
    let m15_macd_signal = if !klines_15m.is_empty() {
        macd_signal_at(klines_15m, m15_last, 12, 26, 9)
    } else {
        0.0
    };
    let m15_macd_histogram = if !klines_15m.is_empty() {
        macd_histogram_at(klines_15m, m15_last, 12, 26, 9)
    } else {
        0.0
    };
    let m15_bb_width_pct = if !klines_15m.is_empty() {
        bbands_width_at(klines_15m, m15_last, 20, 2.0)
    } else {
        0.0
    };
    let m15_atr = if !klines_15m.is_empty() {
        atr_at(klines_15m, m15_last, 14)
    } else {
        0.0
    };
    let m15_atr_sma20 = if klines_15m.len() >= 20 {
        let atr_series = atr(klines_15m, 14);
        sma_at_from(&atr_series, m15_last, 20)
    } else {
        0.0
    };
    let m15_adx = if !klines_15m.is_empty() {
        adx_at(klines_15m, m15_last, 14)
    } else {
        0.0
    };
    let (m15_bb_upper, _, m15_bb_lower) = if !klines_15m.is_empty() {
        bbands_at(klines_15m, m15_last, 20, 2.0)
    } else {
        (0.0, 0.0, 0.0)
    };
    let m15_bars_outside_band = compute_bars_outside_band(klines_15m, m15_bb_upper, m15_bb_lower);
    let m15_ema20 = if !klines_15m.is_empty() {
        ema_at(klines_15m, m15_last, 20)
    } else {
        0.0
    };
    let m15_ema50 = if klines_15m.len() >= 50 {
        ema_at(klines_15m, m15_last, 50)
    } else {
        0.0
    };
    let m15_last_completed = klines_15m.len().saturating_sub(2);
    let m15_volume = klines_15m
        .get(m15_last_completed)
        .map(|k| k.volume)
        .unwrap_or_else(|| {
            warn!(
                indicator = "m15_volume",
                idx = m15_last_completed,
                "Insufficient data for indicator calculation — defaulting to 0.0"
            );
            0.0
        });
    let m15_volume_sma20 = if m15_last_completed >= 19 {
        volume_sma_at(klines_15m, m15_last_completed, 20)
    } else {
        0.0
    };
    let m15_high_50 = if !klines_15m.is_empty() {
        highest_at(klines_15m, m15_last, 50)
    } else {
        0.0
    };
    let m15_low_50 = if !klines_15m.is_empty() {
        lowest_at(klines_15m, m15_last, 50)
    } else {
        0.0
    };
    let m15_ema_cross_bars_ago = compute_ema_cross_bars_ago(klines_15m, 20, 50, m15_last);

    MarketIndicators {
        current_price,
        rsi,
        atr: atr_val,
        atr_pct,
        bb_width,
        bb_upper,
        bb_middle,
        bb_lower,
        ema12,
        ema20,
        ema26,
        ema50,
        macd,
        macd_signal,
        macd_histogram,
        adx,
        change_1h,
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
        h1_ema_gap_trend,
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
        funding_rate,
        funding_next_time,
    }
}

fn compute_ema_cross_bars_ago(
    klines: &[Kline],
    fast_period: usize,
    slow_period: usize,
    last_idx: usize,
) -> i32 {
    if klines.len() < slow_period + 5 {
        return -1;
    }
    let lookback = 20.min(last_idx);
    for i in 0..lookback {
        let idx = last_idx - i;
        if idx < 1 {
            break;
        }
        let fast_curr = ema_at(klines, idx, fast_period);
        let slow_curr = ema_at(klines, idx, slow_period);
        let fast_prev = ema_at(klines, idx - 1, fast_period);
        let slow_prev = ema_at(klines, idx - 1, slow_period);
        if (fast_prev <= slow_prev && fast_curr > slow_curr)
            || (fast_prev >= slow_prev && fast_curr < slow_curr)
        {
            return i as i32;
        }
    }
    -1
}
