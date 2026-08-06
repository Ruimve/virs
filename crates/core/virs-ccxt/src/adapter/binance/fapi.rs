use chrono::Utc;

use crate::auth::Signer;
use crate::types::*;
use crate::ExchangeClient;
use crate::{parse_f64, parse_str, parse_u32};
use virs_error::ExchangeError;
use virs_type::{
    FundingRate, Kline, Ticker, Balance, ExchangePosition, MarginMode, MarketType,
    OrderResult, PlaceOrderParams, PositionMode, PositionSide, Side, TimeInForce,
};

// 币安U本位合约 API 基础域名
const BASE_URL: &str = "https://fapi.binance.com";

// 拼接完整请求 URL
fn url(path: &str) -> String {
    format!("{BASE_URL}{path}")
}

/// 将 interval 字符串转为毫秒数
fn timeframe_to_ms(interval: &str) -> i64 {
    match interval {
        "1m" => 60_000,
        "5m" => 300_000,
        "15m" => 900_000,
        "30m" => 1_800_000,
        "1h" => 3_600_000,
        "4h" => 14_400_000,
        "1d" => 86_400_000,
        "1w" => 604_800_000,
        _ => 3_600_000,
    }
}

// 连通性检测
pub async fn ping(client: &ExchangeClient) -> Result<bool, ExchangeError> {
    let data = client.public_get(&url("/fapi/v1/ping"), &[]).await?;
    Ok(!data.is_null())
}

// 获取服务器时间
pub async fn fetch_server_time(client: &ExchangeClient) -> Result<i64, ExchangeError> {
    let data = client.public_get(&url("/fapi/v1/time"), &[]).await?;
    data.get("serverTime")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| {
            ExchangeError::Internal("serverTime missing in /fapi/v1/time response".into())
        })
}

// 24小时行情统计 + 最优挂单
pub async fn fetch_ticker(
    client: &ExchangeClient,
    symbol: &str,
) -> Result<Ticker, ExchangeError> {
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
    let last = last.unwrap();

    let book = client
        .public_get(
            &url("/fapi/v1/ticker/bookTicker"),
            &[("symbol", native.as_str())],
        )
        .await?;

    let bid = parse_f64(&book, "bidPrice");
    let ask = parse_f64(&book, "askPrice");
    let high_24h = parse_f64(&data, "highPrice").ok_or_else(|| {
        ExchangeError::no_data(format!("Ticker high_24h missing for {}", symbol))
    })?;
    let low_24h = parse_f64(&data, "lowPrice").ok_or_else(|| {
        ExchangeError::no_data(format!("Ticker low_24h missing for {}", symbol))
    })?;
    let volume_24h = parse_f64(&data, "volume").ok_or_else(|| {
        ExchangeError::no_data(format!("Ticker volume_24h missing for {}", symbol))
    })?;
    let price_change_24h = parse_f64(&data, "priceChange").ok_or_else(|| {
        ExchangeError::no_data(format!("Ticker price_change_24h missing for {}", symbol))
    })?;
    let price_change_pct_24h = parse_f64(&data, "priceChangePercent").ok_or_else(|| {
        ExchangeError::no_data(format!(
            "Ticker price_change_pct_24h missing for {}",
            symbol
        ))
    })?;

    // 解析交易所原生时间戳：closeTime 是 24h 滚动窗口的结束时间
    let timestamp = data
        .get("closeTime")
        .and_then(|v| v.as_i64())
        .and_then(chrono::DateTime::from_timestamp_millis)
        .unwrap_or_else(|| {
            tracing::warn!(symbol = %symbol, "Ticker closeTime missing — falling back to Utc::now()");
            Utc::now()
        });

    Ok(Ticker {
        symbol: symbol.to_string(),
        exchange: "binance".into(),
        bid,
        ask,
        last,
        high_24h,
        low_24h,
        volume_24h,
        price_change_24h,
        price_change_pct_24h,
        timestamp,
    })
}

// K线数据
pub async fn fetch_ohlcv(
    client: &ExchangeClient,
    symbol: &str,
    timeframe: &str,
    limit: u32,
    since: Option<i64>,
) -> Result<Vec<Kline>, ExchangeError> {
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

    let interval_ms = timeframe_to_ms(timeframe);
    let exchange_name = "binance";

    let klines: Vec<Kline> = arr
        .iter()
        .filter_map(|k| {
            let a = match k.as_array() {
                Some(a) if a.len() >= 6 => a,
                _ => return None,
            };
            let timestamp = a[0].as_i64()?;
            let close_time_raw = a.get(6).and_then(|v| v.as_i64());
            let open = a[1].as_str().and_then(|s| s.parse().ok())?;
            let high = a[2].as_str().and_then(|s| s.parse().ok())?;
            let low = a[3].as_str().and_then(|s| s.parse().ok())?;
            let close = a[4].as_str().and_then(|s| s.parse().ok())?;
            let volume = a[5].as_str().and_then(|s| s.parse().ok())?;
            let quote_volume = a
                .get(7)
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| {
                    tracing::warn!("Kline quote_volume is None — defaulting to 0.0");
                    0.0
                });
            let trades = a
                .get(8)
                .and_then(|v| v.as_i64())
                .unwrap_or_else(|| {
                    tracing::warn!("Kline trades count is None — defaulting to 0");
                    0
                });
            Some(Kline {
                open_time: timestamp,
                open,
                high,
                low,
                close,
                volume,
                close_time: close_time_raw.unwrap_or(timestamp + interval_ms - 1),
                quote_volume,
                trades,
                symbol: symbol.to_string(),
                exchange: exchange_name.to_string(),
                interval: timeframe.to_string(),
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

// 交易对信息
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
                        f.get("filterType").and_then(|v| v.as_str()) == Some("MIN_NOTIONAL")
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

// 标记价格和资金费率
pub async fn fetch_funding_rate(
    client: &ExchangeClient,
    symbol: &str,
) -> Result<FundingRate, ExchangeError> {
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
        .filter(|&ts| ts > 0)
        .and_then(|ts| {
            chrono::DateTime::from_timestamp_millis(ts).or_else(|| {
                tracing::warn!(ts, %symbol, "nextFundingTime timestamp invalid — returning None");
                None
            })
        });

    Ok(FundingRate {
        symbol: symbol.to_string(),
        rate,
        next_funding_time,
    })
}

// 合约账户余额
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

// 下单
pub async fn create_order(
    client: &ExchangeClient,
    signer: &dyn Signer,
    params: PlaceOrderParams,
) -> Result<OrderResult, ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(&params.symbol);
    let mut body = serde_json::json!({
        "symbol": native,
        "side": crate::adapter::binance::BinanceExchange::side_str(&params.side),
        "type": crate::adapter::binance::BinanceExchange::order_type_str(&params.order_type),
        "quantity": params.amount,
    });

    if let Some(price) = params.price {
        body["price"] = serde_json::json!(price);
        let tif = params.time_in_force.as_ref().ok_or_else(|| {
            ExchangeError::InvalidRequest(
                "time_in_force is required for limit orders".into(),
            )
        })?;
        body["timeInForce"] = serde_json::json!(match tif {
            TimeInForce::Gtc => "GTC",
            TimeInForce::Ioc => "IOC",
            TimeInForce::Fok => "FOK",
            TimeInForce::Gtx => "GTX",
            TimeInForce::Gtd => "GTD",
        });
    }

    if let Some(stop_price) = params.stop_price {
        body["stopPrice"] = serde_json::json!(stop_price);
    }

    if let Some(ref client_id) = params.client_order_id {
        body["newClientOrderId"] = serde_json::json!(client_id);
    }

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
        (Side::Unknown(raw), _) => {
            return Err(ExchangeError::InvalidRequest(
                format!("side is Unknown({}) — cannot place order with unknown side", raw),
            ));
        }
        (_, Some(PositionSide::Unknown(raw))) => {
            return Err(ExchangeError::InvalidRequest(
                format!("position_side is Unknown({}) — cannot place order with unknown position side", raw),
            ));
        }
    };
    body["positionSide"] = serde_json::json!(position_side);

    let data = client
        .signed_post(signer, &url("/fapi/v1/order"), body)
        .await?;

    let order_id = parse_str(&data, "orderId")
        .ok_or_else(|| ExchangeError::no_data("orderId missing in create_order response".into()))?;
    let client_order_id = parse_str(&data, "clientOrderId")
        .ok_or_else(|| ExchangeError::no_data("clientOrderId missing in create_order response".into()))?;

    Ok(OrderResult {
        order_id,
        client_order_id,
    })
}

// 撤单
pub async fn cancel_order(
    client: &ExchangeClient,
    signer: &dyn Signer,
    symbol: &str,
    order_id: &str,
) -> Result<OrderResult, ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let params = vec![
        ("symbol".into(), native),
        ("orderId".into(), order_id.to_string()),
    ];

    let data = client
        .signed_delete(signer, &url("/fapi/v1/order"), params)
        .await?;

    let order_id = parse_str(&data, "orderId")
        .ok_or_else(|| ExchangeError::no_data("orderId missing in cancel_order response".into()))?;
    let client_order_id = parse_str(&data, "clientOrderId")
        .ok_or_else(|| ExchangeError::no_data("clientOrderId missing in cancel_order response".into()))?;

    Ok(OrderResult {
        order_id,
        client_order_id,
    })
}

// 批量撤单
pub async fn cancel_all_orders(
    client: &ExchangeClient,
    signer: &dyn Signer,
    symbol: &str,
) -> Result<(), ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let params = vec![("symbol".into(), native)];

    let _data = client
        .signed_delete(signer, &url("/fapi/v1/allOpenOrders"), params)
        .await?;

    Ok(())
}

// 变换逐全仓模式
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

    let _ = client
        .signed_post(signer, &url("/fapi/v1/marginType"), body)
        .await;
    Ok(())
}

// 调整杠杆
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

// 持仓信息
pub async fn fetch_positions(
    client: &ExchangeClient,
    signer: &dyn Signer,
    symbol: Option<&str>,
) -> Result<Vec<ExchangePosition>, ExchangeError> {
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

    let mut positions: Vec<ExchangePosition> = Vec::new();
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

        let margin_type_str = match parse_str(p, "marginType") {
            Some(s) => s,
            None => {
                tracing::warn!(symbol = %symbol_str, "positionRisk marginType missing — skipping position");
                continue;
            }
        };
        let margin_mode = match margin_type_str.as_str() {
            "isolated" => MarginMode::Isolated,
            "cross" => MarginMode::Cross,
            other => {
                tracing::warn!(symbol = %symbol_str, margin_type = %other, "positionRisk unknown marginType — skipping position");
                continue;
            }
        };

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

        positions.push(ExchangePosition {
            symbol: crate::adapter::binance::BinanceExchange::to_unified_symbol(&symbol_str),
            side,
            quantity: size,
            entry_price,
            margin_mode,
            info: p.clone(),
        });
    }

    Ok(positions)
}

// 查询持仓模式
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
        .ok_or_else(|| {
            ExchangeError::InvalidRequest(
                "Unexpected response from Binance: 'dualSidePosition' field missing or not a boolean".into(),
            )
        })?;

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

// 创建 listenKey
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

// 续期 listenKey
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
