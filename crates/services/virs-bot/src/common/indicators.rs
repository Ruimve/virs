//! 指标向后兼容层。
//!
//! 本模块保留 [`MarketIndicators`] 作为上游 `MarketDataProvider` 与 bot worker 之间的
//! JSON 传输格式（`indicators_json`），但内部计算已委托给
//! [`crate::strategy::indicator`] 统一指标库。
//!
//! 原子计算函数从本模块迁移至 `strategy::indicator::library`，此处通过 `pub use`
//! 转发以保持向后兼容（`market_data.rs` 等外部调用方无需改动）。

use virs_models::Kline;

use crate::strategy::indicator::{
    all_market_indicators_specs, IndicatorSet, IndicatorSpec, KlineSet, Timeframe,
};

// 重新导出原子计算函数，保持向后兼容。
pub use crate::strategy::indicator::library::{
    adx_at, atr, atr_at, bbands_at, bbands_width_at, closes, compute_bars_outside_band,
    compute_ema_cross_bars_ago, ema_at, find_round_number, highest_at, highs, lows, macd_at,
    macd_histogram_at, macd_signal_at, rsi_at, sma_at_from, volume_sma_at,
};

/// 市场指标快照。作为 `indicators_json` 的 JSON 传输格式。
///
/// 注意：本 struct 仅用于过渡期的 JSON 序列化/反序列化。
/// 新代码应直接使用 [`crate::strategy::indicator::IndicatorSet`]。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MarketIndicators {
    pub current_price: f64,
    pub rsi: f64,
    pub atr: f64,
    pub atr_pct: f64,
    pub bb_width: f64,
    pub bb_upper: f64,
    pub bb_middle: f64,
    pub bb_lower: f64,
    pub ema12: f64,
    pub ema20: f64,
    pub ema26: f64,
    pub ema50: f64,
    pub macd: f64,
    pub macd_signal: f64,
    pub macd_histogram: f64,
    pub adx: f64,
    pub change_1h: f64,
    pub h1_atr_sma20: f64,
    pub h1_candle_body: f64,
    pub h1_bars_outside_band: i32,
    pub h1_bandwidth_5bars_ago: f64,
    pub h1_high_20: f64,
    pub h1_low_20: f64,
    pub nearest_round_up: f64,
    pub nearest_round_down: f64,
    pub h1_volume: f64,
    pub h1_volume_sma20: f64,
    pub h1_ema_cross_bars_ago: i32,
    pub h1_ema_gap_pct: f64,
    pub h1_ema_gap_trend: String,
    pub h1_high_50: f64,
    pub h1_low_50: f64,

    pub m15_current_price: f64,
    pub m15_rsi: f64,
    pub m15_macd: f64,
    pub m15_macd_signal: f64,
    pub m15_macd_histogram: f64,
    pub m15_bb_width_pct: f64,
    pub m15_atr: f64,
    pub m15_atr_sma20: f64,
    pub m15_adx: f64,
    pub m15_bars_outside_band: i32,
    pub m15_ema20: f64,
    pub m15_ema50: f64,
    pub m15_volume: f64,
    pub m15_volume_sma20: f64,
    pub m15_ema_cross_bars_ago: i32,
    pub m15_high_50: f64,
    pub m15_low_50: f64,

    pub h4_ema20: f64,
    pub h4_ema50: f64,
    pub h4_adx: f64,
    pub h4_bb_width_pct: f64,
    pub h4_rsi: f64,
    pub h4_macd: f64,
    pub h4_macd_signal: f64,
    pub h4_macd_histogram: f64,

    pub funding_rate: f64,
    pub funding_next_time: String,
}

impl MarketIndicators {
    /// 从 [`IndicatorSet`] 构造。要求 set 已计算 [`all_market_indicators_specs`] 的全部 specs。
    ///
    /// 缺失某个 spec 属于编程错误（spec 列表不完整），记 `error!` 后回退 0.0/空串，
    /// 以避免 panic 中断交易引擎，同时通过日志暴露问题。
    pub fn from_indicator_set(set: &IndicatorSet) -> Self {
        use IndicatorSpec::*;
        let num = |spec: IndicatorSpec| -> f64 {
            set.get_num(&spec).unwrap_or_else(|| {
                tracing::error!(?spec, "指标缺失 — from_indicator_set 收到不完整的 IndicatorSet，回退 0.0");
                0.0
            })
        };
        let int = |spec: IndicatorSpec| -> i32 {
            set.get_int(&spec).unwrap_or_else(|| {
                tracing::error!(?spec, "指标缺失 — from_indicator_set 收到不完整的 IndicatorSet，回退 0");
                0
            })
        };
        let s = |spec: IndicatorSpec| -> String {
            set.get_str(&spec).unwrap_or_else(|| {
                tracing::error!(?spec, "指标缺失 — from_indicator_set 收到不完整的 IndicatorSet，回退空串");
                ""
            })
            .to_string()
        };

        Self {
            current_price: num(CurrentPrice { tf: Timeframe::H1 }),
            rsi: num(Rsi { tf: Timeframe::H1, period: 14 }),
            atr: num(Atr { tf: Timeframe::H1, period: 14 }),
            atr_pct: num(AtrPct { tf: Timeframe::H1, period: 14 }),
            bb_width: num(BbandsWidth { tf: Timeframe::H1, period: 20, stddev: 2 }),
            bb_upper: num(BbandsUpper { tf: Timeframe::H1, period: 20, stddev: 2 }),
            bb_middle: num(BbandsMiddle { tf: Timeframe::H1, period: 20, stddev: 2 }),
            bb_lower: num(BbandsLower { tf: Timeframe::H1, period: 20, stddev: 2 }),
            ema12: num(Ema { tf: Timeframe::H1, period: 12 }),
            ema20: num(Ema { tf: Timeframe::H1, period: 20 }),
            ema26: num(Ema { tf: Timeframe::H1, period: 26 }),
            ema50: num(Ema { tf: Timeframe::H1, period: 50 }),
            macd: num(Macd { tf: Timeframe::H1, fast: 12, slow: 26, signal: 9 }),
            macd_signal: num(MacdSignal { tf: Timeframe::H1, fast: 12, slow: 26, signal: 9 }),
            macd_histogram: num(MacdHistogram { tf: Timeframe::H1, fast: 12, slow: 26, signal: 9 }),
            adx: num(Adx { tf: Timeframe::H1, period: 14 }),
            change_1h: num(ChangePct { tf: Timeframe::H1, period: 1 }),
            h1_atr_sma20: num(AtrSma { tf: Timeframe::H1, atr_period: 14, sma_period: 20 }),
            h1_candle_body: num(CandleBody { tf: Timeframe::H1 }),
            h1_bars_outside_band: int(BarsOutsideBand { tf: Timeframe::H1, period: 20, stddev: 2 }),
            h1_bandwidth_5bars_ago: num(BandwidthBarsAgo { tf: Timeframe::H1, period: 20, stddev: 2, bars_ago: 5 }),
            h1_high_20: num(Highest { tf: Timeframe::H1, period: 20 }),
            h1_low_20: num(Lowest { tf: Timeframe::H1, period: 20 }),
            nearest_round_up: num(RoundNumberUp),
            nearest_round_down: num(RoundNumberDown),
            h1_volume: num(LastCompletedVolume { tf: Timeframe::H1 }),
            h1_volume_sma20: num(VolumeSma { tf: Timeframe::H1, period: 20 }),
            h1_ema_cross_bars_ago: int(EmaCrossBarsAgo { tf: Timeframe::H1, fast: 20, slow: 50 }),
            h1_ema_gap_pct: num(EmaGapPct { tf: Timeframe::H1, fast: 20, slow: 50 }),
            h1_ema_gap_trend: s(EmaGapTrend { tf: Timeframe::H1, fast: 20, slow: 50 }),
            h1_high_50: num(Highest { tf: Timeframe::H1, period: 50 }),
            h1_low_50: num(Lowest { tf: Timeframe::H1, period: 50 }),

            m15_current_price: num(CurrentPrice { tf: Timeframe::M15 }),
            m15_rsi: num(Rsi { tf: Timeframe::M15, period: 14 }),
            m15_macd: num(Macd { tf: Timeframe::M15, fast: 12, slow: 26, signal: 9 }),
            m15_macd_signal: num(MacdSignal { tf: Timeframe::M15, fast: 12, slow: 26, signal: 9 }),
            m15_macd_histogram: num(MacdHistogram { tf: Timeframe::M15, fast: 12, slow: 26, signal: 9 }),
            m15_bb_width_pct: num(BbandsWidth { tf: Timeframe::M15, period: 20, stddev: 2 }),
            m15_atr: num(Atr { tf: Timeframe::M15, period: 14 }),
            m15_atr_sma20: num(AtrSma { tf: Timeframe::M15, atr_period: 14, sma_period: 20 }),
            m15_adx: num(Adx { tf: Timeframe::M15, period: 14 }),
            m15_bars_outside_band: int(BarsOutsideBand { tf: Timeframe::M15, period: 20, stddev: 2 }),
            m15_ema20: num(Ema { tf: Timeframe::M15, period: 20 }),
            m15_ema50: num(Ema { tf: Timeframe::M15, period: 50 }),
            m15_volume: num(LastCompletedVolume { tf: Timeframe::M15 }),
            m15_volume_sma20: num(VolumeSma { tf: Timeframe::M15, period: 20 }),
            m15_ema_cross_bars_ago: int(EmaCrossBarsAgo { tf: Timeframe::M15, fast: 20, slow: 50 }),
            m15_high_50: num(Highest { tf: Timeframe::M15, period: 50 }),
            m15_low_50: num(Lowest { tf: Timeframe::M15, period: 50 }),

            h4_ema20: num(Ema { tf: Timeframe::H4, period: 20 }),
            h4_ema50: num(Ema { tf: Timeframe::H4, period: 50 }),
            h4_adx: num(Adx { tf: Timeframe::H4, period: 14 }),
            h4_bb_width_pct: num(BbandsWidth { tf: Timeframe::H4, period: 20, stddev: 2 }),
            h4_rsi: num(Rsi { tf: Timeframe::H4, period: 14 }),
            h4_macd: num(Macd { tf: Timeframe::H4, fast: 12, slow: 26, signal: 9 }),
            h4_macd_signal: num(MacdSignal { tf: Timeframe::H4, fast: 12, slow: 26, signal: 9 }),
            h4_macd_histogram: num(MacdHistogram { tf: Timeframe::H4, fast: 12, slow: 26, signal: 9 }),

            funding_rate: num(FundingRate),
            funding_next_time: s(FundingNextTime),
        }
    }
}

/// 计算全部市场指标。行为与重构前完全等价（由 [`IndicatorSet::compute`] 驱动）。
pub fn compute_market_indicators(
    klines_1h: &[Kline],
    klines_4h: &[Kline],
    klines_15m: &[Kline],
    funding_rate: f64,
    funding_next_time: String,
) -> MarketIndicators {
    let kline_set = KlineSet {
        h1: klines_1h,
        h4: klines_4h,
        m15: klines_15m,
    };
    let specs = all_market_indicators_specs();
    let set = IndicatorSet::compute(&specs, &kline_set, funding_rate, &funding_next_time);
    MarketIndicators::from_indicator_set(&set)
}
