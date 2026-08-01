use chrono::Utc;
use uuid::Uuid;
use virs_api::handlers::ai::{resolve_provider_base_url, resolve_provider_model};
use virs_api::handlers::ai_credentials::{parse_balance_response, parse_models_response};
use virs_api::handlers::response::ApiResponse;
use virs_api::ws::{kline_event_to_json, position_to_ws_json};
use virs_ccxt::ws_types::Candle;
use virs_error::{ApiError, VirsError};
use virs_market::types::{KlineEvent, KlineEventType, Timeframe};
use virs_types::{PositionSide, PositionStatus};
use virs_types::position::Position;


#[test]
fn int_1_1_provider_url_model_consistency() {

    for provider in &["deepseek", "openai", "openrouter"] {
        let url = resolve_provider_base_url(provider);
        let model = resolve_provider_model(provider);
        assert!(url.is_some(), "URL missing for provider: {}", provider);
        assert!(model.is_some(), "Model missing for provider: {}", provider);
    }
}

#[test]
fn int_1_2_known_provider_roundtrip() {

    let url = resolve_provider_base_url("deepseek").unwrap();
    let model = resolve_provider_model("deepseek").unwrap();
    assert!(url.starts_with("https://"));
    assert!(!model.is_empty());
}


#[test]
fn int_2_1_position_serde_then_ws_json() {
    let pos = make_position(PositionSide::Long);
    let json = serde_json::to_value(position_to_ws_json(&pos, None, None)).unwrap();
    assert_eq!(json["type"], "position_updated");
    assert_eq!(json["symbol"], "BTC/USDT");
    assert_eq!(json["side"], "long");


    let serialized = serde_json::to_string(&json).unwrap();
    assert!(serialized.contains("position_updated"));
}

#[test]
fn int_2_2_kline_event_to_json_chain() {
    let event = KlineEvent {
        exchange: "binance".into(),
        symbol: "ETH/USDT".into(),
        timeframe: Timeframe::H1,
        candle: Candle {
            open_time: 1700000000000,
            close_time: 1700003599999,
            open: 3000.0,
            high: 3050.0,
            low: 2980.0,
            close: 3020.0,
            volume: 500.0,
            quote_volume: 1510000.0,
            trades: 3000,
            closed: true,
        },
        event_type: KlineEventType::Closed,
    };
    let json = serde_json::to_value(kline_event_to_json(&event)).unwrap();
    assert_eq!(json["exchange"], "binance");
    assert_eq!(json["symbol"], "ETH/USDT");
    assert_eq!(json["timeframe"], "1h");
    assert_eq!(json["candle"]["close"], 3020.0);
    assert_eq!(json["candle"]["closed"], true);
    assert_eq!(json["event_type"], "Closed");
}


#[test]
fn int_3_1_api_response_ok_then_serialize() {
    let resp = ApiResponse::ok(serde_json::json!({"result": "success"}));
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["result"], "success");
    assert!(json["message"].is_null());
}

#[test]
fn int_3_2_api_error_from_virs_error_then_serialize() {
    let err = ApiError::from(VirsError::bad_request("operation failed"));
    let json = serde_json::to_value(&err).unwrap();
    assert_eq!(json["success"], false);
    assert_eq!(json["status"], 400);
    assert_eq!(json["message"], "operation failed");
}


#[test]
fn int_4_1_models_response_parse_chain() {

    let api_response = serde_json::json!({
        "object": "list",
        "data": [
            {"id": "deepseek-chat", "object": "model", "owned_by": "deepseek"},
            {"id": "deepseek-reasoner", "object": "model", "owned_by": "deepseek"}
        ]
    });
    let models = parse_models_response(&api_response);
    assert_eq!(models.len(), 2);
    assert_eq!(models[0]["id"], "deepseek-chat");
    assert_eq!(models[0]["owned_by"], "deepseek");
    assert_eq!(models[1]["id"], "deepseek-reasoner");


    for m in &models {
        let resp = ApiResponse::ok(m.clone());
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["success"], true);
    }
}

#[test]
fn int_4_2_balance_response_parse_chain() {

    let api_response = serde_json::json!({
        "is_available": true,
        "balance_infos": [
            {"total_balance": "10.50", "currency": "USD"}
        ]
    });
    let balances = parse_balance_response(&api_response);
    assert_eq!(balances.len(), 1);
    assert_eq!(balances[0]["total_balance"], "10.50");
    assert_eq!(balances[0]["currency"], "USD");


    let resp = ApiResponse::ok(serde_json::json!({"balances": balances}));
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["balances"][0]["total_balance"], "10.50");
}


fn make_position(side: PositionSide) -> Position {
    Position {
        id: Uuid::nil(),
        exchange: "binance".into(),
        symbol: "BTC/USDT".into(),
        side,
        status: PositionStatus::Open,
        quantity: 1.0,
        entry_price: 50000.0,
        realized_pnl: 0.0,
        client_order_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}
