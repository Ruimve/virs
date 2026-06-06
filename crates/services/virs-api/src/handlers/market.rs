//! Market data handlers.

use axum::{
    extract::{Query, State},
    Json,
};

use crate::handlers::auth::ApiResponse;
use crate::state::AppState;

#[derive(serde::Deserialize)]
pub struct SymbolQuery {
    pub exchange: Option<String>,
    pub symbol: Option<String>,
    pub market_type: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct KlineSubscribeRequest {
    pub exchange: String,
    pub symbol: String,
    pub market_type: Option<String>,
    pub timeframe: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct KlineDataQuery {
    pub exchange: String,
    pub symbol: String,
    pub timeframe: Option<String>,
}

pub async fn kline_subscribe(
    State(state): State<AppState>,
    Json(body): Json<KlineSubscribeRequest>,
) -> Json<ApiResponse> {
    let market_type = match body.market_type.as_deref() {
        Some("spot") => virs_models::MarketType::Spot,
        _ => virs_models::MarketType::Perpetual,
    };
    match state.kline_engine.subscribe(&body.exchange, &body.symbol, market_type).await {
        Ok(_) => Json(ApiResponse::ok(serde_json::json!({
            "subscribed": true,
            "exchange": body.exchange,
            "symbol": body.symbol,
        }))),
        Err(e) => Json(ApiResponse::err(format!("Subscribe failed: {}", e))),
    }
}

pub async fn kline_data(
    State(state): State<AppState>,
    Query(params): Query<KlineDataQuery>,
) -> Json<ApiResponse> {
    let tf = match params.timeframe.as_deref() {
        Some("1m") => virs_market::Timeframe::M1,
        Some("5m") => virs_market::Timeframe::M5,
        Some("15m") => virs_market::Timeframe::M15,
        Some("1h") => virs_market::Timeframe::H1,
        Some("4h") => virs_market::Timeframe::H4,
        Some("1d") => virs_market::Timeframe::D1,
        _ => virs_market::Timeframe::M1,
    };

    if let Some(candles) = state.kline_engine.get_klines_async(&params.exchange, &params.symbol, tf).await {
        if !candles.is_empty() {
            return Json(ApiResponse::ok(serde_json::json!({
                "SingleTimeframe": candles.iter().map(|c| serde_json::json!({
                    "open_time": c.open_time,
                    "open": c.open,
                    "high": c.high,
                    "low": c.low,
                    "close": c.close,
                    "volume": c.volume,
                })).collect::<Vec<_>>(),
            })));
        }
    }

    Json(ApiResponse::ok(serde_json::json!({
        "SingleTimeframe": [],
    })))
}

pub async fn get_ticker(
    State(state): State<AppState>,
    Query(params): Query<SymbolQuery>,
) -> Json<ApiResponse> {
    let exchange = match params.exchange {
        Some(ref e) => e,
        None => return Json(ApiResponse::err("exchange is required")),
    };
    let symbol = match params.symbol {
        Some(ref s) => s,
        None => return Json(ApiResponse::err("symbol is required")),
    };

    // Try kline engine first for latest price
    if let Some(candles) = state.kline_engine.get_klines_async(exchange, symbol, virs_market::Timeframe::M1).await {
        if let Some(last) = candles.last() {
            if last.close > 0.0 {
                return Json(ApiResponse::ok(serde_json::json!({
                    "symbol": symbol,
                    "exchange": exchange,
                    "last": last.close,
                    "high": last.high,
                    "low": last.low,
                    "volume": last.volume,
                    "open_time": last.open_time,
                })));
            }
        }
    }

    // Fallback to exchange ticker
    let exchange_key = format!("{}:perpetual", exchange);
    match state.exchange_registry.get(&exchange_key) {
        Some(ex) => match ex.get_ticker(symbol).await {
            Ok(ticker) => Json(ApiResponse::ok(serde_json::json!({
                "symbol": ticker.symbol,
                "exchange": exchange,
                "last": ticker.last,
                "bid": ticker.bid,
                "ask": ticker.ask,
                "high_24h": ticker.high_24h,
                "low_24h": ticker.low_24h,
                "volume_24h": ticker.volume_24h,
                "change_24h": ticker.price_change_24h,
                "change_pct_24h": ticker.price_change_pct_24h,
            }))),
            Err(e) => Json(ApiResponse::err(format!("Ticker error: {}", e))),
        },
        None => Json(ApiResponse::err(format!("Exchange '{}' not registered", exchange))),
    }
}

pub async fn get_klines(
    State(state): State<AppState>,
    Query(params): Query<SymbolQuery>,
) -> Json<ApiResponse> {
    let exchange = match params.exchange {
        Some(ref e) => e,
        None => return Json(ApiResponse::err("exchange is required")),
    };
    let symbol = match params.symbol {
        Some(ref s) => s,
        None => return Json(ApiResponse::err("symbol is required")),
    };

    // Auto-subscribe to kline engine for WS push
    let market_type = match params.market_type.as_deref() {
        Some("spot") => virs_models::MarketType::Spot,
        _ => virs_models::MarketType::Perpetual,
    };
    let _ = state.kline_engine.subscribe(exchange, symbol, market_type).await;

    // Try kline engine cache
    for tf in &[virs_market::Timeframe::M1, virs_market::Timeframe::M5, virs_market::Timeframe::M15,
                virs_market::Timeframe::H1, virs_market::Timeframe::H4, virs_market::Timeframe::D1] {
        if let Some(candles) = state.kline_engine.get_klines_async(exchange, symbol, *tf).await {
            if !candles.is_empty() {
                return Json(ApiResponse::ok(serde_json::json!({
                    "symbol": symbol,
                    "exchange": exchange,
                    "timeframe": tf.as_str(),
                    "candles": candles.iter().map(|c| serde_json::json!({
                        "open_time": c.open_time,
                        "open": c.open,
                        "high": c.high,
                        "low": c.low,
                        "close": c.close,
                        "volume": c.volume,
                    })).collect::<Vec<_>>(),
                })));
            }
        }
    }

    // Fallback to exchange REST API
    let exchange_key = format!("{}:perpetual", exchange);
    match state.exchange_registry.get(&exchange_key) {
        Some(ex) => match ex.get_klines(symbol, "1h", 100, None).await {
            Ok(klines) => Json(ApiResponse::ok(serde_json::json!({
                "symbol": symbol,
                "exchange": exchange,
                "timeframe": "1h",
                "candles": klines.iter().map(|k| serde_json::json!({
                    "open_time": k.open_time,
                    "open": k.open,
                    "high": k.high,
                    "low": k.low,
                    "close": k.close,
                    "volume": k.volume,
                })).collect::<Vec<_>>(),
            }))),
            Err(e) => Json(ApiResponse::err(format!("Klines error: {}", e))),
        },
        None => Json(ApiResponse::err(format!("Exchange '{}' not registered", exchange))),
    }
}

pub async fn get_order_book(
    State(state): State<AppState>,
    Query(params): Query<SymbolQuery>,
) -> Json<ApiResponse> {
    let exchange = match params.exchange {
        Some(ref e) => e,
        None => return Json(ApiResponse::err("exchange is required")),
    };
    let symbol = match params.symbol {
        Some(ref s) => s,
        None => return Json(ApiResponse::err("symbol is required")),
    };

    let exchange_key = format!("{}:perpetual", exchange);
    match state.exchange_registry.get(&exchange_key) {
        Some(ex) => match ex.get_order_book(symbol, 20).await {
            Ok(ob) => Json(ApiResponse::ok(serde_json::json!({
                "symbol": symbol,
                "exchange": exchange,
                "bids": ob.bids.iter().take(20).map(|(price, amount)| serde_json::json!([price, amount])).collect::<Vec<_>>(),
                "asks": ob.asks.iter().take(20).map(|(price, amount)| serde_json::json!([price, amount])).collect::<Vec<_>>(),
            }))),
            Err(e) => Json(ApiResponse::err(format!("OrderBook error: {}", e))),
        },
        None => Json(ApiResponse::err(format!("Exchange '{}' not registered", exchange))),
    }
}

pub async fn get_balances(
    State(state): State<AppState>,
    Query(params): Query<SymbolQuery>,
) -> Json<ApiResponse> {
    let exchange = match params.exchange {
        Some(ref e) => e,
        None => return Json(ApiResponse::err("exchange is required")),
    };

    let exchange_key = format!("{}:perpetual", exchange);
    match state.exchange_registry.get(&exchange_key) {
        Some(ex) => match ex.get_balances().await {
            Ok(balances) => {
                let filtered: Vec<_> = balances.into_iter()
                    .filter(|b| b.total > 0.0)
                    .map(|b| serde_json::json!({
                        "asset": b.asset,
                        "total": b.total,
                        "free": b.free,
                        "used": b.used,
                    }))
                    .collect();
                Json(ApiResponse::ok(serde_json::json!({ "balances": filtered })))
            }
            Err(e) => Json(ApiResponse::err(format!("Balances error: {}", e))),
        },
        None => Json(ApiResponse::err(format!("Exchange '{}' not registered", exchange))),
    }
}

pub async fn get_symbols(
    State(state): State<AppState>,
    Query(params): Query<SymbolQuery>,
) -> Json<ApiResponse> {
    let exchange = match params.exchange {
        Some(ref e) => e,
        None => return Json(ApiResponse::err("exchange is required")),
    };

    let exchange_key = format!("{}:perpetual", exchange);
    match state.exchange_registry.get(&exchange_key) {
        Some(ex) => match ex.get_symbols().await {
            Ok(symbols) => Json(ApiResponse::ok(serde_json::json!({
                "exchange": exchange,
                "symbols": symbols,
            }))),
            Err(e) => Json(ApiResponse::err(format!("Symbols error: {}", e))),
        },
        None => Json(ApiResponse::err(format!("Exchange '{}' not registered", exchange))),
    }
}
