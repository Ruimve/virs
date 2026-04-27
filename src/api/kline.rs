use axum::{
    extract::{Query, State, WebSocketUpgrade, ws::WebSocket},
    response::Response,
    Json,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::kline::types::{AllTimeframesData, Candle, Timeframe, BacktestRangeInfo, MarketType};
use crate::api::AppState;

#[derive(Debug, Serialize)]
pub struct BacktestLimitsResponse {
    pub success: bool,
    pub data: Vec<BacktestRangeInfo>,
}

#[derive(Debug, Deserialize)]
pub struct KlinesQuery {
    pub exchange: String,
    pub symbol: String,
    pub timeframe: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct KlineResponse {
    pub success: bool,
    pub data: Option<KlineData>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum KlineData {
    SingleTimeframe(Vec<Candle>),
    AllTimeframes(AllTimeframesData),
}

pub async fn get_klines(
    State(state): State<Arc<AppState>>,
    Query(params): Query<KlinesQuery>,
) -> Json<KlineResponse> {
    let engine = match &state.kline_engine {
        Some(e) => e,
        None => {
            return Json(KlineResponse {
                success: false,
                data: None,
                error: Some("Kline engine not initialized".to_string()),
            });
        }
    };

    if let Some(tf_str) = &params.timeframe {
        let tf = match Timeframe::from_str_lossy(tf_str) {
            Some(t) => t,
            None => {
                return Json(KlineResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Invalid timeframe: {}", tf_str)),
                });
            }
        };

        match engine.get_klines_async(&params.exchange, &params.symbol, tf).await {
            Some(candles) => Json(KlineResponse {
                success: true,
                data: Some(KlineData::SingleTimeframe(candles)),
                error: None,
            }),
            None => Json(KlineResponse {
                success: false,
                data: None,
                error: Some(format!("Not subscribed to {}/{}", params.exchange, params.symbol)),
            }),
        }
    } else {
        match engine.get_all_timeframes(&params.exchange, &params.symbol).await {
            Some(data) => Json(KlineResponse {
                success: true,
                data: Some(KlineData::AllTimeframes(data)),
                error: None,
            }),
            None => Json(KlineResponse {
                success: false,
                data: None,
                error: Some(format!("Not subscribed to {}/{}", params.exchange, params.symbol)),
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub exchange: String,
    pub symbol: String,
    pub market_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SubscribeResponse {
    pub success: bool,
    pub message: Option<String>,
    pub error: Option<String>,
}

pub async fn subscribe_kline(
    State(state): State<Arc<AppState>>,
    Json(params): Json<SubscribeRequest>,
) -> Json<SubscribeResponse> {
    let engine = match &state.kline_engine {
        Some(e) => e,
        None => {
            return Json(SubscribeResponse {
                success: false,
                message: None,
                error: Some("Kline engine not initialized".to_string()),
            });
        }
    };

    let market_type = match params.market_type.as_deref() {
        Some("perpetual") => MarketType::Perpetual,
        _ => MarketType::Spot,
    };

    match engine.subscribe(&params.exchange, &params.symbol, market_type).await {
        Ok(()) => Json(SubscribeResponse {
            success: true,
            message: Some(format!("Subscribed to {}/{}", params.exchange, params.symbol)),
            error: None,
        }),
        Err(e) => Json(SubscribeResponse {
            success: false,
            message: None,
            error: Some(e.to_string()),
        }),
    }
}

#[derive(Debug, Deserialize)]
pub struct UnsubscribeRequest {
    pub exchange: String,
    pub symbol: String,
}

pub async fn unsubscribe_kline(
    State(state): State<Arc<AppState>>,
    Json(params): Json<UnsubscribeRequest>,
) -> Json<SubscribeResponse> {
    let engine = match &state.kline_engine {
        Some(e) => e,
        None => {
            return Json(SubscribeResponse {
                success: false,
                message: None,
                error: Some("Kline engine not initialized".to_string()),
            });
        }
    };

    match engine.unsubscribe(&params.exchange, &params.symbol).await {
        Ok(()) => Json(SubscribeResponse {
            success: true,
            message: Some(format!("Unsubscribed from {}/{}", params.exchange, params.symbol)),
            error: None,
        }),
        Err(e) => Json(SubscribeResponse {
            success: false,
            message: None,
            error: Some(e.to_string()),
        }),
    }
}

#[derive(Debug, Serialize)]
pub struct SubscriptionsResponse {
    pub success: bool,
    pub data: Vec<SubscriptionInfo>,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionInfo {
    pub exchange: String,
    pub symbol: String,
    pub market_type: String,
}

pub async fn list_subscriptions(
    State(state): State<Arc<AppState>>,
) -> Json<SubscriptionsResponse> {
    let engine = match &state.kline_engine {
        Some(e) => e,
        None => {
            return Json(SubscriptionsResponse {
                success: false,
                data: vec![],
            });
        }
    };

    let subs = engine.subscribed_symbols();
    Json(SubscriptionsResponse {
        success: true,
        data: subs
            .into_iter()
            .map(|(exchange, symbol, mt)| SubscriptionInfo {
                exchange,
                symbol,
                market_type: mt.to_string(),
            })
            .collect(),
    })
}

pub async fn kline_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_kline_ws(socket, state))
}

async fn handle_kline_ws(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    let engine = match &state.kline_engine {
        Some(e) => e,
        None => return,
    };

    let mut event_rx = engine.subscribe_events();

    let send_task = tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    let json = match serde_json::to_string(&event) {
                        Ok(j) => j,
                        Err(_) => continue,
                    };
                    if sender.send(axum::extract::ws::Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("[KlineWS] Client lagged, skipped {} events", n);
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::info!("[KlineWS] Broadcast channel closed");
                    break;
                }
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                axum::extract::ws::Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

pub async fn get_backtest_limits() -> Json<BacktestLimitsResponse> {
    Json(BacktestLimitsResponse {
        success: true,
        data: crate::kline::KlineEngine::backtest_range_limits(),
    })
}

#[derive(Debug, Deserialize)]
pub struct BacktestDataQuery {
    pub exchange: String,
    pub symbol: String,
    pub timeframe: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Serialize)]
pub struct BacktestDataResponse {
    pub success: bool,
    pub data: Option<Vec<Candle>>,
    pub error: Option<String>,
}

pub async fn get_backtest_data(
    State(state): State<Arc<AppState>>,
    Query(params): Query<BacktestDataQuery>,
) -> Json<BacktestDataResponse> {
    let engine = match &state.kline_engine {
        Some(e) => e,
        None => {
            return Json(BacktestDataResponse {
                success: false,
                data: None,
                error: Some("Kline engine not initialized".to_string()),
            });
        }
    };

    let tf = match Timeframe::from_str_lossy(&params.timeframe) {
        Some(t) => t,
        None => {
            return Json(BacktestDataResponse {
                success: false,
                data: None,
                error: Some(format!("Invalid timeframe: {}", params.timeframe)),
            });
        }
    };

    if params.start_ms >= params.end_ms {
        return Json(BacktestDataResponse {
            success: false,
            data: None,
            error: Some("start_ms must be less than end_ms".to_string()),
        });
    }

    match engine.fetch_backtest_data(&params.exchange, &params.symbol, tf, params.start_ms, params.end_ms).await {
        Ok(candles) => Json(BacktestDataResponse {
            success: true,
            data: Some(candles),
            error: None,
        }),
        Err(e) => Json(BacktestDataResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kline::types::{KlineEvent, KlineEventType, AllTimeframesData};
    use tokio::sync::broadcast;

    /// Helper: create a sample Candle for testing
    fn sample_candle() -> Candle {
        Candle {
            open_time: 1713900000000,
            close_time: 1713903599999,
            open: 65000.0,
            high: 65500.0,
            low: 64800.0,
            close: 65200.0,
            volume: 1234.5,
            quote_volume: 80_000_000.0,
            trades: 5000,
            closed: true,
        }
    }

    /// Helper: create a sample KlineEvent for testing
    fn sample_event(event_type: KlineEventType) -> KlineEvent {
        KlineEvent {
            exchange: "binance".to_string(),
            symbol: "BTCUSDT".to_string(),
            timeframe: Timeframe::H1,
            candle: sample_candle(),
            event_type,
        }
    }

    // ── Test 1: KlineEvent (Update) serialization ──────────────────────
    #[test]
    fn test_kline_event_serialization() {
        let event = sample_event(KlineEventType::Update);
        let json = serde_json::to_string(&event).expect("KlineEvent should serialize to JSON");
        assert!(
            json.contains("\"exchange\":\"binance\""),
            "JSON should contain exchange field: {json}"
        );
        assert!(
            json.contains("\"symbol\":\"BTCUSDT\""),
            "JSON should contain symbol field: {json}"
        );
    }

    // ── Test 2: KlineEvent (Closed) serialization ──────────────────────
    #[test]
    fn test_kline_event_closed_serialization() {
        let event = sample_event(KlineEventType::Closed);
        let json = serde_json::to_string(&event).expect("KlineEvent should serialize to JSON");
        assert!(
            json.contains("\"event_type\":\"Closed\""),
            "JSON should contain event_type Closed: {json}"
        );
    }

    // ── Test 3: KlineEvent (Backfilled) serialization ──────────────────
    #[test]
    fn test_kline_event_backfilled_serialization() {
        let event = sample_event(KlineEventType::Backfilled);
        let json = serde_json::to_string(&event).expect("KlineEvent should serialize to JSON");
        assert!(
            json.contains("\"event_type\":\"Backfilled\""),
            "JSON should contain event_type Backfilled: {json}"
        );
    }

    // ── Test 4: broadcast channel Lagged recovery ──────────────────────
    #[tokio::test]
    async fn test_broadcast_lagged_recovery() {
        let (tx, mut rx) = broadcast::channel::<KlineEvent>(2);

        // Send 5 messages into a channel with capacity 2 => receiver will lag
        for i in 0..5 {
            let mut ev = sample_event(KlineEventType::Update);
            ev.candle.open = ev.candle.open + i as f64;
            let _ = tx.send(ev);
        }

        // First recv should return Lagged because receiver fell behind
        let result = rx.recv().await;
        match &result {
            Err(broadcast::error::RecvError::Lagged(n)) => {
                assert!(*n > 0, "Lagged count should be positive");
            }
            other => panic!("Expected Lagged error, got: {other:?}"),
        }

        // After Lagged, next recv should succeed (not exit) — this mirrors the
        // handler logic: `Err(Lagged(_)) => continue`
        let result2 = rx.recv().await;
        assert!(result2.is_ok(), "Should receive an event after Lagged recovery");
    }

    // ── Test 5: broadcast channel Lagged then Closed ───────────────────
    #[tokio::test]
    async fn test_broadcast_lagged_then_closed() {
        let (tx, mut rx) = broadcast::channel::<KlineEvent>(2);

        // Overwhelm the receiver
        for i in 0..5 {
            let mut ev = sample_event(KlineEventType::Update);
            ev.candle.open = ev.candle.open + i as f64;
            let _ = tx.send(ev);
        }

        // Drop the sender so the channel closes
        drop(tx);

        // First recv: Lagged
        let result = rx.recv().await;
        assert!(
            matches!(result, Err(broadcast::error::RecvError::Lagged(_))),
            "Expected Lagged, got: {result:?}"
        );

        // Drain any remaining events (the channel still has buffered messages)
        // Keep receiving until we get Closed
        loop {
            match rx.recv().await {
                Ok(_) => continue,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => {
                    // This is the expected terminal state
                    break;
                }
            }
        }
    }

    // ── Test 6: broadcast channel multiple receivers ───────────────────
    #[tokio::test]
    async fn test_broadcast_multiple_receivers() {
        let (tx, mut rx1) = broadcast::channel::<KlineEvent>(16);
        let mut rx2 = tx.subscribe();
        let mut rx3 = tx.subscribe();

        let event = sample_event(KlineEventType::Update);
        tx.send(event).expect("send should succeed");

        let r1 = rx1.recv().await.expect("rx1 should receive");
        let r2 = rx2.recv().await.expect("rx2 should receive");
        let r3 = rx3.recv().await.expect("rx3 should receive");

        assert_eq!(r1.exchange, "binance");
        assert_eq!(r2.exchange, "binance");
        assert_eq!(r3.exchange, "binance");
    }

    // ── Test 7: broadcast receivers lag independently ──────────────────
    #[tokio::test]
    async fn test_broadcast_receiver_lagged_independently() {
        let (tx, mut rx1) = broadcast::channel::<KlineEvent>(2);
        let mut rx2 = tx.subscribe();

        // Send 5 messages into capacity-2 channel
        for i in 0..5 {
            let mut ev = sample_event(KlineEventType::Update);
            ev.candle.open = ev.candle.open + i as f64;
            let _ = tx.send(ev);
        }

        // rx1 has been consuming immediately, so it should get the latest message
        // (it may have gotten a Lagged first since we sent 5 into capacity 2
        //  without consuming — but we didn't consume on rx1 either!)
        // Actually both rx1 and rx2 haven't consumed, so both should be lagged.
        // Let's consume on rx1 first to drain it, then check rx2.

        // rx1: first recv may be Lagged, then we get an event
        let r1_first = rx1.recv().await;
        // Both receivers lagged equally since neither consumed
        assert!(
            matches!(r1_first, Err(broadcast::error::RecvError::Lagged(_))),
            "rx1 should be lagged too: {r1_first:?}"
        );

        // Now rx2 should also be lagged
        let r2_first = rx2.recv().await;
        assert!(
            matches!(r2_first, Err(broadcast::error::RecvError::Lagged(_))),
            "rx2 should be lagged: {r2_first:?}"
        );

        // After recovery, both should be able to receive
        let r1_ok = rx1.recv().await;
        assert!(r1_ok.is_ok(), "rx1 should receive after lagged recovery");

        let r2_ok = rx2.recv().await;
        assert!(r2_ok.is_ok(), "rx2 should receive after lagged recovery");
    }

    // ── Test 8: KlineResponse success serialization ────────────────────
    #[test]
    fn test_kline_response_serialization() {
        let response = KlineResponse {
            success: true,
            data: Some(KlineData::SingleTimeframe(vec![sample_candle()])),
            error: None,
        };
        let json = serde_json::to_string(&response).expect("KlineResponse should serialize");
        assert!(json.contains("\"success\":true"), "JSON should contain success:true: {json}");
        assert!(
            json.contains("\"open_time\":1713900000000"),
            "JSON should contain candle data: {json}"
        );
    }

    // ── Test 9: KlineResponse error serialization ──────────────────────
    #[test]
    fn test_kline_response_error() {
        let response = KlineResponse {
            success: false,
            data: None,
            error: Some("not subscribed".to_string()),
        };
        let json = serde_json::to_string(&response).expect("KlineResponse should serialize");
        assert!(json.contains("\"success\":false"), "JSON should contain success:false: {json}");
        assert!(
            json.contains("\"error\":\"not subscribed\""),
            "JSON should contain error message: {json}"
        );
    }

    // ── Test 10: AllTimeframesData serialization ───────────────────────
    #[test]
    fn test_all_timeframes_response() {
        let data = AllTimeframesData {
            m1: vec![sample_candle()],
            m5: vec![],
            m15: vec![],
            h1: vec![],
            h4: vec![],
            d1: vec![],
        };
        let response = KlineResponse {
            success: true,
            data: Some(KlineData::AllTimeframes(data)),
            error: None,
        };
        let json = serde_json::to_string(&response).expect("AllTimeframesData should serialize");
        assert!(json.contains("\"success\":true"), "JSON should contain success:true: {json}");
        assert!(
            json.contains("\"m1\":[") || json.contains("\"m1\": ["),
            "JSON should contain m1 array: {json}"
        );
        assert!(
            json.contains("\"m5\":[]") || json.contains("\"m5\": []"),
            "JSON should contain empty m5 array: {json}"
        );
    }
}
