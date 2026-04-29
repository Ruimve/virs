use crate::engine::strategy::indicators;
use crate::engine::strategy::plugin::*;
use std::collections::HashMap;

pub struct BbSqueezePlugin;

impl IndicatorPlugin for BbSqueezePlugin {
    fn name(&self) -> &str {
        "bb_squeeze"
    }

    fn description(&self) -> &str {
        "Bollinger Squeeze Breakout: BBands contraction (vs 5-bar average width) followed by breakout with daily SMA(20) trend direction."
    }

    fn category(&self) -> &str {
        "breakout"
    }

    fn required_timeframes(&self) -> Vec<&str> {
        vec!["1d"]
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef {
                name: "bb_period".into(),
                label: "BBands Period".into(),
                param_type: ParamType::Int,
                default: 20.0,
                min: Some(5.0),
                max: Some(50.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "bb_std".into(),
                label: "BBands Std Dev".into(),
                param_type: ParamType::Float,
                default: 2.0,
                min: Some(0.5),
                max: Some(4.0),
                step: Some(0.1),
            },
            ParamDef {
                name: "rsi_period".into(),
                label: "RSI Period".into(),
                param_type: ParamType::Int,
                default: 14.0,
                min: Some(5.0),
                max: Some(50.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "rsi_overbought".into(),
                label: "RSI Overbought (unused)".into(),
                param_type: ParamType::Float,
                default: 70.0,
                min: Some(50.0),
                max: Some(95.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "rsi_oversold".into(),
                label: "RSI Oversold (unused)".into(),
                param_type: ParamType::Float,
                default: 30.0,
                min: Some(5.0),
                max: Some(50.0),
                step: Some(1.0),
            },
        ]
    }

    fn signal(&self, ctx: &SignalContext, params: &HashMap<String, f64>) -> i8 {
        let bb_period = params.get("bb_period").map(|v| *v as usize).unwrap_or(20);
        let bb_std = params.get("bb_std").copied().unwrap_or(2.0);

        let klines = ctx.klines;
        let idx = ctx.idx;

        if idx < 5 || klines.len() < 6 || idx < bb_period {
            return 0;
        }

        // Squeeze detection: current width < average of last 5 bars (including current)
        let mut width_sum = 0.0;
        let width_count = 5usize;
        for j in 0..=width_count {
            if idx >= j {
                width_sum += indicators::bbands_width_at(klines, idx - j, bb_period, bb_std);
            }
        }
        let avg_width = width_sum / (width_count as f64 + 1.0);
        let current_width = indicators::bbands_width_at(klines, idx, bb_period, bb_std);
        if current_width >= avg_width || current_width == 0.0 {
            return 0;
        }

        let (upper, _middle, lower) = indicators::bbands_at(klines, idx, bb_period, bb_std);
        let price = klines[idx].close;

        // Daily trend direction (SMA(20) instead of SMA(50) for faster response)
        let daily_bullish = if let Some(tf_klines) = ctx.extra_klines.get("1d") {
            if tf_klines.is_empty() {
                true
            } else {
                let d_idx = tf_klines.len() - 1;
                let d_close = tf_klines[d_idx].close;
                let d_sma20 = indicators::sma_at(tf_klines, d_idx, 20);
                d_close > d_sma20
            }
        } else {
            true
        };

        // Breakout without RSI filter (RSI parameters kept for API compatibility)
        if price > upper && daily_bullish {
            return 1;
        }

        if price < lower && !daily_bullish {
            return -1;
        }

        0
    }
}
