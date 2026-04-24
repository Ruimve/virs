use crate::engine::indicators;
use crate::engine::plugin::*;
use crate::models::Kline;
use std::collections::HashMap;

pub struct RsiPlugin;

impl IndicatorPlugin for RsiPlugin {
    fn name(&self) -> &str {
        "rsi"
    }
    fn description(&self) -> &str {
        "RSI with trend confirmation: Buy on oversold bounce in uptrend, sell on overbought pullback in downtrend."
    }
    fn category(&self) -> &str {
        "oscillator"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef {
                name: "period".into(),
                label: "Period".into(),
                param_type: ParamType::Int,
                default: 14.0,
                min: Some(2.0),
                max: Some(100.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "oversold".into(),
                label: "Oversold".into(),
                param_type: ParamType::Float,
                default: 30.0,
                min: Some(0.0),
                max: Some(50.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "overbought".into(),
                label: "Overbought".into(),
                param_type: ParamType::Float,
                default: 70.0,
                min: Some(50.0),
                max: Some(100.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "trend_filter".into(),
                label: "EMA Trend Filter".into(),
                param_type: ParamType::Int,
                default: 1.0,
                min: Some(0.0),
                max: Some(1.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "ema_period".into(),
                label: "EMA Period".into(),
                param_type: ParamType::Int,
                default: 50.0,
                min: Some(10.0),
                max: Some(200.0),
                step: Some(1.0),
            },
        ]
    }

    fn signal(&self, klines: &[Kline], idx: usize, params: &HashMap<String, f64>) -> i8 {
        let period = params.get("period").map(|v| *v as usize).unwrap_or(14);
        let oversold = params.get("oversold").copied().unwrap_or(30.0);
        let overbought = params.get("overbought").copied().unwrap_or(70.0);
        let trend_filter = params
            .get("trend_filter")
            .map(|v| *v as usize)
            .unwrap_or(1);
        let ema_period = params
            .get("ema_period")
            .map(|v| *v as usize)
            .unwrap_or(50);

        if idx < 1 || idx < period {
            return 0;
        }

        let rsi = indicators::rsi_at(klines, idx, period);
        let prev_rsi = indicators::rsi_at(klines, idx - 1, period);

        if trend_filter == 0 {
            // Original logic: pure overbought/oversold reversal
            if prev_rsi >= oversold && rsi < oversold {
                1
            } else if prev_rsi <= overbought && rsi > overbought {
                -1
            } else {
                0
            }
        } else {
            // Trend-confirmed signals
            let close = klines[idx].close;
            let ema_val = indicators::ema_at(klines, idx, ema_period);

            // RSI crosses above oversold AND price above EMA -> buy (oversold bounce in uptrend)
            if prev_rsi <= oversold && rsi > oversold && close > ema_val {
                1
            // RSI crosses below overbought AND price below EMA -> sell (overbought pullback in downtrend)
            } else if prev_rsi >= overbought && rsi < overbought && close < ema_val {
                -1
            } else {
                0
            }
        }
    }
}
