use std::sync::Arc;

use tokio::sync::Mutex;
use tracing;

use super::cache::SymbolCache;
use super::types::{Candle, Timeframe, KlineEvent, KlineEventType};
use super::aggregator::Aggregator;
use super::KlineSource;

pub struct GapDetector;

impl GapDetector {
    pub async fn detect_and_backfill(
        exchange: &str,
        symbol: &str,
        cache: &Arc<Mutex<SymbolCache>>,
        source: &Arc<dyn KlineSource>,
        event_tx: &tokio::sync::broadcast::Sender<KlineEvent>,
    ) -> anyhow::Result<usize> {
        let last_closed_1m = {
            let guard = cache.lock().await;
            guard.last_closed_1m()
        };

        let now_ms = chrono::Utc::now().timestamp_millis();
        let expected_next = match last_closed_1m {
            Some(c) => c.open_time + 60_000,
            None => {
                tracing::info!("[GapDetector] No 1m candles in cache for {}/{}, loading initial data", exchange, symbol);
                return Self::initial_load(exchange, symbol, cache, source, event_tx).await;
            }
        };

        let current_1m_open = (now_ms / 60_000) * 60_000;
        if expected_next >= current_1m_open {
            tracing::debug!("[GapDetector] No gap for {}/{}", exchange, symbol);
            return Ok(0);
        }

        let gap_start = expected_next;
        let gap_end = current_1m_open;
        let gap_minutes = ((gap_end - gap_start) / 60_000) as u32;

        tracing::info!(
            "[GapDetector] Gap detected for {}/{}: {} minutes ({} to {})",
            exchange, symbol, gap_minutes, gap_start, gap_end
        );

        let limit = gap_minutes.min(1000);
        let fetched = source.fetch_klines(exchange, symbol, "1m", limit, Some(gap_start)).await?;

        if fetched.is_empty() {
            tracing::warn!("[GapDetector] No data returned for gap backfill: {}/{}", exchange, symbol);
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
                [Timeframe::M5, Timeframe::M15, Timeframe::H1, Timeframe::H4, Timeframe::D1]
                    .iter()
                    .filter_map(|&tf| {
                        let aggregated = Aggregator::aggregate_1m_to_timeframe(&all_1m, tf);
                        if aggregated.is_empty() { None } else { Some((tf, aggregated)) }
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

        if backfilled_count > 0 {
            let _ = event_tx.send(KlineEvent {
                exchange: exchange.to_string(),
                symbol: symbol.to_string(),
                timeframe: Timeframe::M1,
                candle: fetched.last().cloned().unwrap_or_else(|| Candle {
                    open_time: 0, close_time: 0, open: 0.0, high: 0.0, low: 0.0,
                    close: 0.0, volume: 0.0, quote_volume: 0.0, trades: 0, closed: false,
                }),
                event_type: KlineEventType::Backfilled,
            });
        }

        tracing::info!("[GapDetector] Backfilled {} candles for {}/{}", backfilled_count, exchange, symbol);
        Ok(backfilled_count)
    }

    async fn initial_load(
        exchange: &str,
        symbol: &str,
        cache: &Arc<Mutex<SymbolCache>>,
        source: &Arc<dyn KlineSource>,
        event_tx: &tokio::sync::broadcast::Sender<KlineEvent>,
    ) -> anyhow::Result<usize> {
        let fetches: Vec<_> = Timeframe::all().iter().map(|&tf| {
            let limit = tf.default_limit() as u32;
            async move {
                let result = source.fetch_klines(exchange, symbol, tf.as_str(), limit, None).await;
                (tf, result)
            }
        }).collect();

        let results = futures_util::future::join_all(fetches).await;

        let mut fetched_data: Vec<(Timeframe, Vec<Candle>)> = Vec::new();
        for (tf, result) in results {
            match result {
                Ok(candles) if !candles.is_empty() => {
                    let count = candles.len();
                    tracing::info!(
                        "[GapDetector] Loaded {} {} candles for {}/{}",
                        count, tf.as_str(), exchange, symbol
                    );
                    fetched_data.push((tf, candles));
                }
                Ok(_) => {
                    tracing::warn!("[GapDetector] No {} candles for {}/{}", tf.as_str(), exchange, symbol);
                }
                Err(e) => {
                    tracing::error!("[GapDetector] Failed to load {} candles for {}/{}: {}", tf.as_str(), exchange, symbol, e);
                }
            }
        }

        let mut total = 0;
        {
            let mut guard = cache.lock().await;
            for (tf, candles) in &fetched_data {
                total += candles.len();
                guard.replace_timeframe(*tf, candles.clone());
            }
        }

        if total > 0 {
            let _ = event_tx.send(KlineEvent {
                exchange: exchange.to_string(),
                symbol: symbol.to_string(),
                timeframe: Timeframe::M1,
                candle: {
                    let guard = cache.lock().await;
                    guard.last_1m().unwrap_or_else(|| Candle {
                        open_time: 0, close_time: 0, open: 0.0, high: 0.0, low: 0.0,
                        close: 0.0, volume: 0.0, quote_volume: 0.0, trades: 0, closed: false,
                    })
                },
                event_type: KlineEventType::Backfilled,
            });
        }

        Ok(total)
    }

    pub async fn check_continuity(
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
pub struct ContinuityReport {
    pub is_continuous: bool,
    #[allow(dead_code)]
    pub gap_start: Option<i64>,
    #[allow(dead_code)]
    pub gap_end: Option<i64>,
    pub missing_minutes: u32,
}
