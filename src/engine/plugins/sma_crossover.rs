use crate::engine::indicators;
use crate::engine::plugin::*;
use crate::models::Kline;
use std::collections::HashMap;

pub struct SmaCrossoverPlugin;

impl IndicatorPlugin for SmaCrossoverPlugin {
    fn name(&self) -> &str {
        "sma_crossover"
    }
    fn description(&self) -> &str {
        "SMA Crossover: Buy when fast SMA crosses above slow SMA, sell when it crosses below."
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
                default: 5.0,
                min: Some(2.0),
                max: Some(200.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "slow_period".into(),
                label: "Slow Period".into(),
                param_type: ParamType::Int,
                default: 20.0,
                min: Some(5.0),
                max: Some(500.0),
                step: Some(1.0),
            },
        ]
    }

    fn signal(&self, klines: &[Kline], idx: usize, params: &HashMap<String, f64>) -> i8 {
        let fast = params
            .get("fast_period")
            .map(|v| *v as usize)
            .unwrap_or(5);
        let slow = params
            .get("slow_period")
            .map(|v| *v as usize)
            .unwrap_or(20);
        if idx < 1 || klines.len() < 2 || idx < slow - 1 {
            return 0;
        }
        let fast_sma = indicators::sma_at(klines, idx, fast);
        let prev_fast_sma = indicators::sma_at(klines, idx - 1, fast);
        let slow_sma = indicators::sma_at(klines, idx, slow);
        let prev_slow_sma = indicators::sma_at(klines, idx - 1, slow);
        if prev_fast_sma <= prev_slow_sma && fast_sma > slow_sma {
            1
        } else if prev_fast_sma >= prev_slow_sma && fast_sma < slow_sma {
            -1
        } else {
            0
        }
    }
}
