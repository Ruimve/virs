use crate::engine::backtest::compute_bollinger_bands;
use crate::engine::plugin::*;
use crate::models::Kline;
use std::collections::HashMap;

pub struct BollingerBandsPlugin;

impl IndicatorPlugin for BollingerBandsPlugin {
    fn name(&self) -> &str {
        "bollinger_bands"
    }
    fn description(&self) -> &str {
        "Bollinger Bands: Buy when price touches lower band, sell when price touches upper band."
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
        ]
    }

    fn signal(&self, klines: &[Kline], idx: usize, params: &HashMap<String, f64>) -> i8 {
        let period = params.get("period").map(|v| *v as usize).unwrap_or(20);
        let std_dev_mult = params.get("std_dev").copied().unwrap_or(2.0);

        if idx < period {
            return 0;
        }

        let (upper, _middle, lower) = compute_bollinger_bands(klines, idx, period, std_dev_mult);
        let price = klines[idx].close;

        if price <= lower {
            1 // Price at lower band - buy
        } else if price >= upper {
            -1 // Price at upper band - sell
        } else {
            0
        }
    }
}
