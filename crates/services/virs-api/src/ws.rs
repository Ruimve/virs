use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use std::sync::Arc;

use crate::state::AppState;


#[derive(serde::Serialize)]
pub struct KlineWsMsg<'a> {
    exchange: &'a str,
    symbol: &'a str,
    timeframe: &'a str,
    candle: &'a virs_market::Candle,
    event_type: &'a str,
}

#[derive(serde::Serialize)]
pub struct PositionWsMsg<'a> {
    #[serde(rename = "type")]
    msg_type: &'static str,
    symbol: &'a str,
    exchange: &'a str,
    side: &'static str,
    status: &'static str,
    quantity: f64,
    entry_price: f64,
    realized_pnl: f64,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    position_id: String,
    created_at: String,
    updated_at: String,
}

#[derive(serde::Serialize)]
pub struct OrderBookWsMsg<'a> {
    exchange: &'a str,
    symbol: &'a str,


    bids: Vec<[f64; 2]>,
    asks: Vec<[f64; 2]>,
    timestamp: i64,
}


fn position_side_str(side: &virs_types::PositionSide) -> &'static str {
    match side {
        virs_types::PositionSide::Long => "long",
        virs_types::PositionSide::Short => "short",
    }
}


fn position_status_str(status: &virs_types::PositionStatus) -> &'static str {
    match status {
        virs_types::PositionStatus::Opening => "opening",
        virs_types::PositionStatus::Open => "open",
        virs_types::PositionStatus::Closing => "closing",
        virs_types::PositionStatus::Closed => "closed",
    }
}


pub fn position_to_ws_json(pos: &virs_types::Position) -> PositionWsMsg<'_> {
    PositionWsMsg {
        msg_type: "position_updated",
        symbol: &pos.symbol,
        exchange: &pos.exchange,
        side: position_side_str(&pos.side),
        status: position_status_str(&pos.status),
        quantity: pos.quantity,
        entry_price: pos.entry_price,
        realized_pnl: pos.realized_pnl,
        stop_loss: pos.stop_loss,
        take_profit: pos.take_profit,
        position_id: pos.id.to_string(),
        created_at: pos.created_at.to_rfc3339(),
        updated_at: pos.updated_at.to_rfc3339(),
    }
}


pub fn kline_event_to_json(event: &virs_market::KlineEvent) -> KlineWsMsg<'_> {
    KlineWsMsg {
        exchange: &event.exchange,
        symbol: &event.symbol,
        timeframe: event.timeframe.as_str(),
        candle: &event.candle,
        event_type: match event.event_type {
            virs_market::KlineEventType::Update => "Update",
            virs_market::KlineEventType::Closed => "Closed",
            virs_market::KlineEventType::Backfilled => "Backfilled",
        },
    }
}


pub fn orderbook_event_to_json(event: &virs_market::OrderBookEvent) -> OrderBookWsMsg<'_> {
    OrderBookWsMsg {
        exchange: &event.exchange,
        symbol: &event.symbol,
        bids: event.bids.iter().map(|l| [l.price, l.amount]).collect(),
        asks: event.asks.iter().map(|l| [l.price, l.amount]).collect(),
        timestamp: event.timestamp,
    }
}


pub async fn kline_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_kline_ws(socket, state.kline_engine))
}

async fn handle_kline_ws(mut socket: WebSocket, kline_engine: Arc<virs_market::KlineEngine>) {
    let mut rx = kline_engine.subscribe_events();


    let mut timeframe_filter: Option<String> = None;

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(event) => {

                        if let Some(ref tf) = timeframe_filter {
                            if event.timeframe.as_str() != tf.as_str() {
                                continue;
                            }
                        }

                        let msg = kline_event_to_json(&event);
                        if let Ok(text) = serde_json::to_string(&msg) {
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

                        if let Ok(req) = serde_json::from_str::<serde_json::Value>(&text) {
                            if req.get("action").and_then(|v| v.as_str()) == Some("subscribe") {
                                if let Some(tf) = req.get("timeframe").and_then(|v| v.as_str()) {
                                    timeframe_filter = Some(tf.to_string());
                                }
                            } else if req.get("action").and_then(|v| v.as_str()) == Some("unsubscribe") {
                                timeframe_filter = None;
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


    let mut subscribed_symbols: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(event) => {

                        if !subscribed_symbols.is_empty()
                            && !subscribed_symbols.contains(&event.symbol)
                        {
                            continue;
                        }

                        let msg = orderbook_event_to_json(&event);
                        if let Ok(text) = serde_json::to_string(&msg) {
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


                        if let Ok(req) = serde_json::from_str::<serde_json::Value>(&text) {
                            if req.get("action").and_then(|v| v.as_str()) == Some("subscribe") {
                                if let Some(sym) = req.get("symbol").and_then(|v| v.as_str()) {
                                    subscribed_symbols.insert(sym.to_string());
                                }
                            } else if req.get("action").and_then(|v| v.as_str()) == Some("unsubscribe") {
                                if let Some(sym) = req.get("symbol").and_then(|v| v.as_str()) {
                                    subscribed_symbols.remove(sym);
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


pub async fn position_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_position_ws(socket, state))
}

async fn handle_position_ws(mut socket: WebSocket, state: AppState) {

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


    let mut subscribed_symbols: std::collections::HashSet<String> =
        std::collections::HashSet::new();


    loop {
        tokio::select! {
            msg = pe_rx.recv() => {
                match msg {
                    Ok(event) => {
                        if let virs_types::position::EngineEvent::PositionUpdated { position } = event {


                            if !subscribed_symbols.is_empty()
                                && !subscribed_symbols.contains(&position.symbol)
                            { continue; }

                            let msg = position_to_ws_json(&position);
                            if let Ok(text) = serde_json::to_string(&msg) {
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


                                    let positions = state.engine_manager.get_positions_by_symbol(sym);
                                    for pos in positions {
                                        let msg = position_to_ws_json(&pos);
                                        if let Ok(text) = serde_json::to_string(&msg) {
                                            if socket.send(Message::Text(text.into())).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                }
                            } else if req.get("action").and_then(|v| v.as_str()) == Some("unsubscribe") {
                                if let Some(sym) = req.get("symbol").and_then(|v| v.as_str()) {
                                    subscribed_symbols.remove(sym);
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
