//! WebSocket handlers for real-time data push.

use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use std::sync::Arc;

use crate::state::AppState;

/// Convert a Position to WebSocket JSON format.
/// Used by position_ws_handler for real-time position updates.
pub fn position_to_ws_json(pos: &virs_types::Position) -> serde_json::Value {
    serde_json::json!({
        "type": "position_updated",
        "symbol": pos.symbol,
        "exchange": pos.exchange,
        "side": format!("{:?}", pos.side).to_lowercase(),
        "status": format!("{:?}", pos.status).to_lowercase(),
        "size": pos.size,
        "entry_price": pos.entry_price,
        "current_price": pos.current_price,
        "leverage": pos.leverage,
        "margin": pos.margin,
        "unrealized_pnl": pos.unrealized_pnl,
        "realized_pnl": pos.realized_pnl,
        "stop_loss": pos.stop_loss,
        "take_profit": pos.take_profit,
        "liquidation_price": pos.liquidation_price,
        "position_id": pos.id.to_string(),
        "updated_at": pos.updated_at.to_rfc3339(),
    })
}

/// Convert a KlineEvent to WebSocket JSON format.
/// Used by kline_ws_handler for real-time kline updates.
pub fn kline_event_to_json(event: &virs_market::KlineEvent) -> serde_json::Value {
    serde_json::json!({
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
    })
}

/// Convert an OrderBookEvent to WebSocket JSON format.
/// Used by orderbook_ws_handler for real-time order book updates.
pub fn orderbook_event_to_json(event: &virs_market::OrderBookEvent) -> serde_json::Value {
    serde_json::json!({
        "exchange": event.exchange,
        "symbol": event.symbol,
        "bids": event.bids.iter().map(|l| serde_json::json!([l.price, l.amount])).collect::<Vec<_>>(),
        "asks": event.asks.iter().map(|l| serde_json::json!([l.price, l.amount])).collect::<Vec<_>>(),
        "timestamp": event.timestamp,
    })
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

                        let json = kline_event_to_json(&event);
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
                        let json = orderbook_event_to_json(&event);
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

/// WebSocket handler for real-time position updates from PositionEngine.
/// 客户端连接后发送 {"action":"subscribe","symbol":"BTCUSDT"} 订阅指定 symbol 的仓位更新。
pub async fn position_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_position_ws(socket, state))
}

async fn handle_position_ws(mut socket: WebSocket, state: AppState) {
    // 订阅 PE 事件
    let mut pe_rx = match state.engine_manager.pe_event_subscribe() {
        Some(rx) => rx,
        None => {
            let _ = socket
                .send(Message::Text(
                    serde_json::json!({"type":"error","message":"engines not started"})
                        .to_string()
                        .into(),
                ))
                .await;
            return;
        }
    };

    // 客户端订阅的 symbol 集合
    // select! 不会并发执行两个分支，所以直接用局部变量即可，无需 Mutex
    let mut subscribed_symbols: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    // 主循环：select! 同时处理 PE 事件和客户端消息
    loop {
        tokio::select! {
            msg = pe_rx.recv() => {
                match msg {
                    Ok(event) => {
                        if let virs_types::position::EngineEvent::PositionUpdated { position } = event {
                            // 检查 symbol 是否被订阅
                            if !subscribed_symbols.contains(&position.symbol) { continue; }

                            let json = position_to_ws_json(&position);
                            if let Ok(text) = serde_json::to_string(&json) {
                                if socket.send(Message::Text(text.into())).await.is_err() {
                                    break;
                                }
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
                        if let Ok(req) = serde_json::from_str::<serde_json::Value>(&text) {
                            if req.get("action").and_then(|v| v.as_str()) == Some("subscribe") {
                                if let Some(sym) = req.get("symbol").and_then(|v| v.as_str()) {
                                    subscribed_symbols.insert(sym.to_string());
                                    tracing::debug!("[position_ws] subscribed symbol: {}", sym);

                                    // 订阅时立即推送当前仓位快照，避免首次显示空仓
                                    let positions = state.engine_manager.get_positions_by_symbol(sym);
                                    for pos in positions {
                                        let json = position_to_ws_json(&pos);
                                        if let Ok(text) = serde_json::to_string(&json) {
                                            if socket.send(Message::Text(text.into())).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                }
                            } else if req.get("action").and_then(|v| v.as_str()) == Some("unsubscribe") {
                                if let Some(sym) = req.get("symbol").and_then(|v| v.as_str()) {
                                    subscribed_symbols.remove(sym);
                                    tracing::debug!("[position_ws] unsubscribed symbol: {}", sym);
                                }
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
