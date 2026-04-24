use crate::engine::indicators;
use crate::engine::plugin::*;
use std::collections::HashMap;

pub struct MomentumBreakoutPlugin;

impl IndicatorPlugin for MomentumBreakoutPlugin {
    fn name(&self) -> &str {
        "momentum_breakout"
    }

    fn description(&self) -> &str {
        "Momentum Breakout: ATR above average + RSI direction with BBands or 4h EMA confirmation. Reduced conditions for more signals."
    }

    fn category(&self) -> &str {
        "breakout"
    }

    fn required_timeframes(&self) -> Vec<&str> {
        vec!["4h"]
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![
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
                name: "rsi_breakout".into(),
                label: "RSI Breakout Level".into(),
                param_type: ParamType::Float,
                default: 60.0,
                min: Some(50.0),
                max: Some(80.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "atr_period".into(),
                label: "ATR Period".into(),
                param_type: ParamType::Int,
                default: 14.0,
                min: Some(5.0),
                max: Some(50.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "atr_accel".into(),
                label: "ATR Acceleration Factor".into(),
                param_type: ParamType::Float,
                default: 1.1,
                min: Some(0.8),
                max: Some(2.0),
                step: Some(0.1),
            },
            ParamDef {
                name: "bb_period".into(),
                label: "BBands Period".into(),
                param_type: ParamType::Int,
                default: 20.0,
                min: Some(5.0),
                max: Some(50.0),
                step: Some(1.0),
            },
        ]
    }

    fn signal(&self, ctx: &SignalContext, params: &HashMap<String, f64>) -> i8 {
        let rsi_period = params.get("rsi_period").map(|v| *v as usize).unwrap_or(14);
        let rsi_breakout = params.get("rsi_breakout").copied().unwrap_or(60.0);
        let atr_period = params.get("atr_period").map(|v| *v as usize).unwrap_or(14);
        let atr_accel = params.get("atr_accel").copied().unwrap_or(1.1);
        let bb_period = params.get("bb_period").map(|v| *v as usize).unwrap_or(20);

        let klines = ctx.klines;
        let idx = ctx.idx;

        if idx < 5 || klines.len() < 6 || idx < bb_period {
            return 0;
        }

        // Condition 1: ATR above 5-bar average (more stable than prev * factor)
        let mut atr_sum = 0.0;
        let atr_lookback = 5usize;
        for j in 0..=atr_lookback {
            if idx >= j {
                atr_sum += indicators::atr_at(klines, idx - j, atr_period);
            }
        }
        let avg_atr = atr_sum / (atr_lookback as f64 + 1.0);
        if avg_atr == 0.0 {
            return 0;
        }
        let atr_high = indicators::atr_at(klines, idx, atr_period) > avg_atr * atr_accel;

        if !atr_high {
            return 0;
        }

        // Condition 2: RSI direction
        let rsi = indicators::rsi_at(klines, idx, rsi_period);
        let rsi_bullish = rsi > rsi_breakout;
        let rsi_bearish = rsi < (100.0 - rsi_breakout);

        // Condition 3: Price breaks BBands
        let (upper, _middle, lower) = indicators::bbands_at(klines, idx, bb_period, 2.0);
        let price = klines[idx].close;

        // Condition 4: 4h EMA(12) > EMA(26)
        let htf4_bullish = if let Some(tf_klines) = ctx.extra_klines.get("4h") {
            if tf_klines.len() < 27 {
                true
            } else {
                let htf_idx = tf_klines.len() - 1;
                let ema12 = indicators::ema_at(tf_klines, htf_idx, 12);
                let ema26 = indicators::ema_at(tf_klines, htf_idx, 26);
                ema12 > ema26
            }
        } else {
            true
        };

        // Buy: ATR high + RSI bullish + (BB upper break OR 4h bullish)
        if rsi_bullish && (price > upper || htf4_bullish) {
            return 1;
        }

        // Sell: ATR high + RSI bearish + (BB lower break OR !4h bullish)
        if rsi_bearish && (price < lower || !htf4_bullish) {
            return -1;
        }

        0
    }
}
