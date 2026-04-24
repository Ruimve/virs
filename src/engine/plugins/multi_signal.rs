use crate::engine::indicators;
use crate::engine::plugin::*;
use crate::models::Kline;
use std::collections::HashMap;

pub struct MultiSignalPlugin;

impl IndicatorPlugin for MultiSignalPlugin {
    fn name(&self) -> &str {
        "multi_signal"
    }
    fn description(&self) -> &str {
        "Multi-Signal Confluence: Combines EMA trend, RSI momentum, and ATR volatility for signal confirmation. Only generates signals when multiple indicators agree, reducing false signals."
    }
    fn category(&self) -> &str {
        "confluence"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef {
                name: "ema_fast".into(),
                label: "Fast EMA".into(),
                param_type: ParamType::Int,
                default: 12.0,
                min: Some(2.0),
                max: Some(50.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "ema_slow".into(),
                label: "Slow EMA".into(),
                param_type: ParamType::Int,
                default: 26.0,
                min: Some(5.0),
                max: Some(200.0),
                step: Some(1.0),
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
                label: "RSI Overbought".into(),
                param_type: ParamType::Float,
                default: 70.0,
                min: Some(50.0),
                max: Some(95.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "rsi_oversold".into(),
                label: "RSI Oversold".into(),
                param_type: ParamType::Float,
                default: 30.0,
                min: Some(5.0),
                max: Some(50.0),
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
                name: "atr_filter".into(),
                label: "ATR Filter".into(),
                param_type: ParamType::Int,
                default: 1.0,
                min: Some(0.0),
                max: Some(1.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "min_signals".into(),
                label: "Min Signals".into(),
                param_type: ParamType::Int,
                default: 2.0,
                min: Some(1.0),
                max: Some(3.0),
                step: Some(1.0),
            },
        ]
    }

    fn signal(&self, klines: &[Kline], idx: usize, params: &HashMap<String, f64>) -> i8 {
        let ema_fast = params
            .get("ema_fast")
            .map(|v| *v as usize)
            .unwrap_or(12);
        let ema_slow = params
            .get("ema_slow")
            .map(|v| *v as usize)
            .unwrap_or(26);
        let rsi_period = params
            .get("rsi_period")
            .map(|v| *v as usize)
            .unwrap_or(14);
        let rsi_overbought = params.get("rsi_overbought").copied().unwrap_or(70.0);
        let rsi_oversold = params.get("rsi_oversold").copied().unwrap_or(30.0);
        let atr_period = params
            .get("atr_period")
            .map(|v| *v as usize)
            .unwrap_or(14);
        let atr_filter = params
            .get("atr_filter")
            .map(|v| *v as usize)
            .unwrap_or(1);
        let min_signals = params
            .get("min_signals")
            .map(|v| *v as usize)
            .unwrap_or(2);

        let required = ema_slow.max(rsi_period).max(atr_period);
        if idx < 1 || klines.len() < 2 || idx < required {
            return 0;
        }

        // EMA values
        let fast_ema = indicators::ema_at(klines, idx, ema_fast);
        let prev_fast_ema = indicators::ema_at(klines, idx - 1, ema_fast);
        let slow_ema = indicators::ema_at(klines, idx, ema_slow);
        let prev_slow_ema = indicators::ema_at(klines, idx - 1, ema_slow);

        // RSI values
        let rsi = indicators::rsi_at(klines, idx, rsi_period);
        let prev_rsi = indicators::rsi_at(klines, idx - 1, rsi_period);

        // ATR values
        let atr_val = indicators::atr_at(klines, idx, atr_period);
        let prev_atr_val = indicators::atr_at(klines, idx - 1, atr_period);

        // Buy signal counting
        let mut buy_count: usize = 0;

        // 1. EMA golden cross (fast crosses above slow)
        if prev_fast_ema <= prev_slow_ema && fast_ema > slow_ema {
            buy_count += 1;
        }

        // 2. RSI recovery from oversold
        if prev_rsi < rsi_oversold && rsi >= rsi_oversold {
            buy_count += 1;
        }

        // 3. ATR rising (volatility expanding, trend starting)
        if atr_filter == 1 && atr_val > prev_atr_val {
            buy_count += 1;
        }

        // Sell signal counting
        let mut sell_count: usize = 0;

        // 1. EMA death cross (fast crosses below slow)
        if prev_fast_ema >= prev_slow_ema && fast_ema < slow_ema {
            sell_count += 1;
        }

        // 2. RSI retreat from overbought
        if prev_rsi > rsi_overbought && rsi <= rsi_overbought {
            sell_count += 1;
        }

        // 3. ATR rising (volatility expanding, downtrend starting)
        if atr_filter == 1 && atr_val > prev_atr_val {
            sell_count += 1;
        }

        if buy_count >= min_signals {
            1
        } else if sell_count >= min_signals {
            -1
        } else {
            0
        }
    }
}
