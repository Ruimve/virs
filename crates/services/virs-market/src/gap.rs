use std::sync::Arc;

use tokio::sync::Mutex;
use virs_error::{ExchangeError, VirsError, VirsResult};

use super::aggregator::Aggregator;
use super::cache::SymbolCache;
use super::types::{align_open_time, KlineSource};
use virs_type::{Candle, KlineEvent, KlineEventType, MarketType, Timeframe};

pub(crate) struct GapDetector;

const INITIAL_1M_LIMIT: u32 = 1000;
const INITIAL_HIGH_TF_LIMIT: u32 = 1000;

impl GapDetector {
    pub(crate) async fn detect_and_backfill(
        exchange: &str,
        symbol: &str,
        cache: &Arc<Mutex<SymbolCache>>,
        source: &Arc<dyn KlineSource>,
        event_tx: &tokio::sync::broadcast::Sender<KlineEvent>,
        market_type: MarketType,
    ) -> VirsResult<usize> {
        let last_closed_1m = {
            let guard = cache.lock().await;
            guard.last_closed_1m()
        };

        let now_ms = chrono::Utc::now().timestamp_millis();
        let expected_next = match last_closed_1m {
            Some(c) => c.open_time + 60_000,
            None => {
                return Self::initial_load(exchange, symbol, cache, source, event_tx, market_type)
                    .await;
            }
        };

        let current_1m_open = (now_ms / 60_000) * 60_000;
        if expected_next >= current_1m_open {
            return Ok(0);
        }

        let gap_start = expected_next;
        let gap_end = current_1m_open;
        let gap_minutes = ((gap_end - gap_start) / 60_000) as u32;

        let limit = gap_minutes.min(1000);
        let fetched = source
            .fetch_klines(
                exchange,
                symbol,
                "1m",
                limit,
                Some(gap_start),
                Some(market_type),
            )
            .await?;

        if fetched.is_empty() {
            tracing::warn!(
                exchange = %exchange,
                symbol = %symbol,
                "No data returned for gap backfill"
            );
            return Ok(0);
        }

        let mut backfilled_count = 0;
        let aggregated_data: Vec<(Timeframe, Vec<Candle>)> = {
            let mut guard = cache.lock().await;

            for candle in &fetched {
                if candle.open_time >= gap_start && candle.open_time < gap_end && candle.closed {
                    guard.update_candle(Timeframe::M1, candle.clone());
                    backfilled_count += 1;
                }
            }

            if backfilled_count > 0 {
                let all_1m = guard.get_klines(Timeframe::M1);
                [
                    Timeframe::M5,
                    Timeframe::M15,
                    Timeframe::H1,
                    Timeframe::H4,
                    Timeframe::D1,
                ]
                .iter()
                .filter_map(|&tf| {
                    let aggregated = Aggregator::aggregate_1m_to_timeframe(&all_1m, tf);
                    if aggregated.is_empty() {
                        None
                    } else {
                        Some((tf, aggregated))
                    }
                })
                .collect()
            } else {
                Vec::new()
            }
        };

        if !aggregated_data.is_empty() {
            let mut guard = cache.lock().await;
            for (tf, candles) in &aggregated_data {
                guard.replace_timeframe(*tf, candles.clone());
            }
        }

        if backfilled_count > 0
            && event_tx.receiver_count() > 0 {
                if let Some(last_candle) = fetched.last().cloned() {
                    if event_tx.send(KlineEvent {
                        exchange: exchange.to_string(),
                        symbol: symbol.to_string(),
                        timeframe: Timeframe::M1,
                        candle: last_candle,
                        event_type: KlineEventType::Backfilled,
                    }).is_err() {
                        tracing::debug!(
                            exchange = %exchange,
                            symbol = %symbol,
                            "KlineEvent (Backfilled) broadcast — receiver dropped between check and send"
                        );
                    }
                } else {
                    tracing::error!(
                        exchange = %exchange,
                        symbol = %symbol,
                        "backfilled_count > 0 but fetched is empty — skipping event broadcast (no candle to send)"
                    );
                }
            }

        Ok(backfilled_count)
    }

    async fn initial_load(
        exchange: &str,
        symbol: &str,
        cache: &Arc<Mutex<SymbolCache>>,
        source: &Arc<dyn KlineSource>,
        event_tx: &tokio::sync::broadcast::Sender<KlineEvent>,
        market_type: MarketType,
    ) -> VirsResult<usize> {
        let (result_1m, results_high): (
            VirsResult<Vec<Candle>>,
            Vec<(Timeframe, VirsResult<Vec<Candle>>)>,
        ) = {
            let fetch_1m = source.fetch_klines(
                exchange,
                symbol,
                "1m",
                INITIAL_1M_LIMIT,
                None,
                Some(market_type),
            );
            let fetch_m5 = source.fetch_klines(
                exchange,
                symbol,
                "5m",
                INITIAL_HIGH_TF_LIMIT,
                None,
                Some(market_type),
            );
            let fetch_m15 = source.fetch_klines(
                exchange,
                symbol,
                "15m",
                INITIAL_HIGH_TF_LIMIT,
                None,
                Some(market_type),
            );
            let fetch_h1 = source.fetch_klines(
                exchange,
                symbol,
                "1h",
                INITIAL_HIGH_TF_LIMIT,
                None,
                Some(market_type),
            );
            let fetch_h4 = source.fetch_klines(
                exchange,
                symbol,
                "4h",
                INITIAL_HIGH_TF_LIMIT,
                None,
                Some(market_type),
            );
            let fetch_d1 = source.fetch_klines(
                exchange,
                symbol,
                "1d",
                INITIAL_HIGH_TF_LIMIT,
                None,
                Some(market_type),
            );

            let (r_1m, r_m5, r_m15, r_h1, r_h4, r_d1) =
                tokio::join!(fetch_1m, fetch_m5, fetch_m15, fetch_h1, fetch_h4, fetch_d1);

            let high = vec![
                (Timeframe::M5, r_m5),
                (Timeframe::M15, r_m15),
                (Timeframe::H1, r_h1),
                (Timeframe::H4, r_h4),
                (Timeframe::D1, r_d1),
            ];
            (r_1m, high)
        };

        let candles_1m = match result_1m {
            Ok(c) if !c.is_empty() => c,
            Ok(_) => {
                return Err(VirsError::Exchange(ExchangeError::no_data(format!(
                    "No 1m candles for {}/{}",
                    exchange, symbol
                ))));
            }
            Err(e) => {
                return Err(e);
            }
        };

        let now_ms = chrono::Utc::now().timestamp_millis();
        let current_1m_open = (now_ms / 60_000) * 60_000;

        let unclosed_high: Vec<(Timeframe, Candle)> = [
            Timeframe::M5,
            Timeframe::M15,
            Timeframe::H1,
            Timeframe::H4,
            Timeframe::D1,
        ]
        .iter()
        .filter_map(|&tf| {
            let current_tf_open = align_open_time(current_1m_open, tf);
            let relevant: Vec<Candle> = candles_1m
                .iter()
                .filter(|c| c.open_time >= current_tf_open)
                .cloned()
                .collect();
            if relevant.is_empty() {
                return None;
            }
            let mut agg = Aggregator::aggregate_1m_to_timeframe(&relevant, tf);
            if let Some(last) = agg.last_mut() {
                last.closed = false;
            }
            agg.last().cloned().map(|c| (tf, c))
        })
        .collect();

        let mut total = 0;
        {
            let mut guard = cache.lock().await;

            guard.replace_timeframe(Timeframe::M1, candles_1m.clone());
            total += guard.get_klines(Timeframe::M1).len();

            for (tf, result) in &results_high {
                match result {
                    Ok(candles) if !candles.is_empty() => {
                        let mut final_candles = candles.clone();
                        if let Some(pos) = final_candles.iter().rposition(|c| !c.closed) {
                            final_candles.truncate(pos);
                        }

                        if let Some(unclosed) = unclosed_high.iter().find(|(t, _)| *t == *tf) {
                            if let Some(last_rest) = final_candles.last() {
                                if last_rest.open_time < unclosed.1.open_time {
                                    final_candles.push(unclosed.1.clone());
                                } else if last_rest.open_time == unclosed.1.open_time {
                                    let len = final_candles.len();
                                    final_candles[len - 1] = unclosed.1.clone();
                                }
                            } else {
                                final_candles.push(unclosed.1.clone());
                            }
                        }

                        guard.replace_timeframe(*tf, final_candles);
                        total += guard.get_klines(*tf).len();
                    }
                    Ok(_) => {
                        tracing::warn!(
                            exchange = %exchange,
                            symbol = %symbol,
                            timeframe = %tf.as_str(),
                            "No candles for timeframe"
                        );
                        if let Some(unclosed) = unclosed_high.iter().find(|(t, _)| *t == *tf) {
                            guard.replace_timeframe(*tf, vec![unclosed.1.clone()]);
                            total += 1;
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            exchange = %exchange,
                            symbol = %symbol,
                            timeframe = %tf.as_str(),
                            error = %e,
                            "Failed to load candles"
                        );
                        if let Some(unclosed) = unclosed_high.iter().find(|(t, _)| *t == *tf) {
                            guard.replace_timeframe(*tf, vec![unclosed.1.clone()]);
                            total += 1;
                        }
                    }
                }
            }
        }

        if total > 0
            && event_tx.receiver_count() > 0 {
                if let Some(last_candle) = candles_1m.last().cloned() {
                    if event_tx.send(KlineEvent {
                        exchange: exchange.to_string(),
                        symbol: symbol.to_string(),
                        timeframe: Timeframe::M1,
                        candle: last_candle,
                        event_type: KlineEventType::Backfilled,
                    }).is_err() {
                        tracing::debug!(
                            exchange = %exchange,
                            symbol = %symbol,
                            "KlineEvent (Backfilled) broadcast — receiver dropped between check and send"
                        );
                    }
                } else {
                    tracing::error!(
                        exchange = %exchange,
                        symbol = %symbol,
                        "total > 0 but candles_1m is empty — skipping event broadcast (no candle to send)"
                    );
                }
            }

        Ok(total)
    }

    pub(crate) async fn check_continuity(
        _exchange: &str,
        _symbol: &str,
        cache: &Arc<Mutex<SymbolCache>>,
    ) -> ContinuityReport {
        let guard = cache.lock().await;
        let last_closed = guard.last_closed_1m();

        let now_ms = chrono::Utc::now().timestamp_millis();
        let current_1m_open = (now_ms / 60_000) * 60_000;

        match last_closed {
            None => ContinuityReport {
                is_continuous: false,
                gap_start: None,
                gap_end: Some(current_1m_open),
                missing_minutes: u32::MAX,
            },
            Some(c) => {
                let expected_next = c.open_time + 60_000;
                if expected_next >= current_1m_open {
                    ContinuityReport {
                        is_continuous: true,
                        gap_start: None,
                        gap_end: None,
                        missing_minutes: 0,
                    }
                } else {
                    ContinuityReport {
                        is_continuous: false,
                        gap_start: Some(expected_next),
                        gap_end: Some(current_1m_open),
                        missing_minutes: ((current_1m_open - expected_next) / 60_000) as u32,
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct ContinuityReport {
    pub(crate) is_continuous: bool,
    pub(crate) gap_start: Option<i64>,
    pub(crate) gap_end: Option<i64>,
    pub(crate) missing_minutes: u32,
}
