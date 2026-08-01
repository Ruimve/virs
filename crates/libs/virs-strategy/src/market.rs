//! 指标向后兼容层。
//!
//! 本模块保留 [`MarketIndicators`] 作为上游 `MarketDataProvider` 与 bot worker 之间的
//! JSON 传输格式（`indicators_json`），但内部计算已委托给
//! [`crate::indicator`] 统一指标库。
//!
//! 原子计算函数从本模块迁移至 `indicator::library`，此处通过 `pub use`
//! 转发以保持向后兼容（`market_data.rs` 等外部调用方无需改动）。

use virs_error::VirsError;
use virs_types::{Kline, MarketIndicators, Timeframe};

use crate::indicator::{
    all_market_indicators_specs, IndicatorSet, IndicatorSpec, KlineSet,
};

// 重新导出原子计算函数，保持向后兼容。
pub use crate::indicator::library::{
    adx_at, atr, atr_at, bbands_at, bbands_width_at, closes, compute_bars_outside_band,
    compute_ema_cross_bars_ago, ema_at, find_round_number, highest_at, highs, lows, macd_at,
    macd_histogram_at, macd_signal_at, rsi_at, sma_at_from, volume_sma_at,
};

/// 从 [`IndicatorSet`] 构造 [`MarketIndicators`]。要求 set 已计算 [`all_market_indicators_specs`] 的全部 specs。
///
/// 缺失某个 spec 属于编程错误（spec 列表不完整），返回 `Err` 而非静默回退默认值。
pub fn build_market_indicators(set: &IndicatorSet) -> Result<MarketIndicators, VirsError> {
    use IndicatorSpec::*;
    let num = |spec: IndicatorSpec| -> Result<f64, VirsError> {
        set.get_num(&spec).ok_or_else(|| {
            VirsError::config(format!(
                "Indicator {:?} missing from IndicatorSet — spec list is incomplete (programming error)",
                spec
            ))
        })
    };
    let int = |spec: IndicatorSpec| -> Result<i32, VirsError> {
        set.get_int(&spec).ok_or_else(|| {
            VirsError::config(format!(
                "Indicator {:?} missing from IndicatorSet — spec list is incomplete (programming error)",
                spec
            ))
        })
    };
    let s = |spec: IndicatorSpec| -> Result<String, VirsError> {
        set.get_str(&spec).ok_or_else(|| {
            VirsError::config(format!(
                "Indicator {:?} missing from IndicatorSet — spec list is incomplete (programming error)",
                spec
            ))
        }).map(|v| v.to_string())
    };

    Ok(MarketIndicators {
        current_price: num(CurrentPrice { tf: Timeframe::H1 })?,
        rsi: num(Rsi { tf: Timeframe::H1, period: 14 })?,
        atr: num(Atr { tf: Timeframe::H1, period: 14 })?,
        atr_pct: num(AtrPct { tf: Timeframe::H1, period: 14 })?,
        bb_width: num(BbandsWidth { tf: Timeframe::H1, period: 20, stddev: 2 })?,
        bb_upper: num(BbandsUpper { tf: Timeframe::H1, period: 20, stddev: 2 })?,
        bb_middle: num(BbandsMiddle { tf: Timeframe::H1, period: 20, stddev: 2 })?,
        bb_lower: num(BbandsLower { tf: Timeframe::H1, period: 20, stddev: 2 })?,
        ema12: num(Ema { tf: Timeframe::H1, period: 12 })?,
        ema20: num(Ema { tf: Timeframe::H1, period: 20 })?,
        ema26: num(Ema { tf: Timeframe::H1, period: 26 })?,
        ema50: num(Ema { tf: Timeframe::H1, period: 50 })?,
        macd: num(Macd { tf: Timeframe::H1, fast: 12, slow: 26, signal: 9 })?,
        macd_signal: num(MacdSignal { tf: Timeframe::H1, fast: 12, slow: 26, signal: 9 })?,
        macd_histogram: num(MacdHistogram { tf: Timeframe::H1, fast: 12, slow: 26, signal: 9 })?,
        adx: num(Adx { tf: Timeframe::H1, period: 14 })?,
        change_1h: num(ChangePct { tf: Timeframe::H1, period: 1 })?,
        h1_atr_sma20: num(AtrSma { tf: Timeframe::H1, atr_period: 14, sma_period: 20 })?,
        h1_candle_body: num(CandleBody { tf: Timeframe::H1 })?,
        h1_bars_outside_band: int(BarsOutsideBand { tf: Timeframe::H1, period: 20, stddev: 2 })?,
        h1_bandwidth_5bars_ago: num(BandwidthBarsAgo { tf: Timeframe::H1, period: 20, stddev: 2, bars_ago: 5 })?,
        h1_high_20: num(Highest { tf: Timeframe::H1, period: 20 })?,
        h1_low_20: num(Lowest { tf: Timeframe::H1, period: 20 })?,
        nearest_round_up: num(RoundNumberUp)?,
        nearest_round_down: num(RoundNumberDown)?,
        h1_volume: num(LastCompletedVolume { tf: Timeframe::H1 })?,
        h1_volume_sma20: num(VolumeSma { tf: Timeframe::H1, period: 20 })?,
        h1_ema_cross_bars_ago: int(EmaCrossBarsAgo { tf: Timeframe::H1, fast: 20, slow: 50 })?,
        h1_ema_gap_pct: num(EmaGapPct { tf: Timeframe::H1, fast: 20, slow: 50 })?,
        h1_ema_gap_trend: s(EmaGapTrend { tf: Timeframe::H1, fast: 20, slow: 50 })?,
        h1_high_50: num(Highest { tf: Timeframe::H1, period: 50 })?,
        h1_low_50: num(Lowest { tf: Timeframe::H1, period: 50 })?,

        m15_current_price: num(CurrentPrice { tf: Timeframe::M15 })?,
        m15_rsi: num(Rsi { tf: Timeframe::M15, period: 14 })?,
        m15_macd: num(Macd { tf: Timeframe::M15, fast: 12, slow: 26, signal: 9 })?,
        m15_macd_signal: num(MacdSignal { tf: Timeframe::M15, fast: 12, slow: 26, signal: 9 })?,
        m15_macd_histogram: num(MacdHistogram { tf: Timeframe::M15, fast: 12, slow: 26, signal: 9 })?,
        m15_bb_width_pct: num(BbandsWidth { tf: Timeframe::M15, period: 20, stddev: 2 })?,
        m15_atr: num(Atr { tf: Timeframe::M15, period: 14 })?,
        m15_atr_sma20: num(AtrSma { tf: Timeframe::M15, atr_period: 14, sma_period: 20 })?,
        m15_adx: num(Adx { tf: Timeframe::M15, period: 14 })?,
        m15_bars_outside_band: int(BarsOutsideBand { tf: Timeframe::M15, period: 20, stddev: 2 })?,
        m15_ema20: num(Ema { tf: Timeframe::M15, period: 20 })?,
        m15_ema50: num(Ema { tf: Timeframe::M15, period: 50 })?,
        m15_volume: num(LastCompletedVolume { tf: Timeframe::M15 })?,
        m15_volume_sma20: num(VolumeSma { tf: Timeframe::M15, period: 20 })?,
        m15_ema_cross_bars_ago: int(EmaCrossBarsAgo { tf: Timeframe::M15, fast: 20, slow: 50 })?,
        m15_high_50: num(Highest { tf: Timeframe::M15, period: 50 })?,
        m15_low_50: num(Lowest { tf: Timeframe::M15, period: 50 })?,

        h4_ema20: num(Ema { tf: Timeframe::H4, period: 20 })?,
        h4_ema50: num(Ema { tf: Timeframe::H4, period: 50 })?,
        h4_adx: num(Adx { tf: Timeframe::H4, period: 14 })?,
        h4_bb_width_pct: num(BbandsWidth { tf: Timeframe::H4, period: 20, stddev: 2 })?,
        h4_rsi: num(Rsi { tf: Timeframe::H4, period: 14 })?,
        h4_macd: num(Macd { tf: Timeframe::H4, fast: 12, slow: 26, signal: 9 })?,
        h4_macd_signal: num(MacdSignal { tf: Timeframe::H4, fast: 12, slow: 26, signal: 9 })?,
        h4_macd_histogram: num(MacdHistogram { tf: Timeframe::H4, fast: 12, slow: 26, signal: 9 })?,

        funding_rate: num(FundingRate)?,
        funding_next_time: s(FundingNextTime)?,
    })
}

/// 计算全部市场指标。行为与重构前完全等价（由 [`IndicatorSet::compute`] 驱动）。
pub fn compute_market_indicators(
    klines_1h: &[Kline],
    klines_4h: &[Kline],
    klines_15m: &[Kline],
    funding_rate: f64,
    funding_next_time: String,
) -> Result<MarketIndicators, VirsError> {
    let kline_set = KlineSet {
        h1: klines_1h,
        h4: klines_4h,
        m15: klines_15m,
    };
    let specs = all_market_indicators_specs();
    let set = IndicatorSet::compute(&specs, &kline_set, funding_rate, &funding_next_time)?;
    build_market_indicators(&set)
}
