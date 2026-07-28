use axum::{
    extract::{Query, State},
    Json,
};
use virs_error::VirsError;

use crate::handlers::response::ApiResponse;
use crate::state::AppState;

#[derive(serde::Deserialize)]
pub struct SymbolQuery {
    pub exchange: Option<String>,
    pub symbol: Option<String>,
    pub timeframe: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct KlineSubscribeRequest {
    pub exchange: String,
    pub symbol: String,
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
) -> Result<Json<ApiResponse>, VirsError> {
    let market_type = virs_types::MarketType::Perpetual;


    let exchange_key = format!("{}:{}", body.exchange, market_type);
    if state.exchange_registry.get(&exchange_key).is_none() {
        return Err(VirsError::bad_request(format!(
            "Exchange '{}' not registered. Please create a bot first.",
            exchange_key
        )));
    }

    match state
        .kline_engine
        .subscribe(&body.exchange, &body.symbol, market_type)
        .await
    {
        Ok(_) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "subscribed": true,
            "exchange": body.exchange,
            "symbol": body.symbol,
        })))),
        Err(e) => Err(VirsError::bad_request(format!("Subscribe failed: {}", e))),
    }
}

pub async fn orderbook_subscribe(
    State(state): State<AppState>,
    Json(body): Json<KlineSubscribeRequest>,
) -> Result<Json<ApiResponse>, VirsError> {
    let market_type = virs_types::MarketType::Perpetual;

    match state
        .orderbook_engine
        .subscribe(&body.exchange, &body.symbol, market_type)
        .await
    {
        Ok(_) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "subscribed": true,
            "exchange": body.exchange,
            "symbol": body.symbol,
        })))),
        Err(e) => Err(VirsError::bad_request(format!(
            "OrderBook subscribe failed: {}",
            e
        ))),
    }
}

pub async fn kline_data(
    State(state): State<AppState>,
    Query(params): Query<KlineDataQuery>,
) -> Result<Json<ApiResponse>, VirsError> {
    let tf = match params.timeframe.as_deref() {
        Some("1m") => virs_market::Timeframe::M1,
        Some("5m") => virs_market::Timeframe::M5,
        Some("15m") => virs_market::Timeframe::M15,
        Some("1h") => virs_market::Timeframe::H1,
        Some("4h") => virs_market::Timeframe::H4,
        Some("1d") => virs_market::Timeframe::D1,
        _ => virs_market::Timeframe::M1,
    };

    if let Some(candles) = state
        .kline_engine
        .get_klines_async(&params.exchange, &params.symbol, tf)
        .await
    {
        if !candles.is_empty() {
            return Ok(Json(ApiResponse::ok(serde_json::json!({
                "SingleTimeframe": candles.iter().map(|c| serde_json::json!({
                    "open_time": c.open_time,
                    "open": c.open,
                    "high": c.high,
                    "low": c.low,
                    "close": c.close,
                    "volume": c.volume,
                })).collect::<Vec<_>>(),
            }))));
        }
    }

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "SingleTimeframe": [],
    }))))
}

pub async fn get_ticker(
    State(state): State<AppState>,
    Query(params): Query<SymbolQuery>,
) -> Result<Json<ApiResponse>, VirsError> {
    let exchange = match params.exchange {
        Some(ref e) => e,
        None => return Err(VirsError::bad_request("exchange is required")),
    };
    let symbol = match params.symbol {
        Some(ref s) => s,
        None => return Err(VirsError::bad_request("symbol is required")),
    };


    if let Some(candles) = state
        .kline_engine
        .get_klines_async(exchange, symbol, virs_market::Timeframe::M1)
        .await
    {
        if let Some(last) = candles.last() {
            if last.close > 0.0 {
                return Ok(Json(ApiResponse::ok(serde_json::json!({
                    "symbol": symbol,
                    "exchange": exchange,
                    "last": last.close,
                    "high": last.high,
                    "low": last.low,
                    "volume": last.volume,
                    "open_time": last.open_time,
                }))));
            }
        }
    }


    let exchange_key = format!("{}:{}", exchange, virs_types::MarketType::Perpetual);
    match state.exchange_registry.get(&exchange_key) {
        Some(ex) => match ex.get_ticker(symbol).await {
            Ok(ticker) => Ok(Json(ApiResponse::ok(serde_json::json!({
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
            })))),
            Err(e) => Err(VirsError::bad_request(format!("Ticker error: {}", e))),
        },
        None => Err(VirsError::bad_request(format!(
            "Exchange '{}' not registered",
            exchange
        ))),
    }
}

pub async fn get_klines(
    State(state): State<AppState>,
    Query(params): Query<SymbolQuery>,
) -> Result<Json<ApiResponse>, VirsError> {
    let exchange = match params.exchange {
        Some(ref e) => e,
        None => return Err(VirsError::bad_request("exchange is required")),
    };
    let symbol = match params.symbol {
        Some(ref s) => s,
        None => return Err(VirsError::bad_request("symbol is required")),
    };


    let requested_tf = match params.timeframe.as_deref() {
        Some("1m") => virs_market::Timeframe::M1,
        Some("5m") => virs_market::Timeframe::M5,
        Some("15m") => virs_market::Timeframe::M15,
        Some("1h") => virs_market::Timeframe::H1,
        Some("4h") => virs_market::Timeframe::H4,
        Some("1d") => virs_market::Timeframe::D1,
        _ => virs_market::Timeframe::M15,
    };


    if let Some(candles) = state
        .kline_engine
        .get_klines_async(exchange, symbol, requested_tf)
        .await
    {
        if !candles.is_empty() {
            return Ok(Json(ApiResponse::ok(serde_json::json!({
                "symbol": symbol,
                "exchange": exchange,
                "timeframe": requested_tf.as_str(),
                "candles": candles.iter().map(|c| serde_json::json!({
                    "open_time": c.open_time,
                    "open": c.open,
                    "high": c.high,
                    "low": c.low,
                    "close": c.close,
                    "volume": c.volume,
                })).collect::<Vec<_>>(),
            }))));
        }
    }


    let exchange_key = format!("{}:{}", exchange, virs_types::MarketType::Perpetual);
    match state.exchange_registry.get(&exchange_key) {
        Some(ex) => {
            let tf_str = match requested_tf {
                virs_market::Timeframe::M1 => "1m",
                virs_market::Timeframe::M5 => "5m",
                virs_market::Timeframe::M15 => "15m",
                virs_market::Timeframe::H1 => "1h",
                virs_market::Timeframe::H4 => "4h",
                virs_market::Timeframe::D1 => "1d",
            };
            match ex.get_klines(symbol, tf_str, 500, None).await {
                Ok(klines) => Ok(Json(ApiResponse::ok(serde_json::json!({
                    "symbol": symbol,
                    "exchange": exchange,
                    "timeframe": tf_str,
                    "candles": klines.iter().map(|k| serde_json::json!({
                        "open_time": k.open_time,
                        "open": k.open,
                        "high": k.high,
                        "low": k.low,
                        "close": k.close,
                        "volume": k.volume,
                    })).collect::<Vec<_>>(),
                })))),
                Err(e) => Err(VirsError::bad_request(format!("Klines error: {}", e))),
            }
        }
        None => Err(VirsError::bad_request(format!(
            "Exchange '{}' not registered",
            exchange
        ))),
    }
}

pub async fn get_order_book(
    State(state): State<AppState>,
    Query(params): Query<SymbolQuery>,
) -> Result<Json<ApiResponse>, VirsError> {
    let exchange = match params.exchange {
        Some(ref e) => e,
        None => return Err(VirsError::bad_request("exchange is required")),
    };
    let symbol = match params.symbol {
        Some(ref s) => s,
        None => return Err(VirsError::bad_request("symbol is required")),
    };

    let exchange_key = format!("{}:{}", exchange, virs_types::MarketType::Perpetual);
    match state.exchange_registry.get(&exchange_key) {
        // ExchangePe 统一 trait 不再提供 get_order_book 接口
        Some(_ex) => Err(VirsError::bad_request(format!(
            "Order book for symbol '{}' is not supported via the unified ExchangePe interface",
            symbol
        ))),
        None => Err(VirsError::bad_request(format!(
            "Exchange '{}' not registered",
            exchange
        ))),
    }
}

pub async fn get_balances(
    State(state): State<AppState>,
    Query(params): Query<SymbolQuery>,
) -> Result<Json<ApiResponse>, VirsError> {
    let exchange = match params.exchange {
        Some(ref e) => e,
        None => return Err(VirsError::bad_request("exchange is required")),
    };

    let exchange_key = format!("{}:{}", exchange, virs_types::MarketType::Perpetual);
    match state.exchange_registry.get(&exchange_key) {
        // ExchangePe::get_balance() 返回单个（通常为 USDT）余额
        Some(ex) => match ex.get_balance().await {
            Ok(b) => {
                let filtered: Vec<_> = if b.total > 0.0 {
                    vec![serde_json::json!({
                        "asset": b.asset,
                        "total": b.total,
                        "free": b.free,
                        "used": b.used,
                    })]
                } else {
                    vec![]
                };
                Ok(Json(ApiResponse::ok(serde_json::json!({ "balances": filtered }))))
            }
            Err(e) => Err(VirsError::bad_request(format!("Balances error: {}", e))),
        },
        None => Err(VirsError::bad_request(format!(
            "Exchange '{}' not registered",
            exchange
        ))),
    }
}

pub async fn get_symbols(
    State(state): State<AppState>,
    Query(params): Query<SymbolQuery>,
) -> Result<Json<ApiResponse>, VirsError> {
    let exchange = match params.exchange {
        Some(ref e) => e,
        None => return Err(VirsError::bad_request("exchange is required")),
    };

    let exchange_key = format!("{}:{}", exchange, virs_types::MarketType::Perpetual);
    match state.exchange_registry.get(&exchange_key) {
        Some(ex) => match ex.get_symbols().await {
            Ok(symbols) => Ok(Json(ApiResponse::ok(serde_json::json!({
                "exchange": exchange,
                "symbols": symbols,
            })))),
            Err(e) => Err(VirsError::bad_request(format!("Symbols error: {}", e))),
        },
        None => Err(VirsError::bad_request(format!(
            "Exchange '{}' not registered",
            exchange
        ))),
    }
}
