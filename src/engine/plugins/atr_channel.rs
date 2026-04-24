use crate::engine::indicators;
use crate::engine::plugin::*;
use crate::models::Kline;
use std::collections::HashMap;

pub struct AtrChannelPlugin;

impl IndicatorPlugin for AtrChannelPlugin {
    fn name(&self) -> &str {
        "atr_channel"
    }
    fn description(&self) -> &str {
        "ATR Channel Breakout: Buy when price breaks above the upper channel (highest + ATR * multiplier), sell when price breaks below the lower channel (lowest - ATR * multiplier). Based on volatility breakout similar to Turtle Trading."
    }
    fn category(&self) -> &str {
        "volatility"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![
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
                name: "channel_period".into(),
                label: "Channel Period".into(),
                param_type: ParamType::Int,
                default: 20.0,
                min: Some(5.0),
                max: Some(100.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "multiplier".into(),
                label: "ATR Multiplier".into(),
                param_type: ParamType::Float,
                default: 2.0,
                min: Some(0.5),
                max: Some(5.0),
                step: Some(0.1),
            },
            ParamDef {
                name: "exit_period".into(),
                label: "Exit Period".into(),
                param_type: ParamType::Int,
                default: 10.0,
                min: Some(3.0),
                max: Some(50.0),
                step: Some(1.0),
            },
        ]
    }

    fn signal(&self, klines: &[Kline], idx: usize, params: &HashMap<String, f64>) -> i8 {
        let atr_period = params
            .get("atr_period")
            .map(|v| *v as usize)
            .unwrap_or(14);
        let channel_period = params
            .get("channel_period")
            .map(|v| *v as usize)
            .unwrap_or(20);
        let multiplier = params.get("multiplier").copied().unwrap_or(2.0);
        let _exit_period = params
            .get("exit_period")
            .map(|v| *v as usize)
            .unwrap_or(10);

        let required = channel_period.max(atr_period);
        if idx < 1 || klines.len() < 2 || idx < required {
            return 0;
        }

        let atr_val = indicators::atr_at(klines, idx, atr_period);
        let highest_val = indicators::highest_at(klines, idx, channel_period);
        let lowest_val = indicators::lowest_at(klines, idx, channel_period);

        let upper_band = highest_val + atr_val * multiplier;
        let lower_band = lowest_val - atr_val * multiplier;

        let prev_highest_val = indicators::highest_at(klines, idx - 1, channel_period);
        let prev_lowest_val = indicators::lowest_at(klines, idx - 1, channel_period);
        let prev_atr_val = indicators::atr_at(klines, idx - 1, atr_period);

        let prev_upper_band = prev_highest_val + prev_atr_val * multiplier;
        let prev_lower_band = prev_lowest_val - prev_atr_val * multiplier;

        let close = klines[idx].close;
        let prev_close = klines[idx - 1].close;

        // Buy: current close breaks above upper band (previous close did not)
        if prev_close <= prev_upper_band && close > upper_band {
            1
        // Sell: current close breaks below lower band (previous close did not)
        } else if prev_close >= prev_lower_band && close < lower_band {
            -1
        } else {
            0
        }
    }
}
