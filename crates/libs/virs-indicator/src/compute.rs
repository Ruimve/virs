//! 指标计算入口。
//!
//! [`compute_indicators`] 接收三周期 K 线 + 资金费率，返回 [`IndicatorSet`]。
//! 调用方按需通过 [`IndicatorSet::get`] / [`get_num`] / [`get_str`] 查询具体指标值。

use virs_error::VirsResult;
use virs_types::Kline;

use crate::set::{IndicatorSet, KlineSet};
use crate::spec::IndicatorSpec;

/// 计算指标并返回 [`IndicatorSet`]。
///
/// - `specs` 为 `None` 时计算 [`default_specs`] 全量指标。
/// - `specs` 为 `Some` 时只计算声明的指标（自动去重）。
///
/// K 线数据不足时返回 `Err`，不使用默认值。
pub fn compute_indicators(
    klines_1h: &[Kline],
    klines_4h: &[Kline],
    klines_15m: &[Kline],
    funding_rate: f64,
    funding_next_time: &str,
    specs: Option<&[IndicatorSpec]>,
) -> VirsResult<IndicatorSet> {
    let kline_set = KlineSet {
        h1: klines_1h,
        h4: klines_4h,
        m15: klines_15m,
    };
    match specs {
        Some(s) => IndicatorSet::compute(s, &kline_set, funding_rate, funding_next_time),
        None => {
            let default = default_specs();
            IndicatorSet::compute(&default, &kline_set, funding_rate, funding_next_time)
        }
    }
}

/// 默认全量指标清单。
///
/// 覆盖 H1 / M15 / H4 三周期的全部指标，与旧 `all_market_indicators_specs` 对应，
/// 新增 `EmaCrossState`（原在 render 层 if-else 派生）。
pub fn default_specs() -> Vec<IndicatorSpec> {
    use IndicatorSpec::*;
    use virs_types::Timeframe::*;
    vec![
        // H1 主周期
        CurrentPrice { tf: H1 },
        Rsi { tf: H1, period: 14 },
        Atr { tf: H1, period: 14 },
        AtrPct { tf: H1, period: 14 },
        BbandsWidth { tf: H1, period: 20, stddev: 2 },
        BbandsUpper { tf: H1, period: 20, stddev: 2 },
        BbandsMiddle { tf: H1, period: 20, stddev: 2 },
        BbandsLower { tf: H1, period: 20, stddev: 2 },
        Ema { tf: H1, period: 12 },
        Ema { tf: H1, period: 20 },
        Ema { tf: H1, period: 26 },
        Ema { tf: H1, period: 50 },
        Macd { tf: H1, fast: 12, slow: 26, signal: 9 },
        MacdSignal { tf: H1, fast: 12, slow: 26, signal: 9 },
        MacdHistogram { tf: H1, fast: 12, slow: 26, signal: 9 },
        Adx { tf: H1, period: 14 },
        ChangePct { tf: H1, period: 1 },
        AtrSma { tf: H1, atr_period: 14, sma_period: 20 },
        CandleBody { tf: H1 },
        BarsOutsideBand { tf: H1, period: 20, stddev: 2 },
        BandwidthBarsAgo { tf: H1, period: 20, stddev: 2, bars_ago: 5 },
        Highest { tf: H1, period: 20 },
        Lowest { tf: H1, period: 20 },
        RoundNumberUp,
        RoundNumberDown,
        LastCompletedVolume { tf: H1 },
        VolumeSma { tf: H1, period: 20 },
        EmaCrossBarsAgo { tf: H1, fast: 20, slow: 50 },
        EmaGapPct { tf: H1, fast: 20, slow: 50 },
        EmaGapTrend { tf: H1, fast: 20, slow: 50 },
        EmaCrossState { tf: H1, fast: 20, slow: 50 },
        Highest { tf: H1, period: 50 },
        Lowest { tf: H1, period: 50 },
        // M15 快周期
        CurrentPrice { tf: M15 },
        Rsi { tf: M15, period: 14 },
        Macd { tf: M15, fast: 12, slow: 26, signal: 9 },
        MacdSignal { tf: M15, fast: 12, slow: 26, signal: 9 },
        MacdHistogram { tf: M15, fast: 12, slow: 26, signal: 9 },
        BbandsWidth { tf: M15, period: 20, stddev: 2 },
        Atr { tf: M15, period: 14 },
        AtrSma { tf: M15, atr_period: 14, sma_period: 20 },
        Adx { tf: M15, period: 14 },
        BarsOutsideBand { tf: M15, period: 20, stddev: 2 },
        Ema { tf: M15, period: 20 },
        Ema { tf: M15, period: 50 },
        LastCompletedVolume { tf: M15 },
        VolumeSma { tf: M15, period: 20 },
        EmaCrossBarsAgo { tf: M15, fast: 20, slow: 50 },
        EmaCrossState { tf: M15, fast: 20, slow: 50 },
        Highest { tf: M15, period: 50 },
        Lowest { tf: M15, period: 50 },
        // H4 慢周期
        Ema { tf: H4, period: 20 },
        Ema { tf: H4, period: 50 },
        Adx { tf: H4, period: 14 },
        BbandsWidth { tf: H4, period: 20, stddev: 2 },
        Rsi { tf: H4, period: 14 },
        Macd { tf: H4, fast: 12, slow: 26, signal: 9 },
        MacdSignal { tf: H4, fast: 12, slow: 26, signal: 9 },
        MacdHistogram { tf: H4, fast: 12, slow: 26, signal: 9 },
        // 资金费率
        FundingRate,
        FundingNextTime,
    ]
}
