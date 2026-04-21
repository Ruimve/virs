use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Query, State},
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

use super::AppState;
use crate::utils::auth::decode_jwt;

#[derive(Deserialize)]
pub struct WsAuthParams {
    pub token: Option<String>,
}

/// Server-sent event types for real-time updates.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum WsEvent {
    #[serde(rename = "strategy_status")]
    StrategyStatus {
        strategy_id: String,
        name: String,
        status: String,
    },
    #[serde(rename = "trade")]
    Trade {
        strategy_id: String,
        symbol: String,
        side: String,
        price: f64,
        amount: f64,
        pnl: f64,
    },
    #[serde(rename = "order")]
    Order {
        order_id: String,
        strategy_id: String,
        symbol: String,
        status: String,
        error: Option<String>,
    },
    #[serde(rename = "position")]
    Position {
        strategy_id: String,
        symbol: String,
        side: String,
        size: f64,
        entry_price: f64,
        action: String,
    },
    #[serde(rename = "risk")]
    Risk {
        strategy_id: String,
        symbol: String,
        reason: String,
        price: f64,
    },
    #[serde(rename = "notification")]
    Notification {
        level: String,
        message: String,
    },
}

pub type WsBroadcaster = broadcast::Sender<WsEvent>;

pub fn create_broadcaster(capacity: usize) -> WsBroadcaster {
    broadcast::channel(capacity).0
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsAuthParams>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let token = match params.token {
        Some(t) if !t.is_empty() => t,
        _ => {
            return Response::builder()
                .status(401)
                .body("Missing token".into())
                .unwrap();
        }
    };

    match std::env::var("SECRET_KEY") {
        Ok(secret) => match decode_jwt(&token, &secret) {
            Ok(_claims) => ws.on_upgrade(move |socket| handle_socket(socket, state.ws_broadcaster.clone())),
            Err(_) => Response::builder()
                .status(401)
                .body("Invalid token".into())
                .unwrap(),
        },
        Err(_) => Response::builder()
            .status(500)
            .body("Server configuration error".into())
            .unwrap(),
    }
}

async fn handle_socket(socket: WebSocket, broadcaster: Arc<WsBroadcaster>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = broadcaster.subscribe();

    let send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let json = match serde_json::to_string(&event) {
                Ok(j) => j,
                Err(e) => {
                    tracing::error!("Failed to serialize WS event: {}", e);
                    continue;
                }
            };
            if sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Close(_) => break,
                Message::Ping(_) => {}
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}
