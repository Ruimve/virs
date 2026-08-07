

use serde::{Deserialize, Serialize};
use virs_type::Timeframe;


#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IndicatorSpec {


    CurrentPrice { tf: Timeframe },

    ChangePct { tf: Timeframe, period: usize },

    CandleBody { tf: Timeframe },

    LastCompletedVolume { tf: Timeframe },


    Ema { tf: Timeframe, period: usize },
    BbandsUpper { tf: Timeframe, period: usize, stddev: u32 },
    BbandsMiddle { tf: Timeframe, period: usize, stddev: u32 },
    BbandsLower { tf: Timeframe, period: usize, stddev: u32 },

    BbandsWidth { tf: Timeframe, period: usize, stddev: u32 },


    Rsi { tf: Timeframe, period: usize },
    Macd { tf: Timeframe, fast: usize, slow: usize, signal: usize },
    MacdSignal { tf: Timeframe, fast: usize, slow: usize, signal: usize },
    MacdHistogram { tf: Timeframe, fast: usize, slow: usize, signal: usize },
    Adx { tf: Timeframe, period: usize },


    Atr { tf: Timeframe, period: usize },

    AtrPct { tf: Timeframe, period: usize },

    AtrSma { tf: Timeframe, atr_period: usize, sma_period: usize },


    Highest { tf: Timeframe, period: usize },
    Lowest { tf: Timeframe, period: usize },


    VolumeSma { tf: Timeframe, period: usize },


    BarsOutsideBand { tf: Timeframe, period: usize, stddev: u32 },

    EmaCrossBarsAgo { tf: Timeframe, fast: usize, slow: usize },

    EmaGapPct { tf: Timeframe, fast: usize, slow: usize },

    EmaGapTrend { tf: Timeframe, fast: usize, slow: usize },

    EmaCrossState { tf: Timeframe, fast: usize, slow: usize },

    BandwidthBarsAgo { tf: Timeframe, period: usize, stddev: u32, bars_ago: usize },

    RoundNumberUp,

    RoundNumberDown,


    FundingRate,
    FundingNextTime,
}

impl IndicatorSpec {

    pub fn timeframe(&self) -> Option<Timeframe> {
        match self {
            IndicatorSpec::RoundNumberUp | IndicatorSpec::RoundNumberDown => None,
            IndicatorSpec::FundingRate | IndicatorSpec::FundingNextTime => None,
            IndicatorSpec::CurrentPrice { tf }
            | IndicatorSpec::ChangePct { tf, .. }
            | IndicatorSpec::CandleBody { tf }
            | IndicatorSpec::LastCompletedVolume { tf }
            | IndicatorSpec::Ema { tf, .. }
            | IndicatorSpec::BbandsUpper { tf, .. }
            | IndicatorSpec::BbandsMiddle { tf, .. }
            | IndicatorSpec::BbandsLower { tf, .. }
            | IndicatorSpec::BbandsWidth { tf, .. }
            | IndicatorSpec::Rsi { tf, .. }
            | IndicatorSpec::Macd { tf, .. }
            | IndicatorSpec::MacdSignal { tf, .. }
            | IndicatorSpec::MacdHistogram { tf, .. }
            | IndicatorSpec::Adx { tf, .. }
            | IndicatorSpec::Atr { tf, .. }
            | IndicatorSpec::AtrPct { tf, .. }
            | IndicatorSpec::AtrSma { tf, .. }
            | IndicatorSpec::Highest { tf, .. }
            | IndicatorSpec::Lowest { tf, .. }
            | IndicatorSpec::VolumeSma { tf, .. }
            | IndicatorSpec::BarsOutsideBand { tf, .. }
            | IndicatorSpec::EmaCrossBarsAgo { tf, .. }
            | IndicatorSpec::EmaGapPct { tf, .. }
            | IndicatorSpec::EmaGapTrend { tf, .. }
            | IndicatorSpec::EmaCrossState { tf, .. }
            | IndicatorSpec::BandwidthBarsAgo { tf, .. } => Some(*tf),
        }
    }
}
