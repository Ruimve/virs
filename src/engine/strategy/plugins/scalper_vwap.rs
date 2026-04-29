use crate::engine::strategy::indicators;
use crate::engine::strategy::plugin::*;
use std::collections::HashMap;

pub struct ScalperVwapPlugin;

impl IndicatorPlugin for ScalperVwapPlugin {
    fn name(&self) -> &str {
        "scalper_vwap"
    }

    fn description(&self) -> &str {
        "Scalper VWAP: Short-term scalping based on VWAP + RSI with volume confirmation. Best for 1m/5m timeframes."
    }

    fn category(&self) -> &str {
        "scalping"
    }

    fn required_timeframes(&self) -> Vec<&str> {
        vec![]
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef {
                name: "vwap_period".into(),
                label: "VWAP Period".into(),
                param_type: ParamType::Int,
                default: 20.0,
                min: Some(5.0),
                max: Some(100.0),
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
                name: "atr_sl_multiplier".into(),
                label: "ATR Stop-Loss Multiplier".into(),
                param_type: ParamType::Float,
                default: 1.5,
                min: Some(0.5),
                max: Some(3.0),
                step: Some(0.1),
            },
            ParamDef {
                name: "volume_filter".into(),
                label: "Volume Filter (0=off, 1=on)".into(),
                param_type: ParamType::Int,
                default: 1.0,
                min: Some(0.0),
                max: Some(1.0),
                step: Some(1.0),
            },
        ]
    }

    fn signal(&self, ctx: &SignalContext, params: &HashMap<String, f64>) -> i8 {
        let vwap_period = params
            .get("vwap_period")
            .map(|v| *v as usize)
            .unwrap_or(20);
        let rsi_period = params.get("rsi_period").map(|v| *v as usize).unwrap_or(14);
        let rsi_overbought = params.get("rsi_overbought").copied().unwrap_or(70.0);
        let rsi_oversold = params.get("rsi_oversold").copied().unwrap_or(30.0);
        let volume_filter = params.get("volume_filter").map(|v| *v as usize).unwrap_or(1);

        let klines = ctx.klines;
        let idx = ctx.idx;

        if idx < 1 || klines.len() < 2 || idx < vwap_period - 1 || idx < rsi_period {
            return 0;
        }

        // Volume filter - current volume > SMA(volume, 20) * 0.8 (relaxed)
        if volume_filter == 1 {
            let vol_sma = indicators::volume_sma_at(klines, idx, 20);
            if vol_sma > 0.0 && klines[idx].volume < vol_sma * 0.8 {
                return 0;
            }
        }

        let vwap = indicators::vwap_at(klines, idx, vwap_period);
        if vwap == 0.0 {
            return 0;
        }

        let close = klines[idx].close;
        let rsi = indicators::rsi_at(klines, idx, rsi_period);
        let prev_rsi = indicators::rsi_at(klines, idx - 1, rsi_period);

        // close > VWAP and RSI recovering from oversold -> buy
        if close > vwap && prev_rsi < rsi_oversold && rsi >= rsi_oversold {
            return 1;
        }

        // close < VWAP and RSI falling from overbought -> sell
        if close < vwap && prev_rsi > rsi_overbought && rsi <= rsi_overbought {
            return -1;
        }

        0
    }
}
