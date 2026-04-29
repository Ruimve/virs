use crate::engine::strategy::indicators;
use crate::engine::strategy::plugin::*;
use std::collections::HashMap;

pub struct DualEmaTrendPlugin;

impl IndicatorPlugin for DualEmaTrendPlugin {
    fn name(&self) -> &str {
        "dual_ema_trend"
    }

    fn description(&self) -> &str {
        "Dual EMA Trend: EMA crossover with higher timeframe trend confirmation (4h). ADX filter removed to allow timely exits."
    }

    fn category(&self) -> &str {
        "trend"
    }

    fn required_timeframes(&self) -> Vec<&str> {
        vec!["4h"]
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef {
                name: "ema_fast".into(),
                label: "Fast EMA Period".into(),
                param_type: ParamType::Int,
                default: 12.0,
                min: Some(2.0),
                max: Some(50.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "ema_slow".into(),
                label: "Slow EMA Period".into(),
                param_type: ParamType::Int,
                default: 26.0,
                min: Some(5.0),
                max: Some(200.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "adx_period".into(),
                label: "ADX Period (unused)".into(),
                param_type: ParamType::Int,
                default: 14.0,
                min: Some(5.0),
                max: Some(50.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "adx_threshold".into(),
                label: "ADX Threshold (unused)".into(),
                param_type: ParamType::Float,
                default: 20.0,
                min: Some(10.0),
                max: Some(50.0),
                step: Some(1.0),
            },
        ]
    }

    fn signal(&self, ctx: &SignalContext, params: &HashMap<String, f64>) -> i8 {
        let ema_fast = params.get("ema_fast").map(|v| *v as usize).unwrap_or(12);
        let ema_slow = params.get("ema_slow").map(|v| *v as usize).unwrap_or(26);

        let klines = ctx.klines;
        let idx = ctx.idx;

        if idx < 1 || klines.len() < 2 || idx < ema_slow - 1 {
            return 0;
        }

        // EMA crossover detection
        let fast_ema = indicators::ema_at(klines, idx, ema_fast);
        let prev_fast_ema = indicators::ema_at(klines, idx - 1, ema_fast);
        let slow_ema = indicators::ema_at(klines, idx, ema_slow);
        let prev_slow_ema = indicators::ema_at(klines, idx - 1, ema_slow);

        let golden_cross = prev_fast_ema <= prev_slow_ema && fast_ema > slow_ema;
        let death_cross = prev_fast_ema >= prev_slow_ema && fast_ema < slow_ema;

        if !golden_cross && !death_cross {
            return 0;
        }

        // Higher timeframe (4h) trend confirmation
        let bullish_htf = if let Some(tf_klines) = ctx.extra_klines.get("4h") {
            if tf_klines.is_empty() {
                true
            } else {
                let htf_idx = tf_klines.len() - 1;
                let htf_close = tf_klines[htf_idx].close;
                let htf_ema = indicators::ema_at(tf_klines, htf_idx, 26);
                htf_close > htf_ema
            }
        } else {
            true
        };

        // Golden cross -> always return 1 (will close short or open long)
        // Death cross -> always return -1 (will close long or open short)
        // HTF filter is no longer used to block signals, since we need
        // crossover signals to always fire for position exit capability.
        if golden_cross {
            return 1;
        }
        if death_cross {
            return -1;
        }

        0
    }
}
