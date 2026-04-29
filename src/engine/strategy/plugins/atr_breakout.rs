use crate::engine::strategy::indicators;
use crate::engine::strategy::plugin::*;
use std::collections::HashMap;

pub struct AtrBreakoutPlugin;

impl IndicatorPlugin for AtrBreakoutPlugin {
    fn name(&self) -> &str {
        "atr_breakout"
    }

    fn description(&self) -> &str {
        "ATR Channel Breakout: Donchian Channel + ATR breakout system (Turtle Trading style). Simplified with lower ATR multiplier."
    }

    fn category(&self) -> &str {
        "breakout"
    }

    fn required_timeframes(&self) -> Vec<&str> {
        vec![]
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![
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
                name: "atr_period".into(),
                label: "ATR Period".into(),
                param_type: ParamType::Int,
                default: 14.0,
                min: Some(5.0),
                max: Some(50.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "atr_multiplier".into(),
                label: "ATR Multiplier".into(),
                param_type: ParamType::Float,
                default: 1.0,
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

    fn signal(&self, ctx: &SignalContext, params: &HashMap<String, f64>) -> i8 {
        let channel_period = params
            .get("channel_period")
            .map(|v| *v as usize)
            .unwrap_or(20);
        let atr_period = params.get("atr_period").map(|v| *v as usize).unwrap_or(14);
        let atr_multiplier = params.get("atr_multiplier").copied().unwrap_or(1.0);

        let klines = ctx.klines;
        let idx = ctx.idx;

        if idx < 1 || klines.len() < 2 || idx < channel_period - 1 {
            return 0;
        }

        let atr = indicators::atr_at(klines, idx, atr_period);
        if atr == 0.0 {
            return 0;
        }

        // Entry channels
        let entry_upper =
            indicators::highest_at(klines, idx, channel_period) + atr * atr_multiplier;
        let entry_lower =
            indicators::lowest_at(klines, idx, channel_period) - atr * atr_multiplier;

        let close = klines[idx].close;

        // Simplified breakout: close breaks above/below entry channel
        if close > entry_upper {
            return 1;
        }

        if close < entry_lower {
            return -1;
        }

        0
    }
}
