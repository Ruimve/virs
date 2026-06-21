//! WebSocket handlers for real-time data push.

use axum::{
    extract::{State, WebSocketUpgrade, ws::{Message, WebSocket}},
    response::IntoResponse,
};
use std::sync::Arc;

use crate::state::{AppState, WsBroadcaster};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state.ws_broadcaster))
}

async fn handle_ws(mut socket: WebSocket, broadcaster: Arc<WsBroadcaster>) {
    let mut rx = broadcaster.subscribe();

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(value) => {
                        if let Ok(text) = serde_json::to_string(&value) {
                            if socket.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => continue,
                }
            }
        }
    }
}

/// WebSocket handler for real-time kline data from KlineEngine.
pub async fn kline_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_kline_ws(socket, state.kline_engine))
}

async fn handle_kline_ws(mut socket: WebSocket, kline_engine: Arc<virs_market::KlineEngine>) {
    let mut rx = kline_engine.subscribe_events();

    // 客户端可订阅指定 timeframe，未指定时推送全部（向后兼容）
    let mut timeframe_filter: Option<String> = None;

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(event) => {
                        // 按 timeframe 过滤
                        if let Some(ref tf) = timeframe_filter {
                            if format!("{}", event.timeframe) != *tf {
                                continue;
                            }
                        }

                        let json = serde_json::json!({
                            "exchange": event.exchange,
                            "symbol": event.symbol,
                            "timeframe": format!("{}", event.timeframe),
                            "candle": {
                                "open_time": event.candle.open_time,
                                "close_time": event.candle.close_time,
                                "open": event.candle.open,
                                "high": event.candle.high,
                                "low": event.candle.low,
                                "close": event.candle.close,
                                "volume": event.candle.volume,
                                "quote_volume": event.candle.quote_volume,
                                "trades": event.candle.trades,
                                "closed": event.candle.closed,
                            },
                            "event_type": match event.event_type {
                                virs_market::KlineEventType::Update => "Update",
                                virs_market::KlineEventType::Closed => "Closed",
                                virs_market::KlineEventType::Backfilled => "Backfilled",
                            },
                        });
                        if let Ok(text) = serde_json::to_string(&json) {
                            if socket.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // 解析客户端订阅消息：{"action":"subscribe","timeframe":"15m"}
                        if let Ok(req) = serde_json::from_str::<serde_json::Value>(&text) {
                            if req.get("action").and_then(|v| v.as_str()) == Some("subscribe") {
                                if let Some(tf) = req.get("timeframe").and_then(|v| v.as_str()) {
                                    timeframe_filter = Some(tf.to_string());
                                    tracing::debug!("[kline_ws] client subscribed to timeframe: {}", tf);
                                }
                            } else if req.get("action").and_then(|v| v.as_str()) == Some("unsubscribe") {
                                timeframe_filter = None;
                                tracing::debug!("[kline_ws] client unsubscribed timeframe filter");
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => continue,
                }
            }
        }
    }
}

/// WebSocket handler for real-time order book data from OrderBookEngine.
pub async fn orderbook_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_orderbook_ws(socket, state.orderbook_engine))
}

async fn handle_orderbook_ws(
    mut socket: WebSocket,
    orderbook_engine: Arc<virs_market::OrderBookEngine>,
) {
    let mut rx = orderbook_engine.subscribe_events();

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(event) => {
                        let json = serde_json::json!({
                            "exchange": event.exchange,
                            "symbol": event.symbol,
                            "bids": event.bids.iter().map(|l| serde_json::json!([l.price, l.amount])).collect::<Vec<_>>(),
                            "asks": event.asks.iter().map(|l| serde_json::json!([l.price, l.amount])).collect::<Vec<_>>(),
                            "timestamp": event.timestamp,
                        });
                        if let Ok(text) = serde_json::to_string(&json) {
                            if socket.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => continue,
                }
            }
        }
    }
}
