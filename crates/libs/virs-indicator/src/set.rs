//! 指标集合：批量计算 + 查询。
//!
//! [`IndicatorSet::compute`] 接收策略声明的 specs，去重后逐个委托到
//! [`crate::indicators`] 各模块的 `compute` 函数计算。

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use virs_error::VirsError;
use virs_types::{Kline, Timeframe};

use crate::indicators;
use crate::spec::IndicatorSpec;

/// 三周期 K 线输入。
#[derive(Debug, Clone, Copy)]
pub struct KlineSet<'a> {
    pub h1: &'a [Kline],
    pub h4: &'a [Kline],
    pub m15: &'a [Kline],
}

/// 指标值。不同指标返回不同类型（数值/整数/字符串）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum IndicatorValue {
    Num(f64),
    Int(i32),
    Str(String),
}

/// 已计算的指标集合。通过 [`IndicatorSpec`] 查询。
///
/// 序列化为 `Vec<(IndicatorSpec, IndicatorValue)>`（JSON 数组）。
#[derive(Debug, Clone, Default)]
pub struct IndicatorSet {
    values: HashMap<IndicatorSpec, IndicatorValue>,
}

impl Serialize for IndicatorSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let pairs: Vec<(&IndicatorSpec, &IndicatorValue)> = self.values.iter().collect();
        pairs.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for IndicatorSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let pairs: Vec<(IndicatorSpec, IndicatorValue)> = Vec::deserialize(deserializer)?;
        let values = pairs.into_iter().collect();
        Ok(Self { values })
    }
}

impl IndicatorSet {
    /// 按策略声明的 specs 批量计算指标。自动去重。
    pub fn compute(
        specs: &[IndicatorSpec],
        klines: &KlineSet,
        funding_rate: f64,
        funding_next_time: &str,
    ) -> Result<Self, VirsError> {
        let unique: HashSet<&IndicatorSpec> = specs.iter().collect();
        let mut values = HashMap::with_capacity(unique.len());
        for spec in unique {
            let val = compute_one(spec, klines, funding_rate, funding_next_time)?;
            values.insert(spec.clone(), val);
        }
        Ok(Self { values })
    }

    /// 插入一个指标值（用于手动构造或测试）。
    pub fn insert(&mut self, spec: IndicatorSpec, value: IndicatorValue) -> &mut Self {
        self.values.insert(spec, value);
        self
    }

    /// 从单个指标值构造。
    pub fn with_value(spec: IndicatorSpec, value: IndicatorValue) -> Self {
        let mut set = Self::default();
        set.values.insert(spec, value);
        set
    }

    /// 查询指标值。缺失返回 `None`。
    pub fn get(&self, spec: &IndicatorSpec) -> Option<&IndicatorValue> {
        self.values.get(spec)
    }

    /// 便捷查询数值型指标。
    pub fn get_num(&self, spec: &IndicatorSpec) -> Option<f64> {
        match self.values.get(spec)? {
            IndicatorValue::Num(v) => Some(*v),
            _ => None,
        }
    }

    /// 便捷查询整数型指标。
    pub fn get_int(&self, spec: &IndicatorSpec) -> Option<i32> {
        match self.values.get(spec)? {
            IndicatorValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    /// 便捷查询字符串型指标。
    pub fn get_str(&self, spec: &IndicatorSpec) -> Option<&str> {
        match self.values.get(spec)? {
            IndicatorValue::Str(v) => Some(v.as_str()),
            _ => None,
        }
    }
}

fn klines_for_tf<'a>(tf: Timeframe, klines: &KlineSet<'a>) -> &'a [Kline] {
    match tf {
        Timeframe::H1 => klines.h1,
        Timeframe::H4 => klines.h4,
        Timeframe::M15 => klines.m15,
        _ => &[],
    }
}

/// 构造数据不足的错误。
fn no_data(spec: &IndicatorSpec, tf: Option<Timeframe>, len: usize) -> VirsError {
    let tf_str = tf.map(|t| t.as_str()).unwrap_or("N/A");
    VirsError::config(format!(
        "Insufficient K-line data for indicator {:?} (tf={}, klines_len={}) — \
         cannot compute indicator with default value",
        spec, tf_str, len
    ))
}

/// 计算单个指标。委托到 indicators 各模块。
fn compute_one(
    spec: &IndicatorSpec,
    klines: &KlineSet,
    funding_rate: f64,
    funding_next_time: &str,
) -> Result<IndicatorValue, VirsError> {
    use IndicatorSpec::*;

    match spec {
        // ── 外部数据直通 ──
        FundingRate => Ok(IndicatorValue::Num(funding_rate)),
        FundingNextTime => Ok(IndicatorValue::Str(funding_next_time.to_string())),

        // ── 整数关口（基于 H1 当前价）──
        RoundNumberUp => {
            let price = klines.h1.last().map(|k| k.close)
                .ok_or_else(|| no_data(spec, None, klines.h1.len()))?;
            Ok(IndicatorValue::Num(indicators::derived::round_number::compute_up(price)))
        }
        RoundNumberDown => {
            let price = klines.h1.last().map(|k| k.close)
                .ok_or_else(|| no_data(spec, None, klines.h1.len()))?;
            Ok(IndicatorValue::Num(indicators::derived::round_number::compute_down(price)))
        }

        // ── 有周期的指标 ──
        _ => {
            let tf = spec.timeframe().expect("无周期指标已在上方处理");
            let k = klines_for_tf(tf, klines);
            if k.is_empty() {
                return Err(no_data(spec, Some(tf), 0));
            }
            let last_idx = k.len().saturating_sub(1);

            match spec {
                // ── primitive ──
                CurrentPrice { .. } => {
                    // M15 当前价为空时回退到 H1
                    let v = if matches!(tf, Timeframe::M15) && k.is_empty() {
                        klines.h1.last().map(|k| k.close).expect("H1 validated non-empty by caller")
                    } else {
                        indicators::primitive::current_price::compute(k)?
                    };
                    Ok(IndicatorValue::Num(v))
                }
                ChangePct { period, .. } => {
                    Ok(IndicatorValue::Num(indicators::primitive::change_pct::compute(k, *period)?))
                }
                CandleBody { .. } => {
                    Ok(IndicatorValue::Num(indicators::primitive::candle_body::compute(k)?))
                }
                LastCompletedVolume { .. } => {
                    Ok(IndicatorValue::Num(indicators::primitive::last_volume::compute(k)?))
                }

                // ── atomic ──
                Ema { period, .. } => {
                    if last_idx < *period - 1 {
                        return Err(no_data(spec, Some(tf), k.len()));
                    }
                    Ok(IndicatorValue::Num(indicators::atomic::ema::ema_at(k, last_idx, *period)?))
                }
                Rsi { period, .. } => {
                    if last_idx < *period {
                        return Err(no_data(spec, Some(tf), k.len()));
                    }
                    Ok(IndicatorValue::Num(indicators::atomic::rsi::rsi_at(k, last_idx, *period)?))
                }
                Adx { period, .. } => {
                    if last_idx < *period * 2 {
                        return Err(no_data(spec, Some(tf), k.len()));
                    }
                    Ok(IndicatorValue::Num(indicators::atomic::adx::adx_at(k, last_idx, *period)?))
                }
                Atr { period, .. } => {
                    if last_idx < *period {
                        return Err(no_data(spec, Some(tf), k.len()));
                    }
                    Ok(IndicatorValue::Num(indicators::atomic::atr::atr_at(k, last_idx, *period)?))
                }

                BbandsUpper { period, stddev, .. } => {
                    if last_idx < *period - 1 {
                        return Err(no_data(spec, Some(tf), k.len()));
                    }
                    let (u, _, _) = indicators::atomic::bbands::bbands_at(k, last_idx, *period, *stddev as f64)?;
                    Ok(IndicatorValue::Num(u))
                }
                BbandsMiddle { period, stddev, .. } => {
                    if last_idx < *period - 1 {
                        return Err(no_data(spec, Some(tf), k.len()));
                    }
                    let (_, m, _) = indicators::atomic::bbands::bbands_at(k, last_idx, *period, *stddev as f64)?;
                    Ok(IndicatorValue::Num(m))
                }
                BbandsLower { period, stddev, .. } => {
                    if last_idx < *period - 1 {
                        return Err(no_data(spec, Some(tf), k.len()));
                    }
                    let (_, _, l) = indicators::atomic::bbands::bbands_at(k, last_idx, *period, *stddev as f64)?;
                    Ok(IndicatorValue::Num(l))
                }

                Macd { fast, slow, .. } => {
                    if last_idx < *slow - 1 {
                        return Err(no_data(spec, Some(tf), k.len()));
                    }
                    Ok(IndicatorValue::Num(indicators::atomic::macd::macd_at(k, last_idx, *fast, *slow)?))
                }
                MacdSignal { fast, slow, signal, .. } => {
                    if last_idx < *slow + *signal - 2 {
                        return Err(no_data(spec, Some(tf), k.len()));
                    }
                    Ok(IndicatorValue::Num(indicators::atomic::macd::macd_signal_at(k, last_idx, *fast, *slow, *signal)?))
                }

                Highest { period, .. } => {
                    if last_idx < *period - 1 {
                        return Err(no_data(spec, Some(tf), k.len()));
                    }
                    Ok(IndicatorValue::Num(indicators::atomic::highest::highest_at(k, last_idx, *period)?))
                }
                Lowest { period, .. } => {
                    if last_idx < *period - 1 {
                        return Err(no_data(spec, Some(tf), k.len()));
                    }
                    Ok(IndicatorValue::Num(indicators::atomic::lowest::lowest_at(k, last_idx, *period)?))
                }

                VolumeSma { period, .. } => {
                    let last_completed = k.len().saturating_sub(2);
                    if last_completed + 1 < *period {
                        return Err(no_data(spec, Some(tf), k.len()));
                    }
                    Ok(IndicatorValue::Num(indicators::atomic::volume_sma::volume_sma_at(k, last_completed, *period)?))
                }

                // ── derived ──
                MacdHistogram { fast, slow, signal, .. } => {
                    Ok(IndicatorValue::Num(indicators::derived::macd_histogram::compute(k, *fast, *slow, *signal)?))
                }
                BbandsWidth { period, stddev, .. } => {
                    if last_idx < *period - 1 {
                        return Err(no_data(spec, Some(tf), k.len()));
                    }
                    Ok(IndicatorValue::Num(indicators::derived::bbands_width::compute(k, last_idx, *period, *stddev)?))
                }
                BandwidthBarsAgo { period, stddev, bars_ago, .. } => {
                    Ok(IndicatorValue::Num(indicators::derived::bandwidth_bars::compute(k, *period, *stddev, *bars_ago)?))
                }
                BarsOutsideBand { period, stddev, .. } => {
                    Ok(IndicatorValue::Int(indicators::derived::bars_outside::compute(k, *period, *stddev)?))
                }
                AtrPct { period, .. } => {
                    Ok(IndicatorValue::Num(indicators::derived::atr_pct::compute(k, *period)?))
                }
                AtrSma { atr_period, sma_period, .. } => {
                    Ok(IndicatorValue::Num(indicators::derived::atr_sma::compute(k, *atr_period, *sma_period)?))
                }
                EmaCrossBarsAgo { fast, slow, .. } => {
                    Ok(IndicatorValue::Int(indicators::derived::ema_cross_bars::compute(k, *fast, *slow)?))
                }
                EmaGapPct { fast, slow, .. } => {
                    Ok(IndicatorValue::Num(indicators::derived::ema_gap_pct::compute(k, *fast, *slow)?))
                }
                EmaGapTrend { fast, slow, .. } => {
                    Ok(IndicatorValue::Str(indicators::derived::ema_gap_trend::compute(k, *fast, *slow)?))
                }
                EmaCrossState { fast, slow, .. } => {
                    Ok(IndicatorValue::Str(indicators::derived::ema_cross_state::compute(k, *fast, *slow)?))
                }

                // 无周期指标已在上方处理
                FundingRate | FundingNextTime | RoundNumberUp | RoundNumberDown => unreachable!(
                    "无周期指标应在 compute_one 顶部处理"
                ),
            }
        }
    }
}
