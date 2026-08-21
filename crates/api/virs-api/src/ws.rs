use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use std::sync::Arc;

use virs_database::fetch_stop_loss_take_profit;

use crate::state::AppState;


#[derive(serde::Serialize)]
pub struct KlineWsMsg<'a> {
    exchange: &'a str,
    symbol: &'a str,
    timeframe: &'a str,
    candle: &'a virs_type::Candle,
    event_type: &'a str,
}

#[derive(serde::Serialize)]
pub struct PositionWsMsg<'a> {
    #[serde(rename = "type")]
    msg_type: &'static str,
    symbol: &'a str,
    exchange: &'a str,
    side: String,
    status: &'static str,
    quantity: f64,
    entry_price: f64,
    realized_pnl: f64,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    id: String,
    client_order_id: Option<&'a str>,
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


fn position_side_str(side: &virs_type::PositionSide) -> String {
    match side {
        virs_type::PositionSide::Long => "long".to_string(),
        virs_type::PositionSide::Short => "short".to_string(),
        virs_type::PositionSide::Unknown(_) => {
            unreachable!("validate ensures position_side is Long/Short")
        }
    }
}


fn position_side_db_str(side: &virs_type::PositionSide) -> String {
    match side {
        virs_type::PositionSide::Long => "LONG".to_string(),
        virs_type::PositionSide::Short => "SHORT".to_string(),
        virs_type::PositionSide::Unknown(_) => {
            unreachable!("validate ensures position_side is Long/Short")
        }
    }
}


fn position_status_str(status: &virs_type::PositionStatus) -> &'static str {
    match status {
        virs_type::PositionStatus::Opening => "opening",
        virs_type::PositionStatus::Open => "open",
        virs_type::PositionStatus::Closing => "closing",
        virs_type::PositionStatus::Closed => "closed",
    }
}


pub fn position_to_ws_json(
    pos: &virs_type::Position,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
) -> PositionWsMsg<'_> {
    PositionWsMsg {
        msg_type: "position_updated",
        symbol: &pos.symbol,
        exchange: &pos.exchange,
        side: position_side_str(&pos.side),
        status: position_status_str(&pos.status),
        quantity: pos.quantity,
        entry_price: pos.entry_price,
        realized_pnl: pos.realized_pnl,
        stop_loss,
        take_profit,
        id: pos.id.to_string(),
        client_order_id: pos.client_order_id.as_deref(),
        created_at: pos.created_at.to_rfc3339(),
        updated_at: pos.updated_at.to_rfc3339(),
    }
}


/* 从pe_bot_order_context表查询持仓的止损止盈价：
 * 关联pe_order_latest表，筛选order_role='open'且status='open'的最新记录。
 * 实现已迁移到virs-database::fetch_stop_loss_take_profit */
async fn fetch_sl_tp(
    db: &virs_database::PgPool,
    symbol: &str,
    exchange: &str,
    side: &virs_type::PositionSide,
) -> (Option<f64>, Option<f64>) {
    let side_str = position_side_db_str(side);
    fetch_stop_loss_take_profit(db, symbol, exchange, &side_str).await
}


pub fn kline_event_to_json(event: &virs_type::KlineEvent) -> KlineWsMsg<'_> {
    KlineWsMsg {
        exchange: &event.exchange,
        symbol: &event.symbol,
        timeframe: event.timeframe.as_str(),
        candle: &event.candle,
        event_type: match event.event_type {
            virs_type::KlineEventType::Update => "Update",
            virs_type::KlineEventType::Closed => "Closed",
            virs_type::KlineEventType::Backfilled => "Backfilled",
        },
    }
}


pub fn orderbook_event_to_json(event: &virs_type::OrderBookEvent) -> OrderBookWsMsg<'_> {
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

async fn handle_kline_ws(mut socket: WebSocket, kline_engine: Arc<dyn virs_type::KlineEngineHandle>) {
    let mut rx = kline_engine.subscribe_kline_events();


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
    orderbook_engine: Arc<dyn virs_type::OrderBookEngineHandle>,
) {
    let mut rx = orderbook_engine.subscribe_orderbook_events();


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


                        let position = match event {
                            virs_type::EngineEvent::PositionOpened { position } => Some(position),
                            virs_type::EngineEvent::PositionClosed { position } => Some(position),
                            virs_type::EngineEvent::PositionUpdated { position } => Some(position),
                            _ => None,
                        };

                        if let Some(position) = position {
                            if !subscribed_symbols.is_empty()
                                && !subscribed_symbols.contains(&position.symbol)
                            { continue; }

                            let (sl, tp) = fetch_sl_tp(
                                &state.db_pool,
                                &position.symbol,
                                &position.exchange,
                                &position.side,
                            ).await;
                            let msg = position_to_ws_json(&position, sl, tp);
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
                                        let (sl, tp) = fetch_sl_tp(
                                            &state.db_pool,
                                            &pos.symbol,
                                            &pos.exchange,
                                            &pos.side,
                                        ).await;
                                        let msg = position_to_ws_json(&pos, sl, tp);
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
