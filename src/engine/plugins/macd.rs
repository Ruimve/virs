use crate::engine::indicators;
use crate::engine::plugin::*;
use crate::models::Kline;
use std::collections::HashMap;

pub struct MacdPlugin;

impl IndicatorPlugin for MacdPlugin {
    fn name(&self) -> &str {
        "macd"
    }
    fn description(&self) -> &str {
        "MACD with zero-line and histogram confirmation: Buy on bullish crossover with momentum confirmation."
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
            ParamDef {
                name: "use_zero_line".into(),
                label: "Zero Line Filter".into(),
                param_type: ParamType::Int,
                default: 1.0,
                min: Some(0.0),
                max: Some(1.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "use_histogram".into(),
                label: "Histogram Confirmation".into(),
                param_type: ParamType::Int,
                default: 1.0,
                min: Some(0.0),
                max: Some(1.0),
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
        let use_zero_line = params
            .get("use_zero_line")
            .map(|v| *v as usize)
            .unwrap_or(1);
        let use_histogram = params
            .get("use_histogram")
            .map(|v| *v as usize)
            .unwrap_or(1);

        if idx < 1 || idx < slow_period + signal_period - 2 {
            return 0;
        }

        let macd = indicators::macd_at(klines, idx, fast_period, slow_period);
        let signal = indicators::macd_signal_at(
            klines,
            idx,
            fast_period,
            slow_period,
            signal_period,
        );
        let prev_macd = indicators::macd_at(klines, idx - 1, fast_period, slow_period);
        let prev_signal = indicators::macd_signal_at(
            klines,
            idx - 1,
            fast_period,
            slow_period,
            signal_period,
        );

        let bullish_cross = prev_macd <= prev_signal && macd > signal;
        let bearish_cross = prev_macd >= prev_signal && macd < signal;

        if bullish_cross {
            // Confirm with zero-line and/or histogram
            let zero_ok = use_zero_line == 0 || macd > 0.0;
            let hist_ok = use_histogram == 0
                || indicators::macd_histogram_at(
                    klines, idx, fast_period, slow_period, signal_period,
                ) > 0.0;
            if zero_ok || hist_ok {
                return 1;
            }
        } else if bearish_cross {
            // Confirm with zero-line and/or histogram
            let zero_ok = use_zero_line == 0 || macd < 0.0;
            let hist_ok = use_histogram == 0
                || indicators::macd_histogram_at(
                    klines, idx, fast_period, slow_period, signal_period,
                ) < 0.0;
            if zero_ok || hist_ok {
                return -1;
            }
        }

        0
    }
}
