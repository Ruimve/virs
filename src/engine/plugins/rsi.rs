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
        "RSI: Buy when RSI crosses below oversold level, sell when RSI crosses above overbought level."
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
        ]
    }

    fn signal(&self, klines: &[Kline], idx: usize, params: &HashMap<String, f64>) -> i8 {
        let period = params.get("period").map(|v| *v as usize).unwrap_or(14);
        let oversold = params.get("oversold").copied().unwrap_or(30.0);
        let overbought = params.get("overbought").copied().unwrap_or(70.0);

        if idx < 1 || idx < period { return 0; }

        let rsi = indicators::rsi_at(klines, idx, period);
        let prev_rsi = indicators::rsi_at(klines, idx - 1, period);

        if prev_rsi >= oversold && rsi < oversold {
            1 // Buy signal (RSI crossed below oversold)
        } else if prev_rsi <= overbought && rsi > overbought {
            -1 // Sell signal (RSI crossed above overbought)
        } else {
            0
        }
    }
}
