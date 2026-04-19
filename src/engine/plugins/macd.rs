use crate::engine::backtest::{compute_macd, compute_macd_signal_line};
use crate::engine::plugin::*;
use crate::models::Kline;
use std::collections::HashMap;

pub struct MacdPlugin;

impl IndicatorPlugin for MacdPlugin {
    fn name(&self) -> &str {
        "macd"
    }
    fn description(&self) -> &str {
        "MACD: Buy when MACD line crosses above signal line, sell when it crosses below."
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
                max: Some(100.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "slow_period".into(),
                label: "Slow Period".into(),
                param_type: ParamType::Int,
                default: 26.0,
                min: Some(5.0),
                max: Some(200.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "signal_period".into(),
                label: "Signal Period".into(),
                param_type: ParamType::Int,
                default: 9.0,
                min: Some(2.0),
                max: Some(100.0),
                step: Some(1.0),
            },
        ]
    }

    fn signal(&self, klines: &[Kline], idx: usize, params: &HashMap<String, f64>) -> i8 {
        let fast_period = params
            .get("fast_period")
            .map(|v| *v as usize)
            .unwrap_or(12);
        let slow_period = params
            .get("slow_period")
            .map(|v| *v as usize)
            .unwrap_or(26);
        let signal_period = params
            .get("signal_period")
            .map(|v| *v as usize)
            .unwrap_or(9);

        if idx < slow_period + signal_period {
            return 0;
        }

        let macd = compute_macd(klines, idx, fast_period, slow_period);
        let signal = compute_macd_signal_line(
            klines,
            idx,
            fast_period,
            slow_period,
            signal_period,
        );
        let prev_macd = compute_macd(klines, idx - 1, fast_period, slow_period);
        let prev_signal = compute_macd_signal_line(
            klines,
            idx - 1,
            fast_period,
            slow_period,
            signal_period,
        );

        if prev_macd <= prev_signal && macd > signal {
            1
        } else if prev_macd >= prev_signal && macd < signal {
            -1
        } else {
            0
        }
    }
}
