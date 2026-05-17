//! OKX exchange implementation.
//!
//! Implements the full CCXT-style Exchange trait for OKX:
//! - REST API: https://www.okx.com
//! - Demo: https://www.okx.com (with x-simulated-trading header)
//! - Auth: HMAC-SHA256 + Base64 signature, timestamp, passphrase
//! - Rate limit: 20 req/2s (public), 20 req/2s (private)
//!
//! Supported features:
//! - Spot trading
//! - Perpetual (SWAP) trading
//! - Ticker, OHLCV, OrderBook, Balance
//! - Create/Cancel/Fetch orders
//! - Set leverage, fetch positions, fetch funding rate

use async_trait::async_trait;
use chrono::Utc;
use tracing::info;

use crate::trading::ccxt::types::*;
use crate::trading::ccxt::errors::ExchangeError;
use crate::trading::ccxt::auth::{Signer, SignedRequest, hmac_sha256_base64, insert_header};
use crate::trading::ccxt::{Exchange, ExchangeClient, parse_str, parse_f64, parse_i64, parse_u32};

// ============================================================
// OKX Signer (HMAC-SHA256 + Base64, timestamp + passphrase)
// ============================================================

/// OKX-specific request signer.
pub struct OkxSigner {
    api_key: String,
    api_secret: String,
    passphrase: String,
}

impl OkxSigner {
    pub fn new(api_key: String, api_secret: String, passphrase: String) -> Self {
        Self { api_key, api_secret, passphrase }
    }
}

impl Signer for OkxSigner {
    fn sign_get(
        &self,
        path: &str,
        query_params: &mut Vec<(String, String)>,
    ) -> Result<SignedRequest, ExchangeError> {
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        let query_string = query_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let sign_str = format!("{}GET{}{}", timestamp, path, query_string);
        let signature = hmac_sha256_base64(&self.api_secret, &sign_str);

        let mut headers = reqwest::header::HeaderMap::new();
        insert_header(&mut headers, "ok-access-key", &self.api_key)?;
        insert_header(&mut headers, "ok-access-sign", &signature)?;
        insert_header(&mut headers, "ok-access-timestamp", &timestamp)?;
        insert_header(&mut headers, "ok-access-passphrase", &self.passphrase)?;

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
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let body_str = serde_json::to_string(body)
            .map_err(|e| ExchangeError::Internal(format!("Failed to serialize body: {}", e)))?;

        let sign_str = format!("{}POST{}{}", timestamp, path, body_str);
        let signature = hmac_sha256_base64(&self.api_secret, &sign_str);

        let mut headers = reqwest::header::HeaderMap::new();
        insert_header(&mut headers, "ok-access-key", &self.api_key)?;
        insert_header(&mut headers, "ok-access-sign", &signature)?;
        insert_header(&mut headers, "ok-access-timestamp", &timestamp)?;
        insert_header(&mut headers, "ok-access-passphrase", &self.passphrase)?;
        insert_header(&mut headers, "content-type", "application/json")?;

        Ok(SignedRequest {
            headers,
            query_params: Vec::new(),
            body: Some(body.clone()),
        })
    }
}

// ============================================================
// OKX Exchange
// ============================================================

/// OKX exchange implementation.
pub struct OkxExchange {
    client: ExchangeClient,
    signer: OkxSigner,
    markets: Option<Vec<MarketInfo>>,
    market_type: MarketType,
}

impl OkxExchange {
    /// Create a new OKX exchange instance.
    pub fn new(
        api_key: &str,
        api_secret: &str,
        passphrase: &str,
        proxy_url: Option<&str>,
        market_type: &MarketType,
    ) -> Result<Self, ExchangeError> {
        let base_url = "https://www.okx.com";
        let client = ExchangeClient::new(base_url, 20, proxy_url)?;
        let signer = OkxSigner::new(
            api_key.to_string(),
            api_secret.to_string(),
            passphrase.to_string(),
        );

        Ok(Self {
            client,
            signer,
            markets: None,
            market_type: market_type.clone(),
        })
    }

    /// Convert unified symbol (e.g. "BTC/USDT") to OKX format (e.g. "BTC-USDT").
    fn to_native_symbol(symbol: &str) -> String {
        symbol.replace('/', "-")
    }

    /// Convert unified symbol to OKX format, accounting for market type.
    /// Spot: BTC/USDT -> BTC-USDT
    /// Perpetual: BTC/USDT -> BTC-USDT-SWAP
    fn to_native_symbol_with_type(&self, symbol: &str) -> String {
        let base = Self::to_native_symbol(symbol);
        if self.is_perpetual() {
            format!("{}-SWAP", base)
        } else {
            base
        }
    }

    /// Convert OKX symbol to unified format.
    fn to_unified_symbol(native: &str) -> String {
        // Handle SWAP suffix: BTC-USDT-SWAP -> BTC/USDT
        let stripped = native.strip_suffix("-SWAP").unwrap_or(native);
        stripped.replace('-', "/")
    }

    /// Convert unified timeframe to OKX bar format.
    fn to_native_timeframe(timeframe: &str) -> &'static str {
        match timeframe {
            "1m" => "1m",
            "3m" => "3m",
            "5m" => "5m",
            "15m" => "15m",
            "30m" => "30m",
            "1h" => "1H",
            "2h" => "2H",
            "4h" => "4H",
            "6h" => "6H",
            "12h" => "12H",
            "1d" => "1D",
            "1w" => "1W",
            "1M" => "1M",
            _ => "1H",
        }
    }

    /// Parse OKX order status to unified OrderStatus.
    fn parse_order_status(status: &str) -> OrderStatus {
        match status {
            "live" | "open" => OrderStatus::Open,
            "partially_filled" => OrderStatus::PartiallyFilled,
            "filled" => OrderStatus::Filled,
            "canceled" | "cancelled" => OrderStatus::Canceled,
            "mmp_canceled" => OrderStatus::Canceled,
            _ => OrderStatus::Open,
        }
    }

    /// Convert unified Side to OKX string.
    fn side_str(side: &Side) -> &'static str {
        match side {
            Side::Buy => "buy",
            Side::Sell => "sell",
        }
    }

    /// Convert unified OrderType to OKX string.
    fn order_type_str(order_type: &OrderType) -> &'static str {
        match order_type {
            OrderType::Market => "market",
            OrderType::Limit => "limit",
            OrderType::StopMarket => "market", // OKX uses trigger orders
            OrderType::StopLimit => "limit",
        }
    }

    /// Extract the first element from OKX's "data" array response.
    fn extract_first_data(data: &serde_json::Value) -> Option<&serde_json::Value> {
        data.get("data")
            .and_then(|d| d.as_array())
            .and_then(|a| a.first())
    }

    /// Extract the "data" array from OKX response.
    fn extract_data_array(data: &serde_json::Value) -> Option<&Vec<serde_json::Value>> {
        data.get("data").and_then(|d| d.as_array())
    }

    /// Check OKX response code for errors.
    fn check_okx_code(data: &serde_json::Value) -> Result<(), ExchangeError> {
        let code = data.get("code").and_then(|c| c.as_str()).unwrap_or("0");
        if code != "0" {
            let msg = data.get("msg").and_then(|m| m.as_str()).unwrap_or("Unknown error");
            return Err(ExchangeError::exchange(code, msg));
        }
        Ok(())
    }

    /// Return the OKX instrument type string for the current market type.
    fn inst_type(&self) -> &'static str {
        match self.market_type {
            MarketType::Spot => "SPOT",
            MarketType::Perpetual => "SWAP",
        }
    }

    /// Return the OKX trade mode string for the current market type.
    fn td_mode(&self) -> &'static str {
        match self.market_type {
            MarketType::Spot => "cash",
            MarketType::Perpetual => "cross",
        }
    }

    /// Check if the current market type is perpetual.
    fn is_perpetual(&self) -> bool {
        self.market_type == MarketType::Perpetual
    }
}

#[async_trait]
impl Exchange for OkxExchange {
    fn id(&self) -> &str { "okx" }
    fn name(&self) -> &str { "OKX" }

    fn capabilities(&self) -> &ExchangeCapabilities {
        static CAPS: std::sync::OnceLock<ExchangeCapabilities> = std::sync::OnceLock::new();
        CAPS.get_or_init(|| ExchangeCapabilities {
            has: ExchangeFeatures {
                spot: true,
                futures: true,
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
                ("1m".into(), "1m".into()),
                ("3m".into(), "3m".into()),
                ("5m".into(), "5m".into()),
                ("15m".into(), "15m".into()),
                ("30m".into(), "30m".into()),
                ("1h".into(), "1H".into()),
                ("2h".into(), "2H".into()),
                ("4h".into(), "4H".into()),
                ("6h".into(), "6H".into()),
                ("12h".into(), "12H".into()),
                ("1d".into(), "1D".into()),
                ("1w".into(), "1W".into()),
                ("1M".into(), "1M".into()),
            ],
        })
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker, ExchangeError> {
        let inst_id = self.to_native_symbol_with_type(symbol);
        let data = self.client
            .public_get("/api/v5/market/ticker", &[("instId", inst_id.as_str())])
            .await?;

        Self::check_okx_code(&data)?;

        let ticker = Self::extract_first_data(&data).ok_or_else(|| {
            ExchangeError::no_data(format!("No ticker data for {} on OKX", symbol))
        })?;

        let last = ticker.get("last").and_then(|v| v.as_str()).and_then(|s| s.parse().ok());
        if last.is_none() || last == Some(0.0) {
            return Err(ExchangeError::no_data(format!(
                "No valid ticker data for {} on OKX", symbol
            )));
        }

        Ok(Ticker {
            symbol: symbol.to_string(),
            exchange: "okx".into(),
            bid: ticker.get("bidPx").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            ask: ticker.get("askPx").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            last,
            high: ticker.get("high24h").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            low: ticker.get("low24h").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            volume: ticker.get("vol24h").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            quote_volume: ticker.get("volCcy24h").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            open: ticker.get("open24h").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            close: last,
            previous_close: None,
            price_change: None,
            price_change_pct: None,
            timestamp: Some(Utc::now()),
            info: ticker.clone(),
        })
    }

    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: &str,
        limit: u32,
        since: Option<i64>,
    ) -> Result<Vec<Kline>, ExchangeError> {
        let inst_id = self.to_native_symbol_with_type(symbol);
        let bar = Self::to_native_timeframe(timeframe);

        let mut params: Vec<(&str, String)> = vec![
            ("instId", inst_id),
            ("bar", bar.to_string()),
            ("limit", limit.to_string()),
        ];
        if let Some(s) = since {
            params.push(("after", s.to_string()));
        }

        let data = self.client
            .public_get("/api/v5/market/candles", &params.iter().map(|(k, v)| (*k, v.as_str())).collect::<Vec<_>>())
            .await?;

        Self::check_okx_code(&data)?;

        let arr = Self::extract_data_array(&data).ok_or_else(|| {
            ExchangeError::no_data(format!("No OHLCV data for {} on OKX", symbol))
        })?;

        if arr.is_empty() {
            return Err(ExchangeError::no_data(format!(
                "No OHLCV data available for {} ({}) on OKX. Check symbol and timeframe.",
                symbol, timeframe
            )));
        }

        // OKX returns candles in reverse chronological order
        let mut klines: Vec<Kline> = arr.iter().filter_map(|k| {
            let a = match k.as_array() {
                Some(a) if a.len() >= 6 => a,
                _ => {
                    tracing::warn!("[OKX] Skipping malformed kline entry: {:?}", k);
                    return None;
                }
            };
            let timestamp = match a[0].as_str().and_then(|s| s.parse::<i64>().ok()) {
                Some(t) if t > 0 => t,
                _ => {
                    tracing::warn!("[OKX] Skipping kline with invalid timestamp: {:?}", a[0]);
                    return None;
                }
            };
            let open = match a[1].as_str().and_then(|s| s.parse::<f64>().ok()) {
                Some(v) if v > 0.0 => v,
                _ => {
                    tracing::warn!("[OKX] Skipping kline with invalid open: {:?}", a[1]);
                    return None;
                }
            };
            let high = match a[2].as_str().and_then(|s| s.parse::<f64>().ok()) {
                Some(v) if v > 0.0 => v,
                _ => {
                    tracing::warn!("[OKX] Skipping kline with invalid high: {:?}", a[2]);
                    return None;
                }
            };
            let low = match a[3].as_str().and_then(|s| s.parse::<f64>().ok()) {
                Some(v) if v > 0.0 => v,
                _ => {
                    tracing::warn!("[OKX] Skipping kline with invalid low: {:?}", a[3]);
                    return None;
                }
            };
            let close = match a[4].as_str().and_then(|s| s.parse::<f64>().ok()) {
                Some(v) if v > 0.0 => v,
                _ => {
                    tracing::warn!("[OKX] Skipping kline with invalid close: {:?}", a[4]);
                    return None;
                }
            };
            let volume = match a[5].as_str().and_then(|s| s.parse::<f64>().ok()) {
                Some(v) => v,
                _ => {
                    tracing::warn!("[OKX] Skipping kline with invalid volume: {:?}", a[5]);
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
                "All kline entries invalid for {} ({}) on OKX", symbol, timeframe
            )));
        }

        klines.reverse();
        Ok(klines)
    }

    async fn fetch_order_book(&self, symbol: &str, limit: u32) -> Result<OrderBook, ExchangeError> {
        let inst_id = self.to_native_symbol_with_type(symbol);
        let data = self.client
            .public_get("/api/v5/market/books", &[("instId", inst_id.as_str()), ("sz", &limit.to_string())])
            .await?;

        Self::check_okx_code(&data)?;

        let ob = Self::extract_first_data(&data).ok_or_else(|| {
            ExchangeError::no_data(format!("No order book data for {} on OKX", symbol))
        })?;

        let bids: Vec<(f64, f64)> = ob.get("bids").and_then(|b| b.as_array())
            .map(|a| a.iter().filter_map(|b| {
                let ba = b.as_array()?;
                Some((ba[0].as_str()?.parse().ok()?, ba[1].as_str()?.parse().ok()?))
            }).collect())
            .unwrap_or_default();

        let asks: Vec<(f64, f64)> = ob.get("asks").and_then(|a| a.as_array())
            .map(|a| a.iter().filter_map(|a| {
                let aa = a.as_array()?;
                Some((aa[0].as_str()?.parse().ok()?, aa[1].as_str()?.parse().ok()?))
            }).collect())
            .unwrap_or_default();

        if bids.is_empty() && asks.is_empty() {
            return Err(ExchangeError::no_data(format!(
                "No order book data for {} on OKX", symbol
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
        let params: Vec<(String, String)> = vec![];
        let data = self.client
            .signed_get(&self.signer, "/api/v5/account/balance", params)
            .await?;

        Self::check_okx_code(&data)?;

        let details = data.pointer("/data/0/details").and_then(|d| d.as_array())
            .ok_or_else(|| ExchangeError::Internal("Invalid balance response from OKX".into()))?;

        let result: Vec<Balance> = details.iter()
            .filter_map(|d| {
                let free = d.get("availBal").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let used = d.get("frozenBal").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                if free == 0.0 && used == 0.0 {
                    return None;
                }
                Some(Balance {
                    asset: d.get("ccy").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    free,
                    used,
                    total: free + used,
                })
            })
            .collect();

        Ok(result)
    }

    async fn fetch_markets(&self) -> Result<Vec<MarketInfo>, ExchangeError> {
        let data = self.client
            .public_get("/api/v5/public/instruments", &[("instType", self.inst_type())])
            .await?;

        Self::check_okx_code(&data)?;

        let arr = Self::extract_data_array(&data)
            .ok_or_else(|| ExchangeError::Internal("Invalid instruments response".into()))?;

        let mt = if self.is_perpetual() {
            MarketType::Perpetual
        } else {
            MarketType::Spot
        };

        let markets: Vec<MarketInfo> = arr.iter()
            .filter_map(|inst| {
                let state = inst.get("state").and_then(|v| v.as_str()).unwrap_or("");
                if state != "live" {
                    return None;
                }

                let inst_id = inst.get("instId").and_then(|v| v.as_str()).unwrap_or("");
                let base_ccy = inst.get("baseCcy").and_then(|v| v.as_str()).unwrap_or("");
                let quote_ccy = inst.get("quoteCcy").and_then(|v| v.as_str()).unwrap_or("");
                if base_ccy.is_empty() || quote_ccy.is_empty() {
                    return None;
                }

                Some(MarketInfo {
                    id: inst_id.to_string(),
                    symbol: format!("{}/{}", base_ccy, quote_ccy),
                    base: base_ccy.to_string(),
                    quote: quote_ccy.to_string(),
                    active: true,
                    market_type: mt.clone(),
                    min_amount: inst.get("minSz").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                    max_amount: inst.get("maxSz").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                    min_price: inst.get("tickSz").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                    max_price: None,
                    min_cost: inst.get("minSz").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                    price_precision: inst.get("tickSz").and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                        .map(|p| p.log10().abs().ceil() as u32),
                    amount_precision: inst.get("lotSz").and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                        .map(|p| p.log10().abs().ceil() as u32),
                    info: inst.clone(),
                })
            })
            .collect();

        Ok(markets)
    }

    async fn create_order(&self, params: PlaceOrderParams) -> Result<Order, ExchangeError> {
        let inst_id = self.to_native_symbol_with_type(&params.symbol);
        let mut body = serde_json::json!({
            "instId": inst_id,
            "tdMode": self.td_mode(),
            "side": Self::side_str(&params.side),
            "ordType": Self::order_type_str(&params.order_type),
            "sz": params.amount.to_string(),
        });

        if let Some(price) = params.price {
            body["px"] = serde_json::json!(price.to_string());
        }

        if let Some(ref client_id) = params.client_order_id {
            body["clOrdId"] = serde_json::json!(client_id);
        }

        // Perpetual: add position side (hedge mode support)
        if self.is_perpetual() {
            let pos_side = match &params.position_side {
                Some(PositionSide::Long) => "long",
                Some(PositionSide::Short) => "short",
                None => "net", // one-way mode
            };
            body["posSide"] = serde_json::json!(pos_side);
        }

        let data = self.client
            .signed_post(&self.signer, "/api/v5/trade/order", body)
            .await?;

        Self::check_okx_code(&data)?;

        let order_data = Self::extract_first_data(&data)
            .ok_or_else(|| ExchangeError::Internal("No order data in OKX response".into()))?;

        Ok(Order {
            id: parse_str(order_data, "ordId").ok_or_else(|| ExchangeError::no_data(format!("ordId missing in create_order response")))?,
            client_order_id: parse_str(order_data, "clOrdId"),
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
            info: order_data.clone(),
        })
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<Order, ExchangeError> {
        let inst_id = self.to_native_symbol_with_type(symbol);
        let body = serde_json::json!({
            "instId": inst_id,
            "ordId": order_id,
        });

        let data = self.client
            .signed_post(&self.signer, "/api/v5/trade/cancel-order", body)
            .await?;

        Self::check_okx_code(&data)?;

        let order_data = Self::extract_first_data(&data)
            .ok_or_else(|| ExchangeError::Internal("No cancel data in OKX response".into()))?;

        Ok(Order {
            id: order_id.to_string(),
            client_order_id: parse_str(order_data, "clOrdId"),
            symbol: symbol.to_string(),
            side: Side::Buy, // Not reliably returned in cancel response
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
            info: order_data.clone(),
        })
    }

    async fn fetch_order(&self, symbol: &str, order_id: &str) -> Result<Order, ExchangeError> {
        let inst_id = self.to_native_symbol_with_type(symbol);
        let params = vec![
            ("instId".into(), inst_id),
            ("ordId".into(), order_id.to_string()),
        ];

        let data = self.client
            .signed_get(&self.signer, "/api/v5/trade/order", params)
            .await?;

        Self::check_okx_code(&data)?;

        let order_data = Self::extract_first_data(&data).ok_or_else(|| {
            ExchangeError::OrderNotFound(format!("Order {} not found on OKX", order_id))
        })?;

        let filled = order_data.get("fillSz").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let amount = order_data.get("sz").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);

        Ok(Order {
            id: parse_str(order_data, "ordId").ok_or_else(|| ExchangeError::no_data(format!("ordId missing in fetch_order response")))?,
            client_order_id: parse_str(order_data, "clOrdId"),
            symbol: symbol.to_string(),
            side: if parse_str(order_data, "side").unwrap_or_default() == "buy" { Side::Buy } else { Side::Sell },
            order_type: match parse_str(order_data, "ordType").unwrap_or_default().as_str() {
                "market" => OrderType::Market,
                "limit" => OrderType::Limit,
                _ => OrderType::Market,
            },
            price: order_data.get("avgPx").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            amount,
            cost: None,
            filled,
            remaining: amount - filled,
            status: Self::parse_order_status(&parse_str(order_data, "state").ok_or_else(|| ExchangeError::no_data(format!("state missing in fetch_order response")))?),
            fee: None,
            created_at: None,
            updated_at: Some(Utc::now()),
            info: order_data.clone(),
        })
    }

    async fn fetch_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>, ExchangeError> {
        let mut params: Vec<(String, String)> = vec![("instType".into(), self.inst_type().into())];
        if let Some(sym) = symbol {
            params.push(("instId".into(), self.to_native_symbol_with_type(sym)));
        }

        let data = self.client
            .signed_get(&self.signer, "/api/v5/trade/orders-pending", params)
            .await?;

        Self::check_okx_code(&data)?;

        let arr = Self::extract_data_array(&data).cloned().unwrap_or_default();
        let orders: Vec<Order> = arr.iter().filter_map(|o| {
            let filled = o.get("fillSz").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let amount = o.get("sz").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let inst_id = parse_str(o, "instId")?;
            let state_str = parse_str(o, "state")?;
            Some(Order {
                id: parse_str(o, "ordId")?,
                client_order_id: parse_str(o, "clOrdId"),
                symbol: Self::to_unified_symbol(&inst_id),
                side: if parse_str(o, "side").unwrap_or_default() == "buy" { Side::Buy } else { Side::Sell },
                order_type: match parse_str(o, "ordType").unwrap_or_default().as_str() {
                    "market" => OrderType::Market,
                    "limit" => OrderType::Limit,
                    _ => OrderType::Market,
                },
                price: o.get("px").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                amount,
                cost: None,
                filled,
                remaining: amount - filled,
                status: Self::parse_order_status(&state_str),
                fee: None,
                created_at: None,
                updated_at: None,
                info: o.clone(),
            })
        }).collect();

        Ok(orders)
    }

    async fn set_leverage(
        &self,
        symbol: &str,
        leverage: u32,
        margin_mode: MarginMode,
    ) -> Result<(), ExchangeError> {
        let inst_id = self.to_native_symbol_with_type(symbol);
        let mgn_mode = match margin_mode {
            MarginMode::Cross => "cross",
            MarginMode::Isolated => "isolated",
        };

        let body = serde_json::json!({
            "instId": inst_id,
            "lever": leverage.to_string(),
            "mgnMode": mgn_mode,
            "posSide": "net",
        });

        let data = self.client
            .signed_post(&self.signer, "/api/v5/account/set-leverage", body)
            .await?;

        Self::check_okx_code(&data)?;
        Ok(())
    }

    async fn fetch_positions(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<Position>, ExchangeError> {
        let mut params: Vec<(String, String)> = vec![("instType".to_string(), "SWAP".to_string())];
        if let Some(sym) = symbol {
            params.push(("instId".to_string(), self.to_native_symbol_with_type(sym)));
        }

        let data = self.client
            .signed_get(&self.signer, "/api/v5/account/positions", params)
            .await?;

        Self::check_okx_code(&data)?;

        let positions = Self::extract_data_array(&data)
            .cloned()
            .unwrap_or_default();

        let result: Vec<Position> = positions.iter()
            .filter_map(|p| {
                // Only include positions with non-zero size
                let pos_size = parse_f64(p, "pos").unwrap_or(0.0);
                if pos_size == 0.0 {
                    return None;
                }

                let side = match parse_str(p, "posSide").unwrap_or_default().as_str() {
                    "long" => PositionSide::Long,
                    "short" => PositionSide::Short,
                    _ => {
                        if pos_size > 0.0 {
                            PositionSide::Long
                        } else {
                            PositionSide::Short
                        }
                    }
                };

                let margin_mode = match parse_str(p, "mgnMode").unwrap_or_default().as_str() {
                    "isolated" => MarginMode::Isolated,
                    _ => MarginMode::Cross,
                };

                let inst_id = parse_str(p, "instId").unwrap_or_default();
                let unified_symbol = Self::to_unified_symbol(&inst_id);

                Some(Position {
                    symbol: unified_symbol,
                    side,
                    size: pos_size.abs(),
                    entry_price: parse_f64(p, "avgPx").unwrap_or(0.0),
                    leverage: parse_u32(p, "lever").unwrap_or(1),
                    unrealized_pnl: parse_f64(p, "upl").unwrap_or(0.0),
                    margin_mode,
                    liquidation_price: parse_f64(p, "liqPx"),
                    info: p.clone(),
                })
            })
            .collect();

        Ok(result)
    }

    async fn get_position_mode(&self) -> Result<PositionMode, ExchangeError> {
        Err(ExchangeError::NotSupported("get_position_mode not implemented for OKX".into()))
    }

    async fn fetch_funding_rate(
        &self,
        symbol: &str,
    ) -> Result<FundingRate, ExchangeError> {
        let inst_id = self.to_native_symbol_with_type(symbol);
        let data = self.client
            .public_get("/api/v5/public/funding-rate", &[("instId", inst_id.as_str())])
            .await?;

        Self::check_okx_code(&data)?;

        let rates = Self::extract_data_array(&data)
            .cloned()
            .unwrap_or_default();

        let latest = rates.first().ok_or_else(|| {
            ExchangeError::no_data(format!("No funding rate data for {} on OKX", symbol))
        })?;

        let rate = parse_f64(latest, "fundingRate").unwrap_or(0.0);
        let next_time = parse_i64(latest, "fundingTime").unwrap_or(0);
        let next_funding_time = if next_time > 0 {
            chrono::DateTime::from_timestamp_millis(next_time)
        } else {
            None
        };

        Ok(FundingRate {
            symbol: symbol.to_string(),
            rate,
            next_funding_time,
            info: latest.clone(),
        })
    }

    async fn fetch_funding_history(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<FundingHistoryEntry>, ExchangeError> {
        let inst_id = self.to_native_symbol_with_type(symbol);
        let mut all_entries = Vec::new();
        // OKX uses 'after' as pagination cursor (older records), initial request uses before
        let mut after: Option<String> = None;

        loop {
            let mut params: Vec<(&str, String)> = vec![
                ("instId", inst_id.to_string()),
                ("limit", "100".to_string()),
            ];
            if let Some(a) = &after {
                params.push(("after", a.clone()));
            } else {
                // First request: use end_time as 'before' to get records before end_time
                params.push(("before", (end_time / 1000).to_string()));
            }

            let data = self.client
                .public_get("/api/v5/public/funding-rate-history", &params.iter().map(|(k, v)| (*k, v.as_str())).collect::<Vec<_>>())
                .await?;

            Self::check_okx_code(&data)?;

            let rates = Self::extract_data_array(&data)
                .cloned()
                .unwrap_or_default();

            if rates.is_empty() {
                break;
            }

            for item in &rates {
                let funding_time = parse_i64(item, "fundingTime").unwrap_or(0);
                let rate = parse_f64(item, "fundingRate").unwrap_or(0.0);
                if funding_time >= start_time {
                    all_entries.push(FundingHistoryEntry { funding_time, rate });
                }
            }

            if let Some(earliest) = rates.last() {
                let earliest_time = parse_i64(earliest, "fundingTime").unwrap_or(0);
                if earliest_time <= start_time {
                    break;
                }
                after = Some(earliest_time.to_string());
            } else {
                break;
            }
        }

        all_entries.sort_by_key(|e| e.funding_time);
        Ok(all_entries)
    }

    async fn ping(&self) -> Result<bool, ExchangeError> {
        let data = self.client.public_get("/api/v5/public/time", &[]).await?;
        Ok(data.get("code").and_then(|c| c.as_str()) == Some("0"))
    }

    async fn load_markets(&mut self) -> Result<(), ExchangeError> {
        info!("Loading OKX markets...");
        self.markets = Some(self.fetch_markets().await?);
        info!("Loaded {} OKX markets", self.markets.as_ref().unwrap().len());
        Ok(())
    }

    fn markets(&self) -> &Option<Vec<MarketInfo>> {
        &self.markets
    }
}
