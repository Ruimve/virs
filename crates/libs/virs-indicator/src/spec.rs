//! 指标规格声明。
//!
//! 策略通过返回 `Vec<IndicatorSpec>` 声明所需指标，主程序据此统一计算注入。
//! `IndicatorSpec` 作为 `Hash + Eq` 的 key，支持去重批量计算。
//!
//! 设计说明：
//! - `stddev` 使用 `u32` 而非 `f64`（f64 不实现 Eq/Hash），实践中布林带标准差恒为整数
//! - 衍生指标（AtrPct / BbandsWidth / EmaGapPct 等）单独建模为 spec，
//!   计算时内部调用原子函数，避免依赖其他 spec 的查找结果

use serde::{Deserialize, Serialize};
use virs_type::Timeframe;

/// 指标规格。每个 variant 自包含计算所需的全部参数。
///
/// 每个 variant 自包含计算所需的全部参数（周期、标准差等），
/// 不依赖外部上下文，便于去重与独立计算。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IndicatorSpec {
    // ── 价格衍生 ──
    /// 当前价格（指定周期的最新收盘价）
    CurrentPrice { tf: Timeframe },
    /// N 根 K 线前的收盘价到当前的涨跌幅（百分比）
    ChangePct { tf: Timeframe, period: usize },
    /// 最新 K 线实体（close - open）
    CandleBody { tf: Timeframe },
    /// 最后一根已完成 K 线的成交量
    LastCompletedVolume { tf: Timeframe },

    // ── TA-Lib overlap ──
    Ema { tf: Timeframe, period: usize },
    BbandsUpper { tf: Timeframe, period: usize, stddev: u32 },
    BbandsMiddle { tf: Timeframe, period: usize, stddev: u32 },
    BbandsLower { tf: Timeframe, period: usize, stddev: u32 },
    /// 布林带宽度比 (upper - lower) / middle
    BbandsWidth { tf: Timeframe, period: usize, stddev: u32 },

    // ── TA-Lib momentum ──
    Rsi { tf: Timeframe, period: usize },
    Macd { tf: Timeframe, fast: usize, slow: usize, signal: usize },
    MacdSignal { tf: Timeframe, fast: usize, slow: usize, signal: usize },
    MacdHistogram { tf: Timeframe, fast: usize, slow: usize, signal: usize },
    Adx { tf: Timeframe, period: usize },

    // ── TA-Lib volatility ──
    Atr { tf: Timeframe, period: usize },
    /// ATR 占价格的百分比 (atr / price * 100)
    AtrPct { tf: Timeframe, period: usize },
    /// ATR 序列的 SMA
    AtrSma { tf: Timeframe, atr_period: usize, sma_period: usize },

    // ── TA-Lib math operator ──
    Highest { tf: Timeframe, period: usize },
    Lowest { tf: Timeframe, period: usize },

    // ── 成交量 ──
    VolumeSma { tf: Timeframe, period: usize },

    // ── 自定义复合指标 ──
    /// 连续出轨 K 线数（正=超上轨，负=破下轨）
    BarsOutsideBand { tf: Timeframe, period: usize, stddev: u32 },
    /// EMA 金叉/死叉距今 K 线数
    EmaCrossBarsAgo { tf: Timeframe, fast: usize, slow: usize },
    /// EMA 间距百分比 (ema_fast - ema_slow) / ema_slow * 100
    EmaGapPct { tf: Timeframe, fast: usize, slow: usize },
    /// EMA 间距趋势（"扩大" / "缩小" / "持平"）
    EmaGapTrend { tf: Timeframe, fast: usize, slow: usize },
    /// EMA 交叉状态（"金叉(多头)" / "死叉(空头)"）
    EmaCrossState { tf: Timeframe, fast: usize, slow: usize },
    /// N 根 K 线前的布林带宽度
    BandwidthBarsAgo { tf: Timeframe, period: usize, stddev: u32, bars_ago: usize },
    /// 向上取整的整数关口
    RoundNumberUp,
    /// 向下取整的整数关口
    RoundNumberDown,

    // ── 资金费率 ──
    FundingRate,
    FundingNextTime,
}

impl IndicatorSpec {
    /// 返回该指标关联的周期；无周期的指标（整数关口、资金费率）返回 None。
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
