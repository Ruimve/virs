

use std::collections::HashSet;
use virs_error::VirsError;
use virs_type::{IndicatorSet, IndicatorSpec, IndicatorValue, Kline, Timeframe};

use crate::indicators;


#[derive(Debug, Clone, Copy)]
pub struct KlineSet<'a> {
    pub h1: &'a [Kline],
    pub h4: &'a [Kline],
    pub m15: &'a [Kline],
}


/* 批量计算指标：对规格列表去重后逐一计算，结果存入 IndicatorSet */
pub fn compute(
    specs: &[IndicatorSpec],
    klines: &KlineSet,
    funding_rate: f64,
    funding_next_time: &str,
) -> Result<IndicatorSet, VirsError> {
    /* 去重避免重复计算相同指标 */
    let unique: HashSet<&IndicatorSpec> = specs.iter().collect();
    let mut set = IndicatorSet::new();
    for spec in unique {
        let val = compute_one(spec, klines, funding_rate, funding_next_time)?;
        set.insert(spec.clone(), val);
    }
    Ok(set)
}


/* 根据时间周期从 KlineSet 中选取对应的 K 线切片 */
fn klines_for_tf<'a>(tf: Timeframe, klines: &KlineSet<'a>) -> &'a [Kline] {
    match tf {
        Timeframe::H1 => klines.h1,
        Timeframe::H4 => klines.h4,
        Timeframe::M15 => klines.m15,
        _ => &[],
    }
}


fn no_data(spec: &IndicatorSpec, tf: Option<Timeframe>, len: usize) -> VirsError {
    let tf_str = tf.map(|t| t.as_str()).unwrap_or("N/A");
    VirsError::config(format!(
        "Insufficient K-line data for indicator {:?} (tf={}, klines_len={}) — \
         cannot compute indicator with default value",
        spec, tf_str, len
    ))
}


/* 单个指标计算分发器：根据 IndicatorSpec 类型路由到对应的计算函数，并校验数据充分性 */
fn compute_one(
    spec: &IndicatorSpec,
    klines: &KlineSet,
    funding_rate: f64,
    funding_next_time: &str,
) -> Result<IndicatorValue, VirsError> {
    use IndicatorSpec::*;

    match spec {

        /* 无周期指标：资金费率和下次结算时间直接透传 */
        FundingRate => Ok(IndicatorValue::Num(funding_rate)),
        FundingNextTime => Ok(IndicatorValue::Str(funding_next_time.to_string())),


        /* 整数关口支撑/阻力位：基于 H1 最新收盘价计算最近的整数关口 */
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


        _ => {
            /* 有周期指标：根据时间周期选取对应 K 线数据 */
            let tf = spec.timeframe().expect("无周期指标已在上方处理");
            let k = klines_for_tf(tf, klines);
            if k.is_empty() {
                return Err(no_data(spec, Some(tf), 0));
            }
            let last_idx = k.len().saturating_sub(1);

            match spec {

                CurrentPrice { .. } => {
                    /* M15 无数据时退化使用 H1 收盘价，确保总能返回当前价格 */
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
                    /* 成交量 SMA 使用倒数第二根（已完成的 K 线），而非最后一根（可能未收盘） */
                    let last_completed = k.len().saturating_sub(2);
                    if last_completed + 1 < *period {
                        return Err(no_data(spec, Some(tf), k.len()));
                    }
                    Ok(IndicatorValue::Num(indicators::atomic::volume_sma::volume_sma_at(k, last_completed, *period)?))
                }


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


                FundingRate | FundingNextTime | RoundNumberUp | RoundNumberDown => unreachable!(
                    "无周期指标应在 compute_one 顶部处理"
                ),
            }
        }
    }
}
