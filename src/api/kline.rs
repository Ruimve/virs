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
