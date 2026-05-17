//! Bybit exchange implementation.
//!
//! Implements the full CCXT-style Exchange trait for Bybit:
//! - REST API: https://api.bybit.com
//! - Testnet: https://api-testnet.bybit.com
//! - Auth: HMAC-SHA256 signature via headers (x-api-key, x-timestamp, x-sign)
//! - Rate limit: 10 req/s (public), 10 req/s (private)
//!
//! Supported features:
//! - Spot and Linear (USDT perpetual) trading
//! - Ticker, OHLCV, OrderBook, Balance
//! - Create/Cancel/Fetch orders

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tracing::info;

use crate::trading::ccxt::types::*;
use crate::trading::ccxt::errors::ExchangeError;
use crate::trading::ccxt::auth::{Signer, SignedRequest, hmac_sha256_hex, insert_header};
use crate::trading::ccxt::{Exchange, ExchangeClient, parse_str, parse_f64, parse_i64};

// ============================================================
// Bybit Signer (HMAC-SHA256, similar to Binance but with recv_window)
// ============================================================

/// Bybit-specific request signer.
pub struct BybitSigner {
    api_key: String,
    api_secret: String,
}

impl BybitSigner {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self { api_key, api_secret }
    }
}

impl Signer for BybitSigner {
    fn sign_get(
        &self,
        path: &str,
        query_params: &mut Vec<(String, String)>,
    ) -> Result<SignedRequest, ExchangeError> {
        let timestamp = Utc::now().timestamp_millis();
        query_params.push(("timestamp".into(), timestamp.to_string()));
        query_params.push(("recv_window".into(), "5000".into()));

        let query_string = query_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let pre_sign = format!("{}GET{}{}", timestamp, path, query_string);
        let signature = hmac_sha256_hex(&self.api_secret, &pre_sign);
        query_params.push(("sign".into(), signature));

        let mut headers = reqwest::header::HeaderMap::new();
        insert_header(&mut headers, "x-api-key", &self.api_key)?;

        Ok(SignedRequest {
            headers,
            query_params: query_params.clone(),
            body: None,
        })
    }

    fn sign_post(
        &self,
        path: &str,
        body: &mut serde_json::Value,
    ) -> Result<SignedRequest, ExchangeError> {
        let timestamp = Utc::now().timestamp_millis();
        let body_str = serde_json::to_string(body)
            .map_err(|e| ExchangeError::Internal(format!("Failed to serialize body: {}", e)))?;

        let pre_sign = format!("{}POST{}{}", timestamp, path, body_str);
        let signature = hmac_sha256_hex(&self.api_secret, &pre_sign);

        let mut headers = reqwest::header::HeaderMap::new();
        insert_header(&mut headers, "x-api-key", &self.api_key)?;
        insert_header(&mut headers, "x-timestamp", &timestamp.to_string())?;
        insert_header(&mut headers, "x-sign", &signature)?;
        insert_header(&mut headers, "content-type", "application/json")?;

        Ok(SignedRequest {
            headers,
            query_params: Vec::new(),
            body: Some(body.clone()),
        })
    }
}

// ============================================================
// Bybit Exchange
// ============================================================

/// Bybit exchange implementation.
pub struct BybitExchange {
    client: ExchangeClient,
    signer: BybitSigner,
    markets: Option<Vec<MarketInfo>>,
    market_type: MarketType,
}

impl BybitExchange {
    /// Create a new Bybit exchange instance.
    pub fn new(
        api_key: &str,
        api_secret: &str,
        proxy_url: Option<&str>,
        market_type: &MarketType,
    ) -> Result<Self, ExchangeError> {
        let base_url = "https://api.bybit.com";
        let client = ExchangeClient::new(base_url, 20, proxy_url)?;
        let signer = BybitSigner::new(api_key.to_string(), api_secret.to_string());

        Ok(Self {
            client,
            signer,
            markets: None,
            market_type: market_type.clone(),
        })
    }

    /// Convert unified symbol (e.g. "BTC/USDT") to Bybit format (e.g. "BTCUSDT").
    fn to_native_symbol(symbol: &str) -> String {
        symbol.replace('/', "").replace('-', "")
    }

    /// Convert Bybit symbol to unified format.
    fn to_unified_symbol(native: &str) -> String {
        let quotes = ["USDT", "USDC", "BTC", "ETH", "EUR", "USD"];
        for q in &quotes {
            if native.ends_with(q) {
                let base = &native[..native.len() - q.len()];
                if !base.is_empty() {
                    return format!("{}/{}", base, q);
                }
            }
        }
        native.to_string()
    }

    /// Parse Bybit order status to unified OrderStatus.
    fn parse_order_status(status: &str) -> OrderStatus {
        match status {
            "New" | "Created" | "Untriggered" => OrderStatus::Open,
            "PartiallyFilled" | "PartiallyFilledCanceled" => OrderStatus::PartiallyFilled,
            "Filled" => OrderStatus::Filled,
            "Cancelled" | "Canceled" | "Rejected" | "Deactivated" => OrderStatus::Canceled,
            _ => OrderStatus::Open,
        }
    }

    /// Convert unified Side to Bybit string.
    fn side_str(side: &Side) -> &'static str {
        match side {
            Side::Buy => "Buy",
            Side::Sell => "Sell",
        }
    }

    /// Convert unified OrderType to Bybit string.
    fn order_type_str(order_type: &OrderType) -> &'static str {
        match order_type {
            OrderType::Market => "Market",
            OrderType::Limit => "Limit",
            OrderType::StopMarket => "Market", // Bybit uses conditional orders
            OrderType::StopLimit => "Limit",
        }
    }

    /// Check Bybit response retCode.
    fn check_ret_code(data: &serde_json::Value) -> Result<(), ExchangeError> {
        let code = data.get("retCode").and_then(|c| c.as_i64()).unwrap_or(-1);
        if code != 0 {
            let msg = data.get("retMsg").and_then(|m| m.as_str()).unwrap_or("Unknown error");
            return Err(ExchangeError::exchange(code.to_string(), msg));
        }
        Ok(())
    }

    /// Return the Bybit v5 API category string based on market_type.
    fn category(&self) -> &'static str {
        match self.market_type {
            MarketType::Spot => "spot",
            MarketType::Perpetual => "linear",
        }
    }

    /// Check if this instance is configured for perpetual trading.
    fn is_perpetual(&self) -> bool {
        matches!(self.market_type, MarketType::Perpetual)
    }
}

#[async_trait]
impl Exchange for BybitExchange {
    fn id(&self) -> &str { "bybit" }
    fn name(&self) -> &str { "Bybit" }

    fn capabilities(&self) -> &ExchangeCapabilities {
        static CAPS: std::sync::OnceLock<ExchangeCapabilities> = std::sync::OnceLock::new();
        CAPS.get_or_init(|| ExchangeCapabilities {
            has: ExchangeFeatures {
                spot: true,
                futures: false,
                perpetual: true,
                fetch_ticker: true,
                fetch_tickers: true,
                fetch_order_book: true,
                fetch_ohlcv: true,
                fetch_balance: true,
                create_order: true,
                cancel_order: true,
                fetch_order: true,
                fetch_open_orders: true,
                fetch_markets: true,
            },
            rate_limit: RateLimit {
                max_requests_per_second: 10.0,
                max_requests_per_minute: Some(600.0),
            },
            timeframes: vec![
                ("1m".into(), "1".into()),
                ("3m".into(), "3".into()),
                ("5m".into(), "5".into()),
                ("15m".into(), "15".into()),
                ("30m".into(), "30".into()),
                ("1h".into(), "60".into()),
                ("2h".into(), "120".into()),
                ("4h".into(), "240".into()),
                ("6h".into(), "360".into()),
                ("12h".into(), "720".into()),
                ("1d".into(), "D".into()),
                ("1w".into(), "W".into()),
                ("1M".into(), "M".into()),
            ],
        })
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker, ExchangeError> {
        let native = Self::to_native_symbol(symbol);
        let data = self.client
            .public_get("/v5/market/tickers", &[("category", self.category()), ("symbol", native.as_str())])
            .await?;

        Self::check_ret_code(&data)?;

        let list = data.pointer("/result/list")
            .and_then(|l| l.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| {
                ExchangeError::no_data(format!("No ticker data for {} on Bybit", symbol))
            })?;

        let last = list.get("lastPrice").and_then(|v| v.as_str()).and_then(|s| s.parse().ok());
        if last.is_none() || last == Some(0.0) {
            return Err(ExchangeError::no_data(format!(
                "No valid ticker data for {} on Bybit", symbol
            )));
        }

        Ok(Ticker {
            symbol: symbol.to_string(),
            exchange: "bybit".into(),
            bid: list.get("bid1Price").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            ask: list.get("ask1Price").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            last,
            high: list.get("highPrice24h").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            low: list.get("lowPrice24h").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            volume: list.get("volume24h").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            quote_volume: list.get("turnover24h").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            open: list.get("price24hPcnt").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok())
                .map(|pct: f64| last.unwrap() / (1.0 + pct)),
            close: last,
            previous_close: None,
            price_change: None,
            price_change_pct: list.get("price24hPcnt").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok())
                .map(|pct: f64| pct * 100.0),
            timestamp: Some(Utc::now()),
            info: list.clone(),
        })
    }

    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: &str,
        limit: u32,
        since: Option<i64>,
    ) -> Result<Vec<Kline>, ExchangeError> {
        let native = Self::to_native_symbol(symbol);
        let native_tf = self.capabilities().timeframes.iter()
            .find(|(u, _)| u == timeframe)
            .map(|(_, n)| n.as_str())
            .unwrap_or("60");

        let mut params: Vec<(&str, String)> = vec![
            ("category", self.category().into()),
            ("symbol", native),
            ("interval", native_tf.to_string()),
            ("limit", limit.to_string()),
        ];
        if let Some(s) = since {
            params.push(("endTime", s.to_string()));
        }

        let data = self.client
            .public_get("/v5/market/kline", &params.iter().map(|(k, v)| (*k, v.as_str())).collect::<Vec<_>>())
            .await?;

        Self::check_ret_code(&data)?;

        let list = data.pointer("/result/list").and_then(|l| l.as_array())
            .ok_or_else(|| {
                ExchangeError::no_data(format!("No OHLCV data for {} on Bybit", symbol))
            })?;

        if list.is_empty() {
            return Err(ExchangeError::no_data(format!(
                "No OHLCV data available for {} ({}) on Bybit. Check symbol and timeframe.",
                symbol, timeframe
            )));
        }

        // Bybit returns newest first, reverse to chronological
        let mut klines: Vec<Kline> = list.iter().filter_map(|k| {
            let a = match k.as_array() {
                Some(a) if a.len() >= 6 => a,
                _ => {
                    tracing::warn!("[Bybit] Skipping malformed kline entry: {:?}", k);
                    return None;
                }
            };
            let timestamp = match a[0].as_str().and_then(|s| s.parse::<i64>().ok()) {
                Some(t) if t > 0 => t,
                _ => {
                    tracing::warn!("[Bybit] Skipping kline with invalid timestamp: {:?}", a[0]);
                    return None;
                }
            };
            let open = match a[1].as_str().and_then(|s| s.parse::<f64>().ok()) {
                Some(v) if v > 0.0 => v,
                _ => {
                    tracing::warn!("[Bybit] Skipping kline with invalid open: {:?}", a[1]);
                    return None;
                }
            };
            let high = match a[2].as_str().and_then(|s| s.parse::<f64>().ok()) {
                Some(v) if v > 0.0 => v,
                _ => {
                    tracing::warn!("[Bybit] Skipping kline with invalid high: {:?}", a[2]);
                    return None;
                }
            };
            let low = match a[3].as_str().and_then(|s| s.parse::<f64>().ok()) {
                Some(v) if v > 0.0 => v,
                _ => {
                    tracing::warn!("[Bybit] Skipping kline with invalid low: {:?}", a[3]);
                    return None;
                }
            };
            let close = match a[4].as_str().and_then(|s| s.parse::<f64>().ok()) {
                Some(v) if v > 0.0 => v,
                _ => {
                    tracing::warn!("[Bybit] Skipping kline with invalid close: {:?}", a[4]);
                    return None;
                }
            };
            let volume = match a[5].as_str().and_then(|s| s.parse::<f64>().ok()) {
                Some(v) => v,
                _ => {
                    tracing::warn!("[Bybit] Skipping kline with invalid volume: {:?}", a[5]);
                    return None;
                }
            };
            Some(Kline {
                timestamp,
                open,
                high,
                low,
                close,
                volume,
                quote_volume: a.get(6).and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                trades: None,
            })
        }).collect();

        if klines.is_empty() {
            return Err(ExchangeError::no_data(format!(
                "All kline entries invalid for {} ({}) on Bybit", symbol, timeframe
            )));
        }

        klines.reverse();
        Ok(klines)
    }

    async fn fetch_order_book(&self, symbol: &str, limit: u32) -> Result<OrderBook, ExchangeError> {
        let native = Self::to_native_symbol(symbol);
        let data = self.client
            .public_get("/v5/market/orderbook", &[
                ("category", self.category()),
                ("symbol", native.as_str()),
                ("limit", &limit.to_string()),
            ])
            .await?;

        Self::check_ret_code(&data)?;

        let result = data.pointer("/result");

        let bids: Vec<(f64, f64)> = result.and_then(|r| r.get("b")).and_then(|b| b.as_array())
            .map(|a| a.iter().filter_map(|b| {
                let ba = b.as_array()?;
                Some((ba[0].as_str()?.parse().ok()?, ba[1].as_str()?.parse().ok()?))
            }).collect())
            .unwrap_or_default();

        let asks: Vec<(f64, f64)> = result.and_then(|r| r.get("a")).and_then(|a| a.as_array())
            .map(|a| a.iter().filter_map(|a| {
                let aa = a.as_array()?;
                Some((aa[0].as_str()?.parse().ok()?, aa[1].as_str()?.parse().ok()?))
            }).collect())
            .unwrap_or_default();

        if bids.is_empty() && asks.is_empty() {
            return Err(ExchangeError::no_data(format!(
                "No order book data for {} on Bybit", symbol
            )));
        }

        Ok(OrderBook {
            symbol: symbol.to_string(),
            bids,
            asks,
            timestamp: Some(Utc::now()),
            nonce: None,
        })
    }

    async fn fetch_balance(&self) -> Result<Vec<Balance>, ExchangeError> {
        let params: Vec<(String, String)> = vec![
            ("accountType".into(), "UNIFIED".into()),
        ];

        let data = self.client
            .signed_get(&self.signer, "/v5/account/wallet-balance", params)
            .await?;

        Self::check_ret_code(&data)?;

        let accounts = data.pointer("/result/list").and_then(|l| l.as_array())
            .ok_or_else(|| ExchangeError::Internal("Invalid balance response from Bybit".into()))?;

        let mut result = Vec::new();

        for account in accounts {
            if let Some(coins) = account.get("coin").and_then(|c| c.as_array()) {
                for coin in coins {
                    let free = coin.get("availableToWithdraw").and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let used = coin.get("locked").and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    if free == 0.0 && used == 0.0 {
                        continue;
                    }
                    result.push(Balance {
                        asset: coin.get("coin").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        free,
                        used,
                        total: free + used,
                    });
                }
            }
        }

        Ok(result)
    }

    async fn fetch_markets(&self) -> Result<Vec<MarketInfo>, ExchangeError> {
        let data = self.client
            .public_get("/v5/market/instruments-info", &[("category", self.category())])
            .await?;

        Self::check_ret_code(&data)?;

        let list = data.pointer("/result/list").and_then(|l| l.as_array())
            .ok_or_else(|| ExchangeError::Internal("Invalid instruments response".into()))?;

        let target_market_type = self.market_type.clone();

        let markets: Vec<MarketInfo> = list.iter()
            .filter_map(|inst| {
                let status = inst.get("status").and_then(|v| v.as_str()).unwrap_or("");
                if status != "Trading" {
                    return None;
                }

                let base = inst.get("baseCoin").and_then(|v| v.as_str()).unwrap_or("");
                let quote = inst.get("quoteCoin").and_then(|v| v.as_str()).unwrap_or("");
                if base.is_empty() || quote.is_empty() {
                    return None;
                }

                Some(MarketInfo {
                    id: parse_str(inst, "symbol").unwrap_or_default(),
                    symbol: format!("{}/{}", base, quote),
                    base: base.to_string(),
                    quote: quote.to_string(),
                    active: true,
                    market_type: target_market_type.clone(),
                    min_amount: inst.get("lotSizeFilter").and_then(|f| f.get("minOrderQty"))
                        .and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                    max_amount: inst.get("lotSizeFilter").and_then(|f| f.get("maxOrderQty"))
                        .and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                    min_price: inst.get("priceFilter").and_then(|f| f.get("minPrice"))
                        .and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                    max_price: inst.get("priceFilter").and_then(|f| f.get("maxPrice"))
                        .and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                    min_cost: inst.get("lotSizeFilter").and_then(|f| f.get("minOrderAmt"))
                        .and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                    price_precision: inst.get("priceFilter").and_then(|f| f.get("tickSize"))
                        .and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok())
                        .map(|p| p.log10().abs().ceil() as u32),
                    amount_precision: inst.get("lotSizeFilter").and_then(|f| f.get("qtyStep"))
                        .and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok())
                        .map(|p| p.log10().abs().ceil() as u32),
                    info: inst.clone(),
                })
            })
            .collect();

        Ok(markets)
    }

    async fn create_order(&self, params: PlaceOrderParams) -> Result<Order, ExchangeError> {
        let native = Self::to_native_symbol(&params.symbol);
        let mut body = serde_json::json!({
            "category": self.category(),
            "symbol": native,
            "side": Self::side_str(&params.side),
            "orderType": Self::order_type_str(&params.order_type),
            "qty": params.amount.to_string(),
        });

        if let Some(price) = params.price {
            body["price"] = serde_json::json!(price.to_string());
        }

        if let Some(ref client_id) = params.client_order_id {
            body["orderLinkId"] = serde_json::json!(client_id);
        }

        // Perpetual: add positionIdx for hedge mode
        if self.is_perpetual() {
            let position_idx = match &params.position_side {
                Some(PositionSide::Long) => "1",
                Some(PositionSide::Short) => "2",
                None => "0", // one-way mode
            };
            body["positionIdx"] = serde_json::json!(position_idx);
        }

        let data = self.client
            .signed_post(&self.signer, "/v5/order/create", body)
            .await?;

        Self::check_ret_code(&data)?;

        let result = data.pointer("/result")
            .ok_or_else(|| ExchangeError::Internal("No order result from Bybit".into()))?;

        Ok(Order {
            id: parse_str(result, "orderId").ok_or_else(|| ExchangeError::no_data(format!("orderId missing in create_order response")))?,
            client_order_id: parse_str(result, "orderLinkId"),
            symbol: params.symbol,
            side: params.side,
            order_type: params.order_type,
            price: params.price,
            amount: params.amount,
            cost: None,
            filled: 0.0,
            remaining: params.amount,
            status: OrderStatus::Open,
            fee: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            info: result.clone(),
        })
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<Order, ExchangeError> {
        let native = Self::to_native_symbol(symbol);
        let body = serde_json::json!({
            "category": self.category(),
            "symbol": native,
            "orderId": order_id,
        });

        let data = self.client
            .signed_post(&self.signer, "/v5/order/cancel", body)
            .await?;

        Self::check_ret_code(&data)?;

        let result = data.pointer("/result")
            .ok_or_else(|| ExchangeError::Internal("No cancel result from Bybit".into()))?;

        Ok(Order {
            id: order_id.to_string(),
            client_order_id: parse_str(result, "orderLinkId"),
            symbol: symbol.to_string(),
            side: Side::Buy, // Not returned in cancel response
            order_type: OrderType::Market,
            price: None,
            amount: 0.0,
            cost: None,
            filled: 0.0,
            remaining: 0.0,
            status: OrderStatus::Canceled,
            fee: None,
            created_at: None,
            updated_at: Some(Utc::now()),
            info: result.clone(),
        })
    }

    async fn fetch_order(&self, symbol: &str, order_id: &str) -> Result<Order, ExchangeError> {
        let native = Self::to_native_symbol(symbol);
        let params = vec![
            ("category".into(), self.category().into()),
            ("symbol".into(), native),
            ("orderId".into(), order_id.to_string()),
        ];

        let data = self.client
            .signed_get(&self.signer, "/v5/order/realtime", params)
            .await?;

        Self::check_ret_code(&data)?;

        let list = data.pointer("/result/list").and_then(|l| l.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| {
                ExchangeError::OrderNotFound(format!("Order {} not found on Bybit", order_id))
            })?;

        let filled = list.get("cumExecQty").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let amount = list.get("qty").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);

        Ok(Order {
            id: parse_str(list, "orderId").ok_or_else(|| ExchangeError::no_data(format!("orderId missing in fetch_order response")))?,
            client_order_id: parse_str(list, "orderLinkId"),
            symbol: symbol.to_string(),
            side: if parse_str(list, "side").unwrap_or_default() == "Buy" { Side::Buy } else { Side::Sell },
            order_type: match parse_str(list, "orderType").unwrap_or_default().as_str() {
                "Market" => OrderType::Market,
                "Limit" => OrderType::Limit,
                _ => OrderType::Market,
            },
            price: list.get("avgPrice").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            amount,
            cost: None,
            filled,
            remaining: amount - filled,
            status: Self::parse_order_status(&parse_str(list, "orderStatus").ok_or_else(|| ExchangeError::no_data(format!("orderStatus missing in fetch_order response")))?),
            fee: None,
            created_at: None,
            updated_at: Some(Utc::now()),
            info: list.clone(),
        })
    }

    async fn fetch_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>, ExchangeError> {
        let mut params: Vec<(String, String)> = vec![("category".into(), self.category().into())];
        if let Some(sym) = symbol {
            params.push(("symbol".into(), Self::to_native_symbol(sym)));
        }

        let data = self.client
            .signed_get(&self.signer, "/v5/order/realtime", params)
            .await?;

        Self::check_ret_code(&data)?;

        let list = data.pointer("/result/list").and_then(|l| l.as_array())
            .cloned().unwrap_or_default();

        let orders: Vec<Order> = list.iter().filter_map(|o| {
            let filled = o.get("cumExecQty").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let amount = o.get("qty").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let symbol_str = parse_str(o, "symbol")?;
            let status_str = parse_str(o, "orderStatus")?;
            Some(Order {
                id: parse_str(o, "orderId")?,
                client_order_id: parse_str(o, "orderLinkId"),
                symbol: Self::to_unified_symbol(&symbol_str),
                side: if parse_str(o, "side").unwrap_or_default() == "Buy" { Side::Buy } else { Side::Sell },
                order_type: match parse_str(o, "orderType").unwrap_or_default().as_str() {
                    "Market" => OrderType::Market,
                    "Limit" => OrderType::Limit,
                    _ => OrderType::Market,
                },
                price: o.get("price").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                amount,
                cost: None,
                filled,
                remaining: amount - filled,
                status: Self::parse_order_status(&status_str),
                fee: None,
                created_at: None,
                updated_at: None,
                info: o.clone(),
            })
        }).collect();

        Ok(orders)
    }

    async fn set_leverage(&self, symbol: &str, leverage: u32, _margin_mode: MarginMode) -> Result<(), ExchangeError> {
        let native = Self::to_native_symbol(symbol);
        let body = serde_json::json!({
            "category": self.category(),
            "symbol": native,
            "buyLeverage": leverage.to_string(),
            "sellLeverage": leverage.to_string(),
        });
        let data = self.client.signed_post(&self.signer, "/v5/account/set-leverage", body).await?;
        Self::check_ret_code(&data)?;
        Ok(())
    }

    async fn fetch_positions(&self, symbol: Option<&str>) -> Result<Vec<Position>, ExchangeError> {
        let mut params: Vec<(String, String)> = vec![
            ("category".into(), self.category().into()),
        ];
        if let Some(sym) = symbol {
            params.push(("symbol".into(), Self::to_native_symbol(sym)));
        }

        let data = self.client
            .signed_get(&self.signer, "/v5/position/list", params)
            .await?;

        Self::check_ret_code(&data)?;

        let result = data.pointer("/result/list").and_then(|r| r.as_array())
            .ok_or_else(|| ExchangeError::Internal("No positions from Bybit".into()))?;

        let positions: Vec<Position> = result.iter()
            .filter_map(|p| {
                let size: f64 = parse_str(p, "size").unwrap_or_default().parse().ok()?;
                if size == 0.0 {
                    return None;
                }

                let side = match parse_str(p, "side").unwrap_or_default().as_str() {
                    "Buy" => PositionSide::Long,
                    "Sell" => PositionSide::Short,
                    _ => return None,
                };

                let leverage: u32 = parse_str(p, "leverage").unwrap_or_default().parse().unwrap_or(1);
                let margin_mode = match parse_str(p, "tradeMode").unwrap_or_default().as_str() {
                    "1" => MarginMode::Cross,
                    "0" | _ => MarginMode::Isolated,
                };

                let symbol_str = parse_str(p, "symbol").unwrap_or_default();

                Some(Position {
                    symbol: Self::to_unified_symbol(&symbol_str),
                    side,
                    size,
                    entry_price: parse_str(p, "avgPrice").unwrap_or_default().parse().unwrap_or(0.0),
                    leverage,
                    unrealized_pnl: parse_str(p, "unrealisedPnl").unwrap_or_default().parse().unwrap_or(0.0),
                    margin_mode,
                    liquidation_price: parse_str(p, "liqPrice").unwrap_or_default().parse().ok(),
                    info: p.clone(),
                })
            })
            .collect();

        Ok(positions)
    }

    async fn get_position_mode(&self) -> Result<PositionMode, ExchangeError> {
        Err(ExchangeError::NotSupported("get_position_mode not implemented for Bybit".into()))
    }

    async fn fetch_funding_rate(&self, symbol: &str) -> Result<FundingRate, ExchangeError> {
        let native = Self::to_native_symbol(symbol);
        let data = self.client
            .public_get("/v5/market/tickers", &[
                ("category", "linear"),
                ("symbol", native.as_str()),
            ])
            .await?;

        Self::check_ret_code(&data)?;

        let result = data.pointer("/result/list").and_then(|r| r.as_array())
            .ok_or_else(|| ExchangeError::Internal("No ticker from Bybit".into()))?;

        let ticker = result.first()
            .ok_or_else(|| ExchangeError::no_data(format!("No funding rate data from Bybit")))?;

        let rate = parse_f64(ticker, "fundingRate").unwrap_or(0.0);
        let next_time = parse_i64(ticker, "nextFundingTime").unwrap_or(0);
        let next_time = if next_time > 0 {
            DateTime::from_timestamp_millis(next_time)
        } else {
            None
        };

        Ok(FundingRate {
            symbol: symbol.to_string(),
            rate,
            next_funding_time: next_time,
            info: ticker.clone(),
        })
    }

    async fn fetch_funding_history(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<FundingHistoryEntry>, ExchangeError> {
        let native = Self::to_native_symbol(symbol);
        let mut all_entries = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let mut params: Vec<(&str, String)> = vec![
                ("category", "linear".to_string()),
                ("symbol", native.to_string()),
                ("startTime", start_time.to_string()),
                ("endTime", end_time.to_string()),
                ("limit", "200".to_string()),
            ];
            if let Some(c) = &cursor {
                params.push(("cursor", c.clone()));
            }

            let data = self.client
                .public_get("/v5/market/funding-rate-history", &params.iter().map(|(k, v)| (*k, v.as_str())).collect::<Vec<_>>())
                .await?;

            Self::check_ret_code(&data)?;

            let list = data.pointer("/result/list").and_then(|r| r.as_array())
                .cloned()
                .unwrap_or_default();

            if list.is_empty() {
                break;
            }

            for item in &list {
                let funding_time = parse_i64(item, "fundingTime").unwrap_or(0);
                let rate = parse_f64(item, "fundingRate").unwrap_or(0.0);
                all_entries.push(FundingHistoryEntry { funding_time, rate });
            }

            cursor = data.pointer("/result/nextPageCursor")
                .and_then(|c| c.as_str())
                .filter(|c| !c.is_empty())
                .map(|c| c.to_string());

            if cursor.is_none() {
                break;
            }
        }

        Ok(all_entries)
    }

    async fn ping(&self) -> Result<bool, ExchangeError> {
        let data = self.client.public_get("/v5/market/time", &[]).await?;
        Ok(data.get("retCode").and_then(|c| c.as_i64()) == Some(0))
    }

    async fn load_markets(&mut self) -> Result<(), ExchangeError> {
        info!("Loading Bybit markets...");
        self.markets = Some(self.fetch_markets().await?);
        info!("Loaded {} Bybit markets", self.markets.as_ref().unwrap().len());
        Ok(())
    }

    fn markets(&self) -> &Option<Vec<MarketInfo>> {
        &self.markets
    }
}
