//! Binance Spot REST API (/api/v3) — market data and spot trading.
//!
//! Endpoints:
//! - GET  /api/v3/ping
//! - GET  /api/v3/ticker/24hr
//! - GET  /api/v3/klines
//! - GET  /api/v3/depth
//! - GET  /api/v3/exchangeInfo
//! - GET  /api/v3/account
//! - GET  /api/v3/order
//! - GET  /api/v3/openOrders
//! - POST /api/v3/order
//! - POST /api/v3/userDataStream

use chrono::Utc;

use crate::auth::Signer;
use crate::types::*;
use crate::ExchangeClient;
use crate::{parse_f64, parse_str};
use virs_error::ExchangeError;

use super::parse_order_book_side;

const BASE_URL: &str = "https://api.binance.com";

fn url(path: &str) -> String {
    format!("{BASE_URL}{path}")
}

/// 从币安 exchangeInfo 过滤器的小数值字符串（如 tickSize="0.01000000"）
/// 推算小数位数（精度）。尾部零不计入。
///
/// 现货 exchangeInfo 无 pricePrecision/quantityPrecision 字段（合约专属），
/// 必须从 PRICE_FILTER.tickSize / LOT_SIZE.stepSize 推算。
pub(crate) fn decimal_places(s: &str) -> Option<u32> {
    // 币安过滤器值均为纯小数字符串（如 "0.01000000"），不含科学计数法
    let dot_pos = s.find('.')?;
    let after_dot = &s[dot_pos + 1..];
    let trimmed = after_dot.trim_end_matches('0');
    if trimmed.is_empty() {
        Some(0)
    } else {
        Some(trimmed.len() as u32)
    }
}

// ---- Public endpoints ----

/// GET /api/v3/ping
pub async fn ping(client: &ExchangeClient) -> Result<bool, ExchangeError> {
    let data = client.public_get(&url("/api/v3/ping"), &[]).await?;
    Ok(!data.is_null())
}

/// GET /api/v3/time — 获取币安现货服务器时间（毫秒）
///
/// 用于 sync_time() 校准本地时钟偏移，避免 -1021 签名失败。
pub async fn fetch_server_time(client: &ExchangeClient) -> Result<i64, ExchangeError> {
    let data = client.public_get(&url("/api/v3/time"), &[]).await?;
    data.get("serverTime")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| {
            ExchangeError::Internal("serverTime missing in /api/v3/time response".into())
        })
}

/// GET /api/v3/ticker/24hr
pub async fn fetch_ticker(
    client: &ExchangeClient,
    symbol: &str,
) -> Result<CcxtTicker, ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let data = client
        .public_get(&url("/api/v3/ticker/24hr"), &[("symbol", native.as_str())])
        .await?;

    let last = parse_f64(&data, "lastPrice");
    if last.is_none() || last == Some(0.0) {
        return Err(ExchangeError::no_data(format!(
            "No ticker data available for {} on Binance",
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

/// GET /api/v3/klines
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
            &url("/api/v3/klines"),
            &params
                .iter()
                .map(|(k, v)| (*k, v.as_str()))
                .collect::<Vec<_>>(),
        )
        .await?;

    let arr = data.as_array().ok_or_else(|| {
        ExchangeError::no_data(format!("Invalid kline response for {} on Binance", symbol))
    })?;

    if arr.is_empty() {
        return Err(ExchangeError::no_data(format!(
            "No OHLCV data available for {} ({}) on Binance",
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
            "All kline entries invalid for {} ({}) on Binance",
            symbol, timeframe
        )));
    }

    Ok(klines)
}

/// GET /api/v3/depth
pub async fn fetch_order_book(
    client: &ExchangeClient,
    symbol: &str,
    limit: u32,
) -> Result<CcxtOrderBook, ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let data = client
        .public_get(
            &url("/api/v3/depth"),
            &[("symbol", native.as_str()), ("limit", &limit.to_string())],
        )
        .await?;

    let bids = parse_order_book_side(&data, "bids");
    let asks = parse_order_book_side(&data, "asks");

    if bids.is_empty() && asks.is_empty() {
        return Err(ExchangeError::no_data(format!(
            "No order book data for {} on Binance",
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

/// GET /api/v3/exchangeInfo
pub async fn fetch_markets(client: &ExchangeClient) -> Result<Vec<MarketInfo>, ExchangeError> {
    let data = client.public_get(&url("/api/v3/exchangeInfo"), &[]).await?;

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

            let base = parse_str(s, "baseAsset")?;
            let quote = parse_str(s, "quoteAsset")?;
            let symbol = format!("{}/{}", base, quote);

            let filters = s.get("filters").and_then(|f| f.as_array());
            let (min_amount, max_amount, step_size) = filters
                .map(|arr| {
                    let lot = arr
                        .iter()
                        .find(|f| f.get("filterType").and_then(|v| v.as_str()) == Some("LOT_SIZE"));
                    (
                        lot.and_then(|f| parse_f64(f, "minQty")),
                        lot.and_then(|f| parse_f64(f, "maxQty")),
                        lot.and_then(|f| parse_str(f, "stepSize")),
                    )
                })
                .unwrap_or((None, None, None));
            let (min_price, max_price, tick_size) = filters
                .map(|arr| {
                    let pf = arr.iter().find(|f| {
                        f.get("filterType").and_then(|v| v.as_str()) == Some("PRICE_FILTER")
                    });
                    (
                        pf.and_then(|f| parse_f64(f, "minPrice")),
                        pf.and_then(|f| parse_f64(f, "maxPrice")),
                        pf.and_then(|f| parse_str(f, "tickSize")),
                    )
                })
                .unwrap_or((None, None, None));
            // 现货 NOTIONAL 字段为 minNotional（合约 MIN_NOTIONAL 才用 notional）
            let min_cost = filters
                .and_then(|arr| {
                    arr.iter().find(|f| {
                        f.get("filterType")
                            .and_then(|v| v.as_str())
                            .map(|t| t == "NOTIONAL")
                            .unwrap_or(false)
                    })
                })
                .and_then(|f| parse_f64(f, "minNotional"));

            // 现货 exchangeInfo 无 pricePrecision/quantityPrecision（合约专属）
            // 从 PRICE_FILTER.tickSize 和 LOT_SIZE.stepSize 的小数位推算精度
            let price_precision = tick_size.as_deref().and_then(decimal_places);
            let amount_precision = step_size.as_deref().and_then(decimal_places);

            Some(MarketInfo {
                id: parse_str(s, "symbol")?,
                symbol,
                base,
                quote,
                active: true,
                market_type: MarketType::Spot,
                min_amount,
                max_amount,
                min_price,
                max_price,
                min_cost,
                price_precision,
                amount_precision,
                info: s.clone(),
            })
        })
        .collect();

    Ok(markets)
}

// ---- Authenticated endpoints ----

/// GET /api/v3/account — spot balances
pub async fn fetch_balance(
    client: &ExchangeClient,
    signer: &dyn Signer,
) -> Result<Vec<Balance>, ExchangeError> {
    let data = client
        .signed_get(signer, &url("/api/v3/account"), vec![])
        .await?;

    let balances = data
        .get("balances")
        .and_then(|b| b.as_array())
        .ok_or_else(|| ExchangeError::Internal("Invalid balance response from Binance".into()))?;

    let mut result: Vec<Balance> = balances
        .iter()
        .filter_map(|b| {
            let asset = parse_str(b, "asset").unwrap_or_else(|| {
                tracing::warn!("Balance asset field missing — skipping entry");
                String::new()
            });
            if asset.is_empty() {
                return None;
            }
            let free = parse_f64(b, "free").unwrap_or_else(|| {
                tracing::warn!(asset = %asset, "Balance 'free' field missing or unparseable — skipping entry to avoid 0.0 propagation");
                f64::NAN
            });
            if free.is_nan() {
                return None;
            }
            let used = parse_f64(b, "locked").unwrap_or_else(|| {
                tracing::warn!(asset = %asset, "Balance 'locked' field missing or unparseable — skipping entry to avoid 0.0 propagation");
                f64::NAN
            });
            if used.is_nan() {
                return None;
            }
            if free == 0.0 && used == 0.0 {
                return None;
            }
            Some(Balance {
                asset,
                free,
                used,
                total: 0.0,
            })
        })
        .collect();
    for b in &mut result {
        b.total = b.compute_total();
    }

    Ok(result)
}

/// POST /api/v3/order — create spot order
pub async fn create_order(
    client: &ExchangeClient,
    signer: &dyn Signer,
    params: PlaceOrderParams,
) -> Result<CcxtOrder, ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(&params.symbol);
    let mut body = serde_json::json!({
        "symbol": native,
        "side": crate::adapter::binance::BinanceExchange::side_str(&params.side),
        "type": crate::adapter::binance::BinanceExchange::order_type_str(&params.order_type),
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

    let data = client
        .signed_post(signer, &url("/api/v3/order"), body)
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
    // 现货下单响应包含 `transactTime`（交易时间）
    let transact_time = crate::parse_timestamp_ms(&data, "transactTime").unwrap_or_else(|| {
        tracing::warn!("spot order response missing 'transactTime' — using local time");
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
        created_at: Some(transact_time),
        updated_at: Some(transact_time),
        info: data,
    })
}

/// DELETE /api/v3/order — cancel spot order
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
        .signed_delete(signer, &url("/api/v3/order"), params)
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

/// GET /api/v3/order — fetch spot order
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
        .signed_get(signer, &url("/api/v3/order"), params)
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

/// GET /api/v3/openOrders — fetch open spot orders
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
        .signed_get(signer, &url("/api/v3/openOrders"), params)
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

/// POST /api/v3/userDataStream — create listen key for spot user data stream
///
/// ⚠️ 已废弃（2025-04-25 Spot API Changelog）：币安推荐迁移到 WebSocket API 的
/// `userDataStream.subscribe` 方法（需 Ed25519 API Key）。当前端点仍可工作，
/// 但将在未来被移除。本实现保留作为 HMAC 密钥的兼容路径；若币安返回 410 Gone
/// 或 -2013 类错误，调用方应降级为 REST 轮询或迁移到 Ed25519。
pub async fn create_listen_key(
    client: &ExchangeClient,
    signer: &dyn Signer,
) -> Result<String, ExchangeError> {
    tracing::warn!(
        target: "binance_api",
        "POST /api/v3/userDataStream is deprecated by Binance; migrate to WebSocket API (Ed25519) when possible"
    );
    let body = serde_json::json!({});
    let data = client
        .signed_post(signer, &url("/api/v3/userDataStream"), body)
        .await?;
    data.get("listenKey")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| ExchangeError::Internal("listenKey missing in response".into()))
}

/// PUT /api/v3/userDataStream — keepalive listen key
///
/// ⚠️ 已废弃（同上）。建议迁移到 WebSocket API 后通过 `userDataStream.ping` 保活。
pub async fn keepalive_listen_key(
    client: &ExchangeClient,
    signer: &dyn Signer,
    listen_key: &str,
) -> Result<(), ExchangeError> {
    tracing::warn!(
        target: "binance_api",
        "PUT /api/v3/userDataStream is deprecated by Binance; migrate to WebSocket API (Ed25519) when possible"
    );
    let body = serde_json::json!({ "listenKey": listen_key });
    client
        .signed_put(signer, &url("/api/v3/userDataStream"), body)
        .await?;
    Ok(())
}
