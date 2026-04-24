use crate::engine::indicators;
use crate::engine::plugin::*;
use crate::models::Kline;
use std::collections::HashMap;

pub struct SmaCrossoverPlugin;

impl IndicatorPlugin for SmaCrossoverPlugin {
    fn name(&self) -> &str {
        "ema_crossover"
    }
    fn description(&self) -> &str {
        "EMA Crossover with ADX trend filter: Buy when fast EMA crosses above slow EMA (only in strong trends if filter enabled), sell on opposite crossover."
    }
    fn category(&self) -> &str {
        "trend"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef {
                name: "fast_period".into(),
                label: "Fast Period".into(),
                param_type: ParamType::Int,
                default: 12.0,
                min: Some(2.0),
                max: Some(200.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "slow_period".into(),
                label: "Slow Period".into(),
                param_type: ParamType::Int,
                default: 26.0,
                min: Some(5.0),
                max: Some(500.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "trend_filter".into(),
                label: "ADX Trend Filter".into(),
                param_type: ParamType::Int,
                default: 1.0,
                min: Some(0.0),
                max: Some(1.0),
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
                name: "adx_threshold".into(),
                label: "ADX Threshold".into(),
                param_type: ParamType::Float,
                default: 20.0,
                min: Some(10.0),
                max: Some(50.0),
                step: Some(1.0),
            },
        ]
    }

    fn signal(&self, klines: &[Kline], idx: usize, params: &HashMap<String, f64>) -> i8 {
        let fast = params
            .get("fast_period")
            .map(|v| *v as usize)
            .unwrap_or(12);
        let slow = params
            .get("slow_period")
            .map(|v| *v as usize)
            .unwrap_or(26);
        let trend_filter = params
            .get("trend_filter")
            .map(|v| *v as usize)
            .unwrap_or(1);
        let adx_period = params
            .get("adx_period")
            .map(|v| *v as usize)
            .unwrap_or(14);
        let adx_threshold = params
            .get("adx_threshold")
            .copied()
            .unwrap_or(20.0);

        if idx < 1 || klines.len() < 2 || idx < slow - 1 {
            return 0;
        }

        // ADX trend strength filter
        if trend_filter == 1 {
            let adx_val = indicators::adx_at(klines, idx, adx_period);
            if adx_val < adx_threshold {
                return 0;
            }
        }

        let fast_ema = indicators::ema_at(klines, idx, fast);
        let prev_fast_ema = indicators::ema_at(klines, idx - 1, fast);
        let slow_ema = indicators::ema_at(klines, idx, slow);
        let prev_slow_ema = indicators::ema_at(klines, idx - 1, slow);

        if prev_fast_ema <= prev_slow_ema && fast_ema > slow_ema {
            1
        } else if prev_fast_ema >= prev_slow_ema && fast_ema < slow_ema {
            -1
        } else {
            0
        }
    }
}
