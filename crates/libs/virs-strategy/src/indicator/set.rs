//! 指标集合：批量计算 + 查询。
//!
//! [`IndicatorSet::compute`] 接收策略声明的 specs，去重后逐个调用
//! [`library`] 原子函数计算。计算行为与原 `compute_market_indicators` 完全一致，
//! 包括数据不足时的 `0.0` 兜底与 `warn!` 日志（保留既有行为）。
//!
//! 查询时 [`IndicatorSet::get`] 返回 `Option`，缺失返回 `None`，
//! 由调用方决定是报错还是降级 —— 不在指标库内隐式默认。

use std::collections::{HashMap, HashSet};
use virs_models::Kline;

use crate::indicator::library as lib;
use crate::indicator::spec::{IndicatorSpec, Timeframe};

/// 三周期 K 线输入。对应现有 `compute_market_indicators` 的三个参数。
#[derive(Debug, Clone, Copy)]
pub struct KlineSet<'a> {
    pub h1: &'a [Kline],
    pub h4: &'a [Kline],
    pub m15: &'a [Kline],
}

/// 指标值。不同指标返回不同类型（数值/整数/字符串）。
#[derive(Debug, Clone)]
pub enum IndicatorValue {
    Num(f64),
    Int(i32),
    Str(String),
}

/// 已计算的指标集合。通过 [`IndicatorSpec`] 查询。
#[derive(Debug, Clone, Default)]
pub struct IndicatorSet {
    values: HashMap<IndicatorSpec, IndicatorValue>,
}

impl IndicatorSet {
    /// 按策略声明的 specs 批量计算指标。自动去重。
    ///
    /// 注意：本函数保留与原 `compute_market_indicators` 一致的兜底行为
    /// （数据不足时返回 0.0 + warn!），不返回 `Result`。
    /// 消除 `unwrap_or(0.0)` 属于后续独立任务。
    pub fn compute(
        specs: &[IndicatorSpec],
        klines: &KlineSet,
        funding_rate: f64,
        funding_next_time: &str,
    ) -> Self {
        let unique: HashSet<&IndicatorSpec> = specs.iter().collect();
        let mut values = HashMap::with_capacity(unique.len());
        for spec in unique {
            let val = compute_one(spec, klines, funding_rate, funding_next_time);
            values.insert(spec.clone(), val);
        }
        Self { values }
    }

    /// 查询指标值。缺失返回 `None`（不隐式默认）。
    pub fn get(&self, spec: &IndicatorSpec) -> Option<&IndicatorValue> {
        self.values.get(spec)
    }

    /// 便捷查询数值型指标。缺失或类型不符返回 `None`。
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

    /// 已计算的指标数量。
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

fn klines_for_tf<'a>(tf: Timeframe, klines: &KlineSet<'a>) -> &'a [Kline] {
    match tf {
        Timeframe::H1 => klines.h1,
        Timeframe::H4 => klines.h4,
        Timeframe::M15 => klines.m15,
    }
}

/// 计算单个指标。逻辑与原 `compute_market_indicators` 对应分支逐一对应。
fn compute_one(
    spec: &IndicatorSpec,
    klines: &KlineSet,
    funding_rate: f64,
    funding_next_time: &str,
) -> IndicatorValue {
    use IndicatorSpec::*;
    match spec {
        // ── 资金费率（无周期）──
        FundingRate => IndicatorValue::Num(funding_rate),
        FundingNextTime => IndicatorValue::Str(funding_next_time.to_string()),

        // ── 整数关口（基于 H1 当前价）──
        RoundNumberUp => {
            let price = klines.h1.last().map(|k| k.close).unwrap_or_else(|| {
                tracing::warn!(
                    indicator = "round_number_up",
                    "H1 klines empty — defaulting round number to 0.0"
                );
                0.0
            });
            IndicatorValue::Num(lib::find_round_number(price, true))
        }
        RoundNumberDown => {
            let price = klines.h1.last().map(|k| k.close).unwrap_or_else(|| {
                tracing::warn!(
                    indicator = "round_number_down",
                    "H1 klines empty — defaulting round number to 0.0"
                );
                0.0
            });
            IndicatorValue::Num(lib::find_round_number(price, false))
        }

        // ── 有周期的指标：先取对应周期的 klines 与 last_idx ──
        _ => {
            let tf = spec.timeframe().expect("无周期指标已在上方处理");
            let k = klines_for_tf(tf, klines);
            let last_idx = k.len().saturating_sub(1);

            match spec {
                CurrentPrice { .. } => {
                    // M15 当前价在 klines 为空时回退到 H1 当前价（保留原行为）
                    let v = if matches!(tf, Timeframe::M15) {
                        k.last().map(|k| k.close).unwrap_or_else(|| {
                            klines.h1.last().map(|k| k.close).unwrap_or(0.0)
                        })
                    } else {
                        k.last().map(|k| k.close).unwrap_or_else(|| {
                            tracing::warn!(
                                indicator = "current_price",
                                tf = tf.as_str(),
                                len = k.len(),
                                "Insufficient data for indicator calculation — defaulting to 0.0"
                            );
                            0.0
                        })
                    };
                    IndicatorValue::Num(v)
                }
                ChangePct { period, .. } => {
                    let curr = k.last().map(|k| k.close).unwrap_or(0.0);
                    let v = if last_idx >= *period && *period > 0 {
                        let prev = k.get(last_idx - period).map(|k| k.close).unwrap_or(0.0);
                        if prev > 0.0 {
                            (curr - prev) / prev * 100.0
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    };
                    IndicatorValue::Num(v)
                }
                CandleBody { .. } => {
                    let v = k.last().map(|k| k.close - k.open).unwrap_or_else(|| {
                        tracing::warn!(
                            indicator = "candle_body",
                            tf = tf.as_str(),
                            len = k.len(),
                            "Insufficient data for indicator calculation — defaulting to 0.0"
                        );
                        0.0
                    });
                    IndicatorValue::Num(v)
                }
                LastCompletedVolume { .. } => {
                    let last_completed = k.len().saturating_sub(2);
                    let v = k.get(last_completed).map(|k| k.volume).unwrap_or_else(|| {
                        tracing::warn!(
                            indicator = "last_completed_volume",
                            tf = tf.as_str(),
                            idx = last_completed,
                            "Insufficient data for indicator calculation — defaulting to 0.0"
                        );
                        0.0
                    });
                    IndicatorValue::Num(v)
                }

                Ema { period, .. } => IndicatorValue::Num(lib::ema_at(k, last_idx, *period)),
                Rsi { period, .. } => IndicatorValue::Num(lib::rsi_at(k, last_idx, *period)),
                Adx { period, .. } => IndicatorValue::Num(lib::adx_at(k, last_idx, *period)),
                Atr { period, .. } => IndicatorValue::Num(lib::atr_at(k, last_idx, *period)),

                AtrPct { period, .. } => {
                    let atr_val = lib::atr_at(k, last_idx, *period);
                    let price = k.last().map(|k| k.close).unwrap_or(0.0);
                    let v = if price > 0.0 {
                        atr_val / price * 100.0
                    } else {
                        0.0
                    };
                    IndicatorValue::Num(v)
                }
                AtrSma { atr_period, sma_period, .. } => {
                    let v = if k.len() >= *sma_period {
                        let atr_series = lib::atr(k, *atr_period);
                        lib::sma_at_from(&atr_series, last_idx, *sma_period)
                    } else {
                        0.0
                    };
                    IndicatorValue::Num(v)
                }

                BbandsUpper { period, stddev, .. } => {
                    let (u, _, _) = lib::bbands_at(k, last_idx, *period, *stddev as f64);
                    IndicatorValue::Num(u)
                }
                BbandsMiddle { period, stddev, .. } => {
                    let (_, m, _) = lib::bbands_at(k, last_idx, *period, *stddev as f64);
                    IndicatorValue::Num(m)
                }
                BbandsLower { period, stddev, .. } => {
                    let (_, _, l) = lib::bbands_at(k, last_idx, *period, *stddev as f64);
                    IndicatorValue::Num(l)
                }
                BbandsWidth { period, stddev, .. } => {
                    IndicatorValue::Num(lib::bbands_width_at(k, last_idx, *period, *stddev as f64))
                }
                BandwidthBarsAgo { period, stddev, bars_ago, .. } => {
                    let v = if last_idx >= *bars_ago {
                        lib::bbands_width_at(k, last_idx - bars_ago, *period, *stddev as f64)
                    } else {
                        0.0
                    };
                    IndicatorValue::Num(v)
                }

                Macd { fast, slow, .. } => {
                    IndicatorValue::Num(lib::macd_at(k, last_idx, *fast, *slow))
                }
                MacdSignal { fast, slow, signal, .. } => {
                    IndicatorValue::Num(lib::macd_signal_at(k, last_idx, *fast, *slow, *signal))
                }
                MacdHistogram { fast, slow, signal, .. } => IndicatorValue::Num(
                    lib::macd_histogram_at(k, last_idx, *fast, *slow, *signal),
                ),

                Highest { period, .. } => {
                    IndicatorValue::Num(lib::highest_at(k, last_idx, *period))
                }
                Lowest { period, .. } => {
                    IndicatorValue::Num(lib::lowest_at(k, last_idx, *period))
                }

                VolumeSma { period, .. } => {
                    let last_completed = k.len().saturating_sub(2);
                    let v = if last_completed + 1 >= *period {
                        lib::volume_sma_at(k, last_completed, *period)
                    } else {
                        0.0
                    };
                    IndicatorValue::Num(v)
                }

                BarsOutsideBand { period, stddev, .. } => {
                    let (upper, _, lower) = lib::bbands_at(k, last_idx, *period, *stddev as f64);
                    IndicatorValue::Int(lib::compute_bars_outside_band(k, upper, lower))
                }

                EmaCrossBarsAgo { fast, slow, .. } => {
                    IndicatorValue::Int(lib::compute_ema_cross_bars_ago(k, *fast, *slow, last_idx))
                }

                EmaGapPct { fast, slow, .. } => {
                    let ema_fast = lib::ema_at(k, last_idx, *fast);
                    let ema_slow = lib::ema_at(k, last_idx, *slow);
                    let v = if ema_slow != 0.0 {
                        (ema_fast - ema_slow) / ema_slow * 100.0
                    } else {
                        0.0
                    };
                    IndicatorValue::Num(v)
                }
                EmaGapTrend { fast, slow, .. } => {
                    let ema_fast = lib::ema_at(k, last_idx, *fast);
                    let ema_slow = lib::ema_at(k, last_idx, *slow);
                    let lookback = 5.min(last_idx);
                    let ema_fast_prev = lib::ema_at(k, last_idx - lookback, *fast);
                    let ema_slow_prev =
                        if k.len() >= *slow + lookback {
                            lib::ema_at(k, last_idx - lookback, *slow)
                        } else {
                            ema_slow
                        };
                    let curr_gap_abs = (ema_fast - ema_slow).abs();
                    let prev_gap_abs = (ema_fast_prev - ema_slow_prev).abs();
                    let trend = if curr_gap_abs > prev_gap_abs * 1.01 {
                        "扩大"
                    } else if curr_gap_abs < prev_gap_abs * 0.99 {
                        "缩小"
                    } else {
                        "持平"
                    };
                    IndicatorValue::Str(trend.to_string())
                }

                // 无周期指标已在上方处理，这里不会到达
                FundingRate | FundingNextTime | RoundNumberUp | RoundNumberDown => unreachable!(
                    "无周期指标应在 compute_one 顶部处理"
                ),
            }
        }
    }
}

/// 返回与原 `compute_market_indicators` 完全对应的全部指标 specs。
///
/// 用于过渡期：`compute_market_indicators` 内部调用本函数 + `IndicatorSet::compute`
/// + `MarketIndicators::from_indicator_set`，实现行为等价的重构。
pub fn all_market_indicators_specs() -> Vec<IndicatorSpec> {
    use IndicatorSpec::*;
    use Timeframe::*;
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
