use crate::engine::strategy::indicators;
use crate::engine::strategy::plugin::*;
use std::collections::HashMap;

pub struct RsiMeanReversionPlugin;

impl IndicatorPlugin for RsiMeanReversionPlugin {
    fn name(&self) -> &str {
        "rsi_mean_reversion"
    }

    fn description(&self) -> &str {
        "RSI Mean Reversion: RSI extreme reversal in ranging markets (ADX < threshold). Standard 30/70 thresholds."
    }

    fn category(&self) -> &str {
        "mean_reversion"
    }

    fn required_timeframes(&self) -> Vec<&str> {
        vec![]
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
                name: "rsi_overbought".into(),
                label: "RSI Overbought".into(),
                param_type: ParamType::Float,
                default: 70.0,
                min: Some(60.0),
                max: Some(95.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "rsi_oversold".into(),
                label: "RSI Oversold".into(),
                param_type: ParamType::Float,
                default: 30.0,
                min: Some(5.0),
                max: Some(40.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "adx_period".into(),
                label: "ADX Period".into(),
                param_type: ParamType::Int,
                default: 14.0,
                min: Some(5.0),
                max: Some(50.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "adx_max".into(),
                label: "ADX Max (Ranging)".into(),
                param_type: ParamType::Float,
                default: 30.0,
                min: Some(15.0),
                max: Some(40.0),
                step: Some(1.0),
            },
        ]
    }

    fn signal(&self, ctx: &SignalContext, params: &HashMap<String, f64>) -> i8 {
        let rsi_period = params.get("rsi_period").map(|v| *v as usize).unwrap_or(14);
        let rsi_overbought = params.get("rsi_overbought").copied().unwrap_or(70.0);
        let rsi_oversold = params.get("rsi_oversold").copied().unwrap_or(30.0);
        let adx_period = params.get("adx_period").map(|v| *v as usize).unwrap_or(14);
        let adx_max = params.get("adx_max").copied().unwrap_or(30.0);

        let klines = ctx.klines;
        let idx = ctx.idx;

        if idx < 1 || klines.len() < 2 || idx < rsi_period {
            return 0;
        }

        // ADX < adx_max (confirm ranging market, not trending)
        let adx_val = indicators::adx_at(klines, idx, adx_period);
        if adx_val >= adx_max {
            return 0;
        }

        let rsi = indicators::rsi_at(klines, idx, rsi_period);
        let prev_rsi = indicators::rsi_at(klines, idx - 1, rsi_period);

        // RSI falling back from overbought -> sell
        if prev_rsi > rsi_overbought && rsi <= rsi_overbought {
            return -1;
        }

        // RSI rising back from oversold -> buy
        if prev_rsi < rsi_oversold && rsi >= rsi_oversold {
            return 1;
        }

        0
    }
}
