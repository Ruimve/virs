use crate::engine::indicators;
use crate::engine::plugin::*;
use crate::models::Kline;
use std::collections::HashMap;

pub struct BollingerBandsPlugin;

impl IndicatorPlugin for BollingerBandsPlugin {
    fn name(&self) -> &str {
        "bollinger_bands"
    }
    fn description(&self) -> &str {
        "Bollinger Bands with squeeze detection and trend confirmation: Buy on lower band touch with squeeze or uptrend."
    }
    fn category(&self) -> &str {
        "volatility"
    }

    fn params(&self) -> Vec<ParamDef> {
        vec![
            ParamDef {
                name: "period".into(),
                label: "Period".into(),
                param_type: ParamType::Int,
                default: 20.0,
                min: Some(2.0),
                max: Some(200.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "std_dev".into(),
                label: "Std Dev Multiplier".into(),
                param_type: ParamType::Float,
                default: 2.0,
                min: Some(0.5),
                max: Some(4.0),
                step: Some(0.1),
            },
            ParamDef {
                name: "use_squeeze".into(),
                label: "Squeeze Detection".into(),
                param_type: ParamType::Int,
                default: 1.0,
                min: Some(0.0),
                max: Some(1.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "use_trend".into(),
                label: "EMA Trend Filter".into(),
                param_type: ParamType::Int,
                default: 1.0,
                min: Some(0.0),
                max: Some(1.0),
                step: Some(1.0),
            },
            ParamDef {
                name: "ema_period".into(),
                label: "EMA Period".into(),
                param_type: ParamType::Int,
                default: 50.0,
                min: Some(10.0),
                max: Some(200.0),
                step: Some(1.0),
            },
        ]
    }

    fn signal(&self, klines: &[Kline], idx: usize, params: &HashMap<String, f64>) -> i8 {
        let period = params.get("period").map(|v| *v as usize).unwrap_or(20);
        let std_dev_mult = params.get("std_dev").copied().unwrap_or(2.0);
        let use_squeeze = params
            .get("use_squeeze")
            .map(|v| *v as usize)
            .unwrap_or(1);
        let use_trend = params
            .get("use_trend")
            .map(|v| *v as usize)
            .unwrap_or(1);
        let ema_period = params
            .get("ema_period")
            .map(|v| *v as usize)
            .unwrap_or(50);

        if idx < period - 1 || idx < 1 {
            return 0;
        }

        let (upper, _middle, lower) = indicators::bbands_at(klines, idx, period, std_dev_mult);
        let price = klines[idx].close;

        // Squeeze detection: current bandwidth < previous bandwidth
        let is_squeeze = use_squeeze == 0 || {
            let cur_width = indicators::bbands_width_at(klines, idx, period, std_dev_mult);
            let prev_width = indicators::bbands_width_at(klines, idx - 1, period, std_dev_mult);
            cur_width < prev_width && prev_width > 0.0
        };

        // Trend filter via EMA
        let ema_val = indicators::ema_at(klines, idx, ema_period);
        let uptrend = price > ema_val;
        let downtrend = price < ema_val;

        if price <= lower {
            // Lower band touch: buy if squeeze or uptrend
            if is_squeeze || (use_trend == 0) || (use_trend == 1 && uptrend) {
                return 1;
            }
        } else if price >= upper {
            // Upper band touch: sell if squeeze or downtrend
            if is_squeeze || (use_trend == 0) || (use_trend == 1 && downtrend) {
                return -1;
            }
        }

        0
    }
}
