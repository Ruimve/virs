//! Binance USDT-M Futures REST API (/fapi/v1, /fapi/v2, /fapi/v3) — perpetual futures.
//!
//! Endpoints:
//! - GET  /fapi/v1/ping
//! - GET  /fapi/v1/ticker/24hr
//! - GET  /fapi/v1/klines
//! - GET  /fapi/v1/depth
//! - GET  /fapi/v1/exchangeInfo
//! - GET  /fapi/v3/balance
//! - GET  /fapi/v1/order
//! - GET  /fapi/v1/openOrders
//! - POST /fapi/v1/order
//! - POST /fapi/v1/marginType
//! - POST /fapi/v1/leverage
//! - GET  /fapi/v2/positionRisk
//! - GET  /fapi/v1/positionSide/dual
//! - GET  /fapi/v1/premiumIndex
//! - GET  /fapi/v1/fundingRate
//! - POST /fapi/v1/listenKey
//! - PUT  /fapi/v1/listenKey

use chrono::Utc;

use crate::auth::Signer;
use crate::types::*;
use crate::ExchangeClient;
use crate::{parse_f64, parse_str, parse_u32};
use virs_error::ExchangeError;

use super::parse_order_book_side;

const BASE_URL: &str = "https://fapi.binance.com";

fn url(path: &str) -> String {
    format!("{BASE_URL}{path}")
}

// ---- Public endpoints ----

/// GET /fapi/v1/ping
pub async fn ping(client: &ExchangeClient) -> Result<bool, ExchangeError> {
    let data = client.public_get(&url("/fapi/v1/ping"), &[]).await?;
    Ok(!data.is_null())
}

/// GET /fapi/v1/time — 获取币安合约服务器时间（毫秒）
///
/// 用于 sync_time() 校准本地时钟偏移，避免 -1021 签名失败。
pub async fn fetch_server_time(client: &ExchangeClient) -> Result<i64, ExchangeError> {
    let data = client.public_get(&url("/fapi/v1/time"), &[]).await?;
    data.get("serverTime")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| {
            ExchangeError::Internal("serverTime missing in /fapi/v1/time response".into())
        })
}

/// GET /fapi/v1/ticker/24hr
pub async fn fetch_ticker(
    client: &ExchangeClient,
    symbol: &str,
) -> Result<CcxtTicker, ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let data = client
        .public_get(&url("/fapi/v1/ticker/24hr"), &[("symbol", native.as_str())])
        .await?;

    let last = parse_f64(&data, "lastPrice");
    if last.is_none() || last == Some(0.0) {
        return Err(ExchangeError::no_data(format!(
            "No ticker data available for {} on Binance Futures",
            symbol
        )));
    }

    Ok(CcxtTicker {
        symbol: symbol.to_string(),
        exchange: "binance".into(),
        bid: parse_f64(&data, "bidPrice"),
        ask: parse_f64(&data, "askPrice"),
        last,
        high: parse_f64(&data, "highPrice"),
        low: parse_f64(&data, "lowPrice"),
        volume: parse_f64(&data, "volume"),
        quote_volume: parse_f64(&data, "quoteVolume"),
        open: parse_f64(&data, "openPrice"),
        close: parse_f64(&data, "lastPrice"),
        previous_close: parse_f64(&data, "prevClosePrice"),
        price_change: parse_f64(&data, "priceChange"),
        price_change_pct: parse_f64(&data, "priceChangePercent"),
        timestamp: Some(Utc::now()),
        info: data,
    })
}

/// GET /fapi/v1/klines
pub async fn fetch_ohlcv(
    client: &ExchangeClient,
    symbol: &str,
    timeframe: &str,
    limit: u32,
    since: Option<i64>,
) -> Result<Vec<CcxtKline>, ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let mut params: Vec<(&str, String)> = vec![
        ("symbol", native),
        ("interval", timeframe.to_string()),
        ("limit", limit.to_string()),
    ];
    if let Some(s) = since {
        params.push(("startTime", s.to_string()));
    }

    let data = client
        .public_get(
            &url("/fapi/v1/klines"),
            &params
                .iter()
                .map(|(k, v)| (*k, v.as_str()))
                .collect::<Vec<_>>(),
        )
        .await?;

    let arr = data.as_array().ok_or_else(|| {
        ExchangeError::no_data(format!(
            "Invalid kline response for {} on Binance Futures",
            symbol
        ))
    })?;

    if arr.is_empty() {
        return Err(ExchangeError::no_data(format!(
            "No OHLCV data available for {} ({}) on Binance Futures",
            symbol, timeframe
        )));
    }

    let klines: Vec<CcxtKline> = arr
        .iter()
        .filter_map(|k| {
            let a = match k.as_array() {
                Some(a) if a.len() >= 6 => a,
                _ => return None,
            };
            let timestamp = a[0].as_i64()?;
            let close_time = a.get(6).and_then(|v| v.as_i64());
            let open = a[1].as_str().and_then(|s| s.parse().ok())?;
            let high = a[2].as_str().and_then(|s| s.parse().ok())?;
            let low = a[3].as_str().and_then(|s| s.parse().ok())?;
            let close = a[4].as_str().and_then(|s| s.parse().ok())?;
            let volume = a[5].as_str().and_then(|s| s.parse().ok())?;
            Some(CcxtKline {
                timestamp,
                close_time,
                open,
                high,
                low,
                close,
                volume,
                quote_volume: a
                    .get(7)
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok()),
                trades: a.get(8).and_then(|v| v.as_i64()),
            })
        })
        .collect();

    if klines.is_empty() {
        return Err(ExchangeError::no_data(format!(
            "All kline entries invalid for {} ({}) on Binance Futures",
            symbol, timeframe
        )));
    }

    Ok(klines)
}

/// GET /fapi/v1/depth
///
/// `limit` 合法值集合：5, 10, 20, 50, 100, 500, 1000（币安合约文档）。
/// 传入非法值会返回 `ExchangeError::InvalidRequest`。
pub async fn fetch_order_book(
    client: &ExchangeClient,
    symbol: &str,
    limit: u32,
) -> Result<CcxtOrderBook, ExchangeError> {
    // F12: 合约 depth limit 合法值校验
    const VALID_FUTURES_DEPTH_LIMITS: &[u32] = &[5, 10, 20, 50, 100, 500, 1000];
    if !VALID_FUTURES_DEPTH_LIMITS.contains(&limit) {
        return Err(ExchangeError::InvalidRequest(format!(
            "Invalid depth limit {} for futures /fapi/v1/depth — valid values: {:?}",
            limit, VALID_FUTURES_DEPTH_LIMITS
        )));
    }

    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let data = client
        .public_get(
            &url("/fapi/v1/depth"),
            &[("symbol", native.as_str()), ("limit", &limit.to_string())],
        )
        .await?;

    let bids = parse_order_book_side(&data, "bids");
    let asks = parse_order_book_side(&data, "asks");

    if bids.is_empty() && asks.is_empty() {
        return Err(ExchangeError::no_data(format!(
            "No order book data for {} on Binance Futures",
            symbol
        )));
    }

    Ok(CcxtOrderBook {
        symbol: symbol.to_string(),
        bids,
        asks,
        timestamp: Some(Utc::now()),
        nonce: None,
    })
}

/// GET /fapi/v1/exchangeInfo
pub async fn fetch_markets(client: &ExchangeClient) -> Result<Vec<MarketInfo>, ExchangeError> {
    let data = client
        .public_get(&url("/fapi/v1/exchangeInfo"), &[])
        .await?;

    let symbols = data
        .get("symbols")
        .and_then(|s| s.as_array())
        .ok_or_else(|| ExchangeError::Internal("Invalid exchangeInfo response".into()))?;

    let markets: Vec<MarketInfo> = symbols
        .iter()
        .filter_map(|s| {
            let status = parse_str(s, "status")?;
            if status != "TRADING" {
                return None;
            }

            let contract_type = parse_str(s, "contractType")?;
            if contract_type != "PERPETUAL" {
                return None;
            }

            let base = parse_str(s, "baseAsset")?;
            let quote = parse_str(s, "quoteAsset")?;
            let symbol = format!("{}/{}", base, quote);

            let filters = s.get("filters").and_then(|f| f.as_array());
            let (min_amount, max_amount) = filters
                .map(|arr| {
                    let lot = arr
                        .iter()
                        .find(|f| f.get("filterType").and_then(|v| v.as_str()) == Some("LOT_SIZE"));
                    (
                        lot.and_then(|f| parse_f64(f, "minQty")),
                        lot.and_then(|f| parse_f64(f, "maxQty")),
                    )
                })
                .unwrap_or((None, None));
            let (min_price, max_price) = filters
                .map(|arr| {
                    let pf = arr.iter().find(|f| {
                        f.get("filterType").and_then(|v| v.as_str()) == Some("PRICE_FILTER")
                    });
                    (
                        pf.and_then(|f| parse_f64(f, "minPrice")),
                        pf.and_then(|f| parse_f64(f, "maxPrice")),
                    )
                })
                .unwrap_or((None, None));
            let min_cost = filters
                .and_then(|arr| {
                    arr.iter().find(|f| {
                        f.get("filterType")
                            .and_then(|v| v.as_str())
                            .map(|t| t == "MIN_NOTIONAL")
                            .unwrap_or(false)
                    })
                })
                .and_then(|f| parse_f64(f, "notional"));

            Some(MarketInfo {
                id: parse_str(s, "symbol")?,
                symbol,
                base,
                quote,
                active: true,
                market_type: MarketType::Perpetual,
                min_amount,
                max_amount,
                min_price,
                max_price,
                min_cost,
                price_precision: parse_u32(s, "pricePrecision"),
                amount_precision: parse_u32(s, "quantityPrecision"),
                info: s.clone(),
            })
        })
        .collect();

    Ok(markets)
}

/// GET /fapi/v1/premiumIndex — funding rate
pub async fn fetch_funding_rate(
    client: &ExchangeClient,
    symbol: &str,
) -> Result<CcxtFundingRate, ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let data = client
        .public_get(
            &url("/fapi/v1/premiumIndex"),
            &[("symbol", native.as_str())],
        )
        .await?;

    let rate = parse_f64(&data, "lastFundingRate").ok_or_else(|| {
        tracing::warn!(symbol = %symbol, "lastFundingRate missing — returning NoData instead of 0.0");
        ExchangeError::no_data(format!("lastFundingRate missing for {symbol}"))
    })?;
    let next_funding_time = data
        .get("nextFundingTime")
        .and_then(|t| t.as_i64())
        // T7 WARN fix: nextFundingTime: 0 表示无下次资金费率时间，应返回 None 而非 Some(epoch)
        .filter(|&ts| ts > 0)
        .and_then(|ts| {
            chrono::DateTime::from_timestamp_millis(ts).or_else(|| {
                tracing::warn!(ts, %symbol, "nextFundingTime timestamp invalid — returning None");
                None
            })
        });

    Ok(CcxtFundingRate {
        symbol: symbol.to_string(),
        rate,
        next_funding_time,
        info: data,
    })
}

/// GET /fapi/v1/fundingRate — funding rate history
pub async fn fetch_funding_history(
    client: &ExchangeClient,
    symbol: &str,
    start_time: i64,
    end_time: i64,
) -> Result<Vec<CcxtFundingHistoryEntry>, ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let mut all_entries: Vec<CcxtFundingHistoryEntry> = Vec::new();
    let mut current_start = start_time;

    loop {
        let data = client
            .public_get(
                &url("/fapi/v1/fundingRate"),
                &[
                    ("symbol", native.as_str()),
                    ("startTime", &current_start.to_string()),
                    ("endTime", &end_time.to_string()),
                    ("limit", "1000"),
                ],
            )
            .await?;

        let arr = data.as_array().ok_or_else(|| {
            ExchangeError::Internal("Invalid fundingRate history response".into())
        })?;

        if arr.is_empty() {
            break;
        }

        for item in arr {
            let funding_time = item
                .get("fundingTime")
                .and_then(|t| t.as_i64())
                .and_then(chrono::DateTime::from_timestamp_millis)
                .ok_or_else(|| {
                    tracing::warn!("fundingTime missing or invalid — returning NoData instead of 0");
                    ExchangeError::no_data("fundingTime missing or invalid in funding history".into())
                })?;
            let rate = parse_f64(item, "fundingRate").ok_or_else(|| {
                tracing::warn!("fundingRate missing — returning NoData instead of 0.0");
                ExchangeError::no_data("fundingRate missing in funding history".into())
            })?;
            all_entries.push(CcxtFundingHistoryEntry { funding_time, rate });
        }

        if arr.len() < 1000 {
            break;
        }

        if let Some(last) = arr.last() {
            current_start = last
                .get("fundingTime")
                .and_then(|t| t.as_i64())
                .unwrap_or(end_time)
                + 1;
        } else {
            break;
        }
    }

    Ok(all_entries)
}

// ---- Authenticated endpoints ----

/// GET /fapi/v3/balance — futures account balances
pub async fn fetch_balance(
    client: &ExchangeClient,
    signer: &dyn Signer,
) -> Result<Vec<Balance>, ExchangeError> {
    let data = client
        .signed_get(signer, &url("/fapi/v3/balance"), vec![])
        .await?;

    let balances = data.as_array().ok_or_else(|| {
        ExchangeError::Internal("Invalid futures balance response from Binance".into())
    })?;

    let result: Vec<Balance> = balances
        .iter()
        .filter_map(|b| {
            let asset = parse_str(b, "asset").unwrap_or_else(|| {
                tracing::warn!("Balance asset field missing — skipping entry");
                String::new()
            });
            if asset.is_empty() {
                return None;
            }
            let free = parse_f64(b, "availableBalance").unwrap_or_else(|| {
                tracing::warn!(asset = %asset, "Balance 'availableBalance' field missing or unparseable — skipping entry to avoid 0.0 propagation");
                f64::NAN
            });
            if free.is_nan() {
                return None;
            }
            let total = parse_f64(b, "balance").unwrap_or_else(|| {
                tracing::warn!(asset = %asset, "Balance 'balance' field missing or unparseable — skipping entry to avoid 0.0 propagation");
                f64::NAN
            });
            if total.is_nan() {
                return None;
            }
            let used = total - free;
            if free == 0.0 && used == 0.0 {
                return None;
            }
            Some(Balance {
                asset,
                free,
                used,
                total,
            })
        })
        .collect();

    Ok(result)
}

/// POST /fapi/v1/order — create futures order
pub async fn create_order(
    client: &ExchangeClient,
    signer: &dyn Signer,
    params: PlaceOrderParams,
) -> Result<CcxtOrder, ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(&params.symbol);
    let mut body = serde_json::json!({
        "symbol": native,
        "side": crate::adapter::binance::BinanceExchange::side_str(&params.side),
        "type": crate::adapter::binance::BinanceExchange::order_type_str_futures(&params.order_type),
        "quantity": params.amount,
    });

    if let Some(price) = params.price {
        body["price"] = serde_json::json!(price);
        body["timeInForce"] = serde_json::json!(params
            .time_in_force
            .as_ref()
            .map(|tif| match tif {
                TimeInForce::Gtc => "GTC",
                TimeInForce::Ioc => "IOC",
                TimeInForce::Fok => "FOK",
                TimeInForce::Poc => "GTX",
            })
            .unwrap_or("GTC"));
    }

    if let Some(stop_price) = params.stop_price {
        body["stopPrice"] = serde_json::json!(stop_price);
    }

    if let Some(ref client_id) = params.client_order_id {
        body["newClientOrderId"] = serde_json::json!(client_id);
    }

    // Perpetual: positionSide is mandatory in Hedge mode.
    // OneWay mode (positionSide=BOTH) is not supported — the caller must
    // resolve Long/Short before placing the order.
    let position_side = match (&params.side, &params.position_side) {
        (Side::Buy, Some(PositionSide::Long)) => "LONG",
        (Side::Sell, Some(PositionSide::Short)) => "SHORT",
        (Side::Buy, Some(PositionSide::Short)) => "SHORT",
        (Side::Sell, Some(PositionSide::Long)) => "LONG",
        (_, None) => {
            return Err(ExchangeError::InvalidRequest(
                "position_side is required in Hedge mode — OneWay (BOTH) is not supported. \
                 Switch the Binance futures account to Hedge mode and provide position_side."
                    .into(),
            ));
        }
    };
    body["positionSide"] = serde_json::json!(position_side);

    // reduceOnly is still valid in Hedge mode (marks a close order).
    if let Some(reduce) = params.reduce_only {
        if reduce {
            body["reduceOnly"] = serde_json::json!(true);
        }
    }

    let data = client
        .signed_post(signer, &url("/fapi/v1/order"), body)
        .await?;

    // Critical numeric fields MUST be present — propagate errors instead of
    // silently defaulting to 0.0 (C4 issue fix).
    let amount = parse_f64(&data, "origQty").ok_or_else(|| {
        ExchangeError::no_data("Order amount (origQty) missing in exchange response".into())
    })?;
    let filled = parse_f64(&data, "executedQty").ok_or_else(|| {
        ExchangeError::no_data("Order filled (executedQty) missing in exchange response".into())
    })?;

    // F11: 优先使用币安响应中的时间戳，而非本地时钟
    // 合约下单响应包含 `time`（创建时间）和 `updateTime`（最后更新时间）
    let created_at = crate::parse_timestamp_ms(&data, "time")
        .or_else(|| crate::parse_timestamp_ms(&data, "updateTime"))
        .unwrap_or_else(|| {
            tracing::warn!(symbol = %params.symbol, "futures order response missing 'time'/'updateTime' — using local time");
            Utc::now()
        });
    let updated_at = crate::parse_timestamp_ms(&data, "updateTime").unwrap_or_else(|| {
        tracing::warn!(symbol = %params.symbol, "futures order response missing 'updateTime' — using local time");
        Utc::now()
    });

    Ok(CcxtOrder {
        id: parse_str(&data, "orderId")
            .ok_or_else(|| ExchangeError::no_data("orderId missing".into()))?,
        client_order_id: parse_str(&data, "clientOrderId"),
        symbol: params.symbol,
        side: params.side,
        order_type: params.order_type,
        price: parse_f64(&data, "price"),
        amount,
        cost: None,
        filled,
        remaining: amount - filled,
        status: crate::adapter::binance::BinanceExchange::parse_order_status(
            &parse_str(&data, "status")
                .ok_or_else(|| ExchangeError::no_data("status missing".into()))?,
        ),
        fee: None,
        created_at: Some(created_at),
        updated_at: Some(updated_at),
        info: data,
    })
}

/// DELETE /fapi/v1/order — cancel futures order
pub async fn cancel_order(
    client: &ExchangeClient,
    signer: &dyn Signer,
    symbol: &str,
    order_id: &str,
) -> Result<CcxtOrder, ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let params = vec![
        ("symbol".into(), native),
        ("orderId".into(), order_id.to_string()),
    ];

    let data = client
        .signed_delete(signer, &url("/fapi/v1/order"), params)
        .await?;

    let side_str =
        parse_str(&data, "side").ok_or_else(|| ExchangeError::no_data("side missing".into()))?;
    let type_str =
        parse_str(&data, "type").ok_or_else(|| ExchangeError::no_data("type missing".into()))?;
    // Critical numeric fields MUST be present — propagate errors instead of
    // silently defaulting to 0.0 (C4 issue fix).
    let amount = parse_f64(&data, "origQty").ok_or_else(|| {
        ExchangeError::no_data("Order amount (origQty) missing in exchange response".into())
    })?;
    let filled = parse_f64(&data, "executedQty").ok_or_else(|| {
        ExchangeError::no_data("Order filled (executedQty) missing in exchange response".into())
    })?;
    Ok(CcxtOrder {
        id: parse_str(&data, "orderId")
            .ok_or_else(|| ExchangeError::no_data("orderId missing".into()))?,
        client_order_id: parse_str(&data, "clientOrderId"),
        symbol: symbol.to_string(),
        side: if side_str == "BUY" {
            Side::Buy
        } else {
            Side::Sell
        },
        order_type: crate::adapter::binance::BinanceExchange::parse_order_type(&type_str),
        price: parse_f64(&data, "price"),
        amount,
        cost: None,
        filled,
        remaining: amount - filled,
        status: CcxtOrderStatus::Canceled,
        fee: None,
        created_at: crate::parse_timestamp_ms(&data, "time")
            .or_else(|| {
                tracing::warn!(symbol = %symbol, "cancel_order response missing 'time' — created_at is None");
                None
            }),
        updated_at: crate::parse_timestamp_ms(&data, "updateTime").or_else(|| {
            tracing::warn!(symbol = %symbol, "cancel_order response missing 'updateTime' — using local time");
            Some(Utc::now())
        }),
        info: data,
    })
}

/// GET /fapi/v1/order — fetch futures order
pub async fn fetch_order(
    client: &ExchangeClient,
    signer: &dyn Signer,
    symbol: &str,
    order_id: &str,
) -> Result<CcxtOrder, ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let params = vec![
        ("symbol".into(), native),
        ("orderId".into(), order_id.to_string()),
    ];

    let data = client
        .signed_get(signer, &url("/fapi/v1/order"), params)
        .await?;

    let side_str =
        parse_str(&data, "side").ok_or_else(|| ExchangeError::no_data("side missing".into()))?;
    let type_str =
        parse_str(&data, "type").ok_or_else(|| ExchangeError::no_data("type missing".into()))?;
    let status_str = parse_str(&data, "status")
        .ok_or_else(|| ExchangeError::no_data("status missing".into()))?;
    // Critical numeric fields MUST be present — propagate errors instead of
    // silently defaulting to 0.0 (C4 issue fix).
    let amount = parse_f64(&data, "origQty").ok_or_else(|| {
        ExchangeError::no_data("Order amount (origQty) missing in exchange response".into())
    })?;
    let filled = parse_f64(&data, "executedQty").ok_or_else(|| {
        ExchangeError::no_data("Order filled (executedQty) missing in exchange response".into())
    })?;
    Ok(CcxtOrder {
        id: parse_str(&data, "orderId")
            .ok_or_else(|| ExchangeError::no_data("orderId missing".into()))?,
        client_order_id: parse_str(&data, "clientOrderId"),
        symbol: symbol.to_string(),
        side: if side_str == "BUY" {
            Side::Buy
        } else {
            Side::Sell
        },
        order_type: crate::adapter::binance::BinanceExchange::parse_order_type(&type_str),
        price: parse_f64(&data, "price"),
        amount,
        cost: None,
        filled,
        remaining: amount - filled,
        status: crate::adapter::binance::BinanceExchange::parse_order_status(&status_str),
        fee: None,
        created_at: crate::parse_timestamp_ms(&data, "time")
            .or_else(|| {
                tracing::warn!(symbol = %symbol, "fetch_order response missing 'time' — created_at is None");
                None
            }),
        updated_at: crate::parse_timestamp_ms(&data, "updateTime").or_else(|| {
            tracing::warn!(symbol = %symbol, "fetch_order response missing 'updateTime' — using local time");
            Some(Utc::now())
        }),
        info: data,
    })
}

/// GET /fapi/v1/openOrders — fetch open futures orders
pub async fn fetch_open_orders(
    client: &ExchangeClient,
    signer: &dyn Signer,
    symbol: Option<&str>,
) -> Result<Vec<CcxtOrder>, ExchangeError> {
    let params: Vec<(String, String)> = if let Some(sym) = symbol {
        vec![(
            "symbol".into(),
            crate::adapter::binance::BinanceExchange::to_native_symbol(sym),
        )]
    } else {
        vec![]
    };

    let data = client
        .signed_get(signer, &url("/fapi/v1/openOrders"), params)
        .await?;

    let arr = data.as_array().cloned().unwrap_or_default();
    let mut orders: Vec<CcxtOrder> = Vec::with_capacity(arr.len());
    for o in &arr {
        // Skip orders missing required identifier string fields (lenient parsing,
        // matching the previous filter_map behavior).
        let Some(side_str) = parse_str(o, "side") else {
            continue;
        };
        let Some(type_str) = parse_str(o, "type") else {
            continue;
        };
        let Some(status_str) = parse_str(o, "status") else {
            continue;
        };
        let Some(symbol_str) = parse_str(o, "symbol") else {
            continue;
        };
        let Some(id) = parse_str(o, "orderId") else {
            continue;
        };

        // Critical numeric fields MUST be present — propagate errors instead of
        // silently defaulting to 0.0 (C4 issue fix).
        let amount = parse_f64(o, "origQty").ok_or_else(|| {
            ExchangeError::no_data("Order amount (origQty) missing in exchange response".into())
        })?;
        let filled = parse_f64(o, "executedQty").ok_or_else(|| {
            ExchangeError::no_data("Order filled (executedQty) missing in exchange response".into())
        })?;

        orders.push(CcxtOrder {
            id,
            client_order_id: parse_str(o, "clientOrderId"),
            symbol: crate::adapter::binance::BinanceExchange::to_unified_symbol(&symbol_str),
            side: if side_str == "BUY" {
                Side::Buy
            } else {
                Side::Sell
            },
            order_type: crate::adapter::binance::BinanceExchange::parse_order_type(&type_str),
            price: parse_f64(o, "price"),
            amount,
            cost: None,
            filled,
            remaining: amount - filled,
            status: crate::adapter::binance::BinanceExchange::parse_order_status(&status_str),
            fee: None,
            created_at: crate::parse_timestamp_ms(o, "time"),
            updated_at: crate::parse_timestamp_ms(o, "updateTime"),
            info: o.clone(),
        });
    }

    Ok(orders)
}

/// POST /fapi/v1/marginType — set margin type (CROSSED / ISOLATED)
pub async fn set_margin_type(
    client: &ExchangeClient,
    signer: &dyn Signer,
    symbol: &str,
    margin_mode: MarginMode,
) -> Result<(), ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let margin_type_str = match margin_mode {
        MarginMode::Cross => "CROSSED",
        MarginMode::Isolated => "ISOLATED",
    };
    let body = serde_json::json!({
        "symbol": native,
        "marginType": margin_type_str,
    });
    // Ignore errors — may return "No need to change" if already set
    let _ = client
        .signed_post(signer, &url("/fapi/v1/marginType"), body)
        .await;
    Ok(())
}

/// POST /fapi/v1/leverage — set leverage
pub async fn set_leverage(
    client: &ExchangeClient,
    signer: &dyn Signer,
    symbol: &str,
    leverage: u32,
) -> Result<(), ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let body = serde_json::json!({
        "symbol": native,
        "leverage": leverage,
    });
    client
        .signed_post(signer, &url("/fapi/v1/leverage"), body)
        .await?;
    Ok(())
}

/// GET /fapi/v2/positionRisk — fetch open positions
pub async fn fetch_positions(
    client: &ExchangeClient,
    signer: &dyn Signer,
    symbol: Option<&str>,
) -> Result<Vec<Position>, ExchangeError> {
    let mut params: Vec<(String, String)> = vec![];
    if let Some(sym) = symbol {
        params.push((
            "symbol".into(),
            crate::adapter::binance::BinanceExchange::to_native_symbol(sym),
        ));
    }

    let data = client
        .signed_get(signer, &url("/fapi/v2/positionRisk"), params)
        .await?;

    let arr = data.as_array().ok_or_else(|| {
        ExchangeError::Internal("Invalid positionRisk response from Binance".into())
    })?;

    let mut positions: Vec<Position> = Vec::new();
    for p in arr.iter() {
            let pos_amt = parse_f64(p, "positionAmt").unwrap_or_else(|| {
                tracing::warn!("positionAmt missing — skipping entry to avoid silent position drop");
                f64::NAN
            });
            if pos_amt.is_nan() || pos_amt == 0.0 {
                continue;
            }

            let side = if pos_amt > 0.0 {
                PositionSide::Long
            } else {
                PositionSide::Short
            };
            let size = pos_amt.abs();

            let symbol_str = match parse_str(p, "symbol") {
                Some(s) => s,
                None => {
                    tracing::warn!("positionRisk symbol missing — skipping position");
                    continue;
                }
            };

            let margin_type_str = parse_str(p, "marginType").unwrap_or_else(|| {
                tracing::warn!(symbol = %symbol_str, "positionRisk marginType missing — defaulting to Cross");
                String::new()
            });
            let margin_mode = match margin_type_str.as_str() {
                "isolated" => MarginMode::Isolated,
                _ => MarginMode::Cross,
            };

            // entryPrice / leverage are critical fields; if either is missing
            // the position record is unusable, so log and skip it.
            let entry_price = match parse_f64(p, "entryPrice") {
                Some(v) => v,
                None => {
                    tracing::warn!(
                        symbol = %symbol_str,
                        "positionRisk entryPrice missing — skipping position"
                    );
                    continue;
                }
            };
            let leverage = match parse_u32(p, "leverage") {
                Some(v) => v,
                None => {
                    tracing::warn!(
                        symbol = %symbol_str,
                        "positionRisk leverage missing — skipping position"
                    );
                    continue;
                }
            };

            // unRealizedProfit — 金融值，禁止默认 0.0（项目约束）
            // 币安 positionRisk 响应始终包含此字段；若缺失则响应异常，返回 NoData
            let unrealized_pnl = match parse_f64(p, "unRealizedProfit") {
                Some(v) => v,
                None => {
                    tracing::error!(
                        symbol = %symbol_str,
                        "positionRisk unRealizedProfit missing — cannot calculate PnL"
                    );
                    return Err(ExchangeError::no_data(format!(
                        "unRealizedProfit missing for symbol {} in positionRisk response",
                        symbol_str
                    )));
                }
            };

            positions.push(Position {
                symbol: crate::adapter::binance::BinanceExchange::to_unified_symbol(&symbol_str),
                side,
                size,
                entry_price,
                leverage,
                unrealized_pnl,
                margin_mode,
                liquidation_price: parse_f64(p, "liquidationPrice"),
                info: p.clone(),
            });
    }

    Ok(positions)
}

/// GET /fapi/v1/positionSide/dual — get position mode.
/// Returns `Ok(Hedge)` when the exchange is in Hedge mode.
/// Returns `Err` when the exchange is in OneWay mode — VIRS does not support
/// OneWay. The `check_position_mode` API endpoint catches this error to
/// report "oneway" to the frontend wizard.
pub async fn get_position_mode(
    client: &ExchangeClient,
    signer: &dyn Signer,
) -> Result<PositionMode, ExchangeError> {
    let data = client
        .signed_get(signer, &url("/fapi/v1/positionSide/dual"), vec![])
        .await?;

    let dual_side = data
        .get("dualSidePosition")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if dual_side {
        Ok(PositionMode::Hedge)
    } else {
        Err(ExchangeError::InvalidRequest(
            "Exchange account is in OneWay (single-position) mode. \
             VIRS requires Hedge mode. Switch to Hedge mode in Binance futures \
             settings (API key > Position Mode > Hedge Mode)."
                .into(),
        ))
    }
}

/// POST /fapi/v1/listenKey — create listen key for futures user data stream
pub async fn create_listen_key(
    client: &ExchangeClient,
    signer: &dyn Signer,
) -> Result<String, ExchangeError> {
    let body = serde_json::json!({});
    let data = client
        .signed_post(signer, &url("/fapi/v1/listenKey"), body)
        .await?;
    data.get("listenKey")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| ExchangeError::Internal("listenKey missing in response".into()))
}

/// PUT /fapi/v1/listenKey — keepalive listen key
pub async fn keepalive_listen_key(
    client: &ExchangeClient,
    signer: &dyn Signer,
    listen_key: &str,
) -> Result<(), ExchangeError> {
    let body = serde_json::json!({ "listenKey": listen_key });
    client
        .signed_put(signer, &url("/fapi/v1/listenKey"), body)
        .await?;
    Ok(())
}
