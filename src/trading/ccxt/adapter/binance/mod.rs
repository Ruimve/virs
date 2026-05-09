//! Binance exchange implementation.
//!
//! Implements the full CCXT-style Exchange trait for Binance:
//! - Spot REST API: https://api.binance.com
//! - USDT-M Futures API: https://fapi.binance.com
//! - Testnet: https://testnet.binance.vision (spot), https://testnet.binancefuture.com (futures)
//! - Auth: HMAC-SHA256 via query string (GET) or form body (POST)
//! - Rate limit: 1200 req/min (spot), 2400 req/min (futures)
//!
//! Supported features:
//! - Spot trading
//! - USDT-M Perpetual futures trading
//! - Ticker, OHLCV, OrderBook, Balance
//! - Create/Cancel/Fetch orders
//! - Leverage, Positions, Funding rate (perpetual)
//! - WebSocket: K-line stream, Order updates

pub mod kline_ws;
pub mod order_ws;

use async_trait::async_trait;
use chrono::Utc;
use tracing::info;

use crate::trading::ccxt::types::*;
use crate::trading::ccxt::errors::ExchangeError;
use crate::trading::ccxt::auth::{Signer, SignedRequest, hmac_sha256_hex, insert_header};
use crate::trading::ccxt::{Exchange, ExchangeClient, parse_f64, parse_str, parse_str_opt, parse_u32};

// ============================================================
// Binance Signer (HMAC-SHA256 via query string)
// ============================================================

/// Binance-specific request signer.
pub struct BinanceSigner {
    api_key: String,
    api_secret: String,
}

impl BinanceSigner {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self { api_key, api_secret }
    }
}

impl Signer for BinanceSigner {
    fn sign_get(
        &self,
        _path: &str,
        query_params: &mut Vec<(String, String)>,
    ) -> Result<SignedRequest, ExchangeError> {
        let timestamp = Utc::now().timestamp_millis();
        query_params.push(("timestamp".into(), timestamp.to_string()));

        let query_string = query_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let signature = hmac_sha256_hex(&self.api_secret, &query_string);
        query_params.push(("signature".into(), signature));

        let mut headers = reqwest::header::HeaderMap::new();
        insert_header(&mut headers, "x-mbx-apikey", &self.api_key)?;

        Ok(SignedRequest {
            headers,
            query_params: query_params.clone(),
            body: None,
        })
    }

    fn sign_post(
        &self,
        _path: &str,
        body: &mut serde_json::Value,
    ) -> Result<SignedRequest, ExchangeError> {
        let mut query_params = vec![(
            "timestamp".into(),
            Utc::now().timestamp_millis().to_string(),
        )];

        let form_body = if body.is_object() {
            let mut pairs: Vec<(String, String)> = body
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| {
                    let val = if v.is_string() {
                        v.as_str().unwrap().to_string()
                    } else {
                        v.to_string()
                    };
                    (k.clone(), val)
                })
                .collect();
            pairs.push(("timestamp".into(), query_params[0].1.clone()));

            let query_string = pairs
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");

            let signature = hmac_sha256_hex(&self.api_secret, &query_string);
            pairs.push(("signature".into(), signature));

            query_params = pairs;
            Some(serde_json::Value::String(query_string))
        } else {
            None
        };

        let mut headers = reqwest::header::HeaderMap::new();
        insert_header(&mut headers, "x-mbx-apikey", &self.api_key)?;

        Ok(SignedRequest {
            headers,
            query_params,
            body: form_body,
        })
    }
}

// ============================================================
// Binance Exchange
// ============================================================

/// Binance exchange implementation.
pub struct BinanceExchange {
    client: ExchangeClient,
    signer: BinanceSigner,
    markets: Option<Vec<MarketInfo>>,
    testnet: bool,
    market_type: MarketType,
}

impl BinanceExchange {
    /// Create a new Binance exchange instance.
    pub fn new(
        api_key: &str,
        api_secret: &str,
        proxy_url: Option<&str>,
        market_type: &MarketType,
    ) -> Result<Self, ExchangeError> {
        let (base_url, max_concurrent) = match market_type {
            MarketType::Spot => ("https://api.binance.com", 20),
            MarketType::Perpetual => ("https://fapi.binance.com", 40),
        };
        let client = ExchangeClient::new(base_url, max_concurrent, proxy_url)?;
        let signer = BinanceSigner::new(api_key.to_string(), api_secret.to_string());

        Ok(Self {
            client,
            signer,
            markets: None,
            testnet: false,
            market_type: market_type.clone(),
        })
    }

    /// Convert unified symbol (e.g. "BTC/USDT") to Binance format (e.g. "BTCUSDT").
    fn to_native_symbol(symbol: &str) -> String {
        symbol.replace('/', "").replace('-', "")
    }

    /// Convert Binance symbol to unified format.
    fn to_unified_symbol(native: &str) -> String {
        // Common quote currencies for detection
        let quotes = [
            "USDT", "USDC", "BUSD", "BTC", "ETH", "BNB", "EUR", "GBP", "TRY", "BRL", "ARS",
        ];
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

    /// Parse Binance order status string to unified OrderStatus.
    fn parse_order_status(status: &str) -> OrderStatus {
        match status {
            "NEW" => OrderStatus::Open,
            "PARTIALLY_FILLED" => OrderStatus::PartiallyFilled,
            "FILLED" => OrderStatus::Filled,
            "CANCELED" | "CANCELLED" | "EXPIRED" => OrderStatus::Canceled,
            "REJECTED" => OrderStatus::Rejected,
            "PENDING_CANCEL" => OrderStatus::Open,
            _ => OrderStatus::Open,
        }
    }

    /// Parse Binance order type string to unified OrderType.
    fn parse_order_type(order_type: &str) -> OrderType {
        match order_type {
            "MARKET" => OrderType::Market,
            "LIMIT" => OrderType::Limit,
            "STOP_MARKET" | "STOP_LOSS" => OrderType::StopMarket,
            "STOP_LOSS_LIMIT" | "TAKE_PROFIT_LIMIT" => OrderType::StopLimit,
            _ => OrderType::Market,
        }
    }

    /// Convert unified Side to Binance string.
    fn side_str(side: &Side) -> &'static str {
        match side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        }
    }

    /// Convert unified OrderType to Binance string.
    fn order_type_str(order_type: &OrderType) -> &'static str {
        match order_type {
            OrderType::Market => "MARKET",
            OrderType::Limit => "LIMIT",
            OrderType::StopMarket => "STOP_MARKET",
            OrderType::StopLimit => "STOP_LIMIT",
        }
    }

    /// Return the API path prefix based on market type.
    fn api_prefix(&self) -> &'static str {
        match self.market_type {
            MarketType::Spot => "/api/v3",
            MarketType::Perpetual => "/fapi/v1",
        }
    }

    /// Check if this instance is configured for perpetual futures.
    fn is_perpetual(&self) -> bool {
        self.market_type == MarketType::Perpetual
    }
}

#[async_trait]
impl Exchange for BinanceExchange {
    fn id(&self) -> &str { "binance" }
    fn name(&self) -> &str { "Binance" }

    fn capabilities(&self) -> &ExchangeCapabilities {
        static CAPS: std::sync::OnceLock<ExchangeCapabilities> = std::sync::OnceLock::new();
        CAPS.get_or_init(|| ExchangeCapabilities {
            has: ExchangeFeatures {
                spot: true,
                futures: false,
                perpetual: true,
                fetch_ticker: true,
                fetch_tickers: false,
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
                max_requests_per_second: 20.0,
                max_requests_per_minute: Some(1200.0),
            },
            timeframes: vec![
                ("1m".into(), "1m".into()),
                ("3m".into(), "3m".into()),
                ("5m".into(), "5m".into()),
                ("15m".into(), "15m".into()),
                ("30m".into(), "30m".into()),
                ("1h".into(), "1h".into()),
                ("2h".into(), "2h".into()),
                ("4h".into(), "4h".into()),
                ("6h".into(), "6h".into()),
                ("8h".into(), "8h".into()),
                ("12h".into(), "12h".into()),
                ("1d".into(), "1d".into()),
                ("3d".into(), "3d".into()),
                ("1w".into(), "1w".into()),
                ("1M".into(), "1M".into()),
            ],
        })
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker, ExchangeError> {
        let native = Self::to_native_symbol(symbol);
        let path = format!("{}/ticker/24hr", self.api_prefix());
        let data = self.client
            .public_get(&path, &[("symbol", native.as_str())])
            .await?;

        let last = parse_f64(&data, "lastPrice");
        if last.is_none() || last == Some(0.0) {
            return Err(ExchangeError::no_data(format!(
                "No ticker data available for {} on Binance", symbol
            )));
        }

        Ok(Ticker {
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

    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: &str,
        limit: u32,
        since: Option<i64>,
    ) -> Result<Vec<Kline>, ExchangeError> {
        let native = Self::to_native_symbol(symbol);
        let mut params: Vec<(&str, String)> = vec![
            ("symbol", native),
            ("interval", timeframe.to_string()),
            ("limit", limit.to_string()),
        ];
        if let Some(s) = since {
            params.push(("startTime", s.to_string()));
        }

        let path = format!("{}/klines", self.api_prefix());
        let data = self.client
            .public_get(&path, &params.iter().map(|(k, v)| (*k, v.as_str())).collect::<Vec<_>>())
            .await?;

        let arr = data.as_array().ok_or_else(|| {
            ExchangeError::no_data(format!("Invalid kline response for {} on Binance", symbol))
        })?;

        if arr.is_empty() {
            return Err(ExchangeError::no_data(format!(
                "No OHLCV data available for {} ({}) on Binance. Check symbol and timeframe.",
                symbol, timeframe
            )));
        }

        let klines = arr.iter().map(|k| {
            let a = k.as_array().unwrap();
            Kline {
                timestamp: a[0].as_i64().unwrap_or(0),
                open: a[1].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                high: a[2].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                low: a[3].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                close: a[4].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                volume: a[5].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                quote_volume: a[7].as_str().unwrap_or("0").parse().ok(),
                trades: a[8].as_i64(),
            }
        }).collect();

        Ok(klines)
    }

    async fn fetch_order_book(&self, symbol: &str, limit: u32) -> Result<OrderBook, ExchangeError> {
        let native = Self::to_native_symbol(symbol);
        let path = format!("{}/depth", self.api_prefix());
        let data = self.client
            .public_get(&path, &[("symbol", native.as_str()), ("limit", &limit.to_string())])
            .await?;

        let bids = parse_order_book_side(&data, "bids");
        let asks = parse_order_book_side(&data, "asks");

        if bids.is_empty() && asks.is_empty() {
            return Err(ExchangeError::no_data(format!(
                "No order book data for {} on Binance", symbol
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
        if self.is_perpetual() {
            // USDT-M Futures: /fapi/v2/balance returns an array
            let params: Vec<(String, String)> = vec![];
            let data = self.client
                .signed_get(&self.signer, "/fapi/v2/balance", params)
                .await?;

            let balances = data.as_array()
                .ok_or_else(|| ExchangeError::Internal("Invalid futures balance response from Binance".into()))?;

            let result: Vec<Balance> = balances.iter()
                .filter_map(|b| {
                    let free = parse_f64(b, "availableBalance").unwrap_or(0.0);
                    let total = parse_f64(b, "balance").unwrap_or(0.0);
                    let used = total - free;
                    if free == 0.0 && used == 0.0 {
                        return None; // Skip zero balances
                    }
                    Some(Balance {
                        asset: parse_str(b, "asset"),
                        free,
                        used,
                        total,
                    })
                })
                .collect();

            Ok(result)
        } else {
            // Spot: /api/v3/account returns { "balances": [...] }
            let params: Vec<(String, String)> = vec![];
            let data = self.client
                .signed_get(&self.signer, "/api/v3/account", params)
                .await?;

            let balances = data.get("balances").and_then(|b| b.as_array())
                .ok_or_else(|| ExchangeError::Internal("Invalid balance response from Binance".into()))?;

            let result: Vec<Balance> = balances.iter()
                .filter_map(|b| {
                    let free = parse_f64(b, "free").unwrap_or(0.0);
                    let used = parse_f64(b, "locked").unwrap_or(0.0);
                    if free == 0.0 && used == 0.0 {
                        return None; // Skip zero balances
                    }
                    Some(Balance {
                        asset: parse_str(b, "asset"),
                        free,
                        used,
                        total: free + used,
                    })
                })
                .collect();

            Ok(result)
        }
    }

    async fn fetch_markets(&self) -> Result<Vec<MarketInfo>, ExchangeError> {
        let path = format!("{}/exchangeInfo", self.api_prefix());
        let data = self.client.public_get(&path, &[]).await?;

        let symbols = data.get("symbols").and_then(|s| s.as_array())
            .ok_or_else(|| ExchangeError::Internal("Invalid exchangeInfo response".into()))?;

        let markets: Vec<MarketInfo> = symbols.iter()
            .filter_map(|s| {
                let status = parse_str(s, "status");
                if status != "TRADING" {
                    return None;
                }

                // For perpetual, only keep PERPETUAL contracts
                if self.is_perpetual() {
                    let contract_type = parse_str(s, "contractType");
                    if contract_type != "PERPETUAL" {
                        return None;
                    }
                }

                let base = parse_str(s, "baseAsset");
                let quote = parse_str(s, "quoteAsset");
                let symbol = format!("{}/{}", base, quote);

                let market_type = if self.is_perpetual() {
                    MarketType::Perpetual
                } else {
                    MarketType::Spot
                };

                Some(MarketInfo {
                    id: parse_str(s, "symbol"),
                    symbol,
                    base,
                    quote,
                    active: true,
                    market_type,
                    min_amount: parse_f64(s, "minQty"),
                    max_amount: parse_f64(s, "maxQty"),
                    min_price: parse_f64(s, "minPrice"),
                    max_price: parse_f64(s, "maxPrice"),
                    min_cost: None, // Binance doesn't directly expose minCost
                    price_precision: Some(parse_u32(s, "pricePrecision")),
                    amount_precision: Some(parse_u32(s, "quantityPrecision")),
                    info: s.clone(),
                })
            })
            .collect();

        Ok(markets)
    }

    async fn create_order(&self, params: PlaceOrderParams) -> Result<Order, ExchangeError> {
        let native = Self::to_native_symbol(&params.symbol);
        let mut body = serde_json::json!({
            "symbol": native,
            "side": Self::side_str(&params.side),
            "type": Self::order_type_str(&params.order_type),
            "quantity": params.amount,
        });

        if let Some(price) = params.price {
            body["price"] = serde_json::json!(price);
            body["timeInForce"] = serde_json::json!(
                params.time_in_force.as_ref()
                    .map(|tif| match tif {
                        TimeInForce::Gtc => "GTC",
                        TimeInForce::Ioc => "IOC",
                        TimeInForce::Fok => "FOK",
                        TimeInForce::Poc => "GTC", // POST_ONLY not directly mapped
                    })
                    .unwrap_or("GTC")
            );
        }

        if let Some(stop_price) = params.stop_price {
            body["stopPrice"] = serde_json::json!(stop_price);
        }

        if let Some(ref client_id) = params.client_order_id {
            body["newClientOrderId"] = serde_json::json!(client_id);
        }

        // Perpetual: add positionSide for hedge mode
        if self.is_perpetual() {
            let position_side = match (&params.side, &params.position_side) {
                (Side::Buy, Some(PositionSide::Long)) => "LONG",
                (Side::Sell, Some(PositionSide::Short)) => "SHORT",
                (Side::Buy, Some(PositionSide::Short)) => "SHORT",  // close short
                (Side::Sell, Some(PositionSide::Long)) => "LONG",   // close long
                _ => "BOTH",  // one-way mode
            };
            body["positionSide"] = serde_json::json!(position_side);
        }

        let path = format!("{}/order", self.api_prefix());
        let data = self.client
            .signed_post(&self.signer, &path, body)
            .await?;

        Ok(Order {
            id: parse_str(&data, "orderId"),
            client_order_id: parse_str_opt(&data, "clientOrderId"),
            symbol: params.symbol,
            side: params.side,
            order_type: params.order_type,
            price: parse_f64(&data, "price"),
            amount: parse_f64(&data, "origQty").unwrap_or(params.amount),
            cost: None,
            filled: parse_f64(&data, "executedQty").unwrap_or(0.0),
            remaining: parse_f64(&data, "origQty").unwrap_or(params.amount)
                - parse_f64(&data, "executedQty").unwrap_or(0.0),
            status: Self::parse_order_status(&parse_str(&data, "status")),
            fee: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            info: data,
        })
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<Order, ExchangeError> {
        let native = Self::to_native_symbol(symbol);
        let body = serde_json::json!({
            "symbol": native,
            "orderId": order_id,
        });

        let path = format!("{}/order", self.api_prefix());
        let data = self.client
            .signed_post(&self.signer, &path, body)
            .await?;

        Ok(Order {
            id: parse_str(&data, "orderId"),
            client_order_id: parse_str_opt(&data, "clientOrderId"),
            symbol: symbol.to_string(),
            side: if parse_str(&data, "side") == "BUY" { Side::Buy } else { Side::Sell },
            order_type: Self::parse_order_type(&parse_str(&data, "type")),
            price: parse_f64(&data, "price"),
            amount: parse_f64(&data, "origQty").unwrap_or(0.0),
            cost: None,
            filled: parse_f64(&data, "executedQty").unwrap_or(0.0),
            remaining: 0.0,
            status: OrderStatus::Canceled,
            fee: None,
            created_at: None,
            updated_at: Some(Utc::now()),
            info: data,
        })
    }

    async fn fetch_order(&self, symbol: &str, order_id: &str) -> Result<Order, ExchangeError> {
        let native = Self::to_native_symbol(symbol);
        let params = vec![
            ("symbol".into(), native),
            ("orderId".into(), order_id.to_string()),
        ];

        let path = format!("{}/order", self.api_prefix());
        let data = self.client
            .signed_get(&self.signer, &path, params)
            .await?;

        Ok(Order {
            id: parse_str(&data, "orderId"),
            client_order_id: parse_str_opt(&data, "clientOrderId"),
            symbol: symbol.to_string(),
            side: if parse_str(&data, "side") == "BUY" { Side::Buy } else { Side::Sell },
            order_type: Self::parse_order_type(&parse_str(&data, "type")),
            price: parse_f64(&data, "price"),
            amount: parse_f64(&data, "origQty").unwrap_or(0.0),
            cost: None,
            filled: parse_f64(&data, "executedQty").unwrap_or(0.0),
            remaining: parse_f64(&data, "origQty").unwrap_or(0.0)
                - parse_f64(&data, "executedQty").unwrap_or(0.0),
            status: Self::parse_order_status(&parse_str(&data, "status")),
            fee: None,
            created_at: None,
            updated_at: Some(Utc::now()),
            info: data,
        })
    }

    async fn fetch_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>, ExchangeError> {
        let params: Vec<(String, String)> = if let Some(sym) = symbol {
            vec![("symbol".into(), Self::to_native_symbol(sym))]
        } else {
            vec![]
        };

        let path = format!("{}/openOrders", self.api_prefix());
        let data = self.client
            .signed_get(&self.signer, &path, params)
            .await?;

        let arr = data.as_array().cloned().unwrap_or_default();
        let orders = arr.iter().map(|o| {
            Order {
                id: parse_str(o, "orderId"),
                client_order_id: parse_str_opt(o, "clientOrderId"),
                symbol: Self::to_unified_symbol(&parse_str(o, "symbol")),
                side: if parse_str(o, "side") == "BUY" { Side::Buy } else { Side::Sell },
                order_type: Self::parse_order_type(&parse_str(o, "type")),
                price: parse_f64(o, "price"),
                amount: parse_f64(o, "origQty").unwrap_or(0.0),
                cost: None,
                filled: parse_f64(o, "executedQty").unwrap_or(0.0),
                remaining: parse_f64(o, "origQty").unwrap_or(0.0)
                    - parse_f64(o, "executedQty").unwrap_or(0.0),
                status: Self::parse_order_status(&parse_str(o, "status")),
                fee: None,
                created_at: None,
                updated_at: None,
                info: o.clone(),
            }
        }).collect();

        Ok(orders)
    }

    async fn set_leverage(
        &self,
        symbol: &str,
        leverage: u32,
        margin_mode: MarginMode,
    ) -> Result<(), ExchangeError> {
        if !self.is_perpetual() {
            return Err(ExchangeError::NotSupported(
                "Leverage is only supported for perpetual futures".into(),
            ));
        }

        let native = Self::to_native_symbol(symbol);

        // First set margin type (CROSSED or ISOLATED)
        let margin_type_str = match margin_mode {
            MarginMode::Cross => "CROSSED",
            MarginMode::Isolated => "ISOLATED",
        };
        let margin_body = serde_json::json!({
            "symbol": native,
            "marginType": margin_type_str,
        });
        // Ignore errors from marginType — it may return "No need to change" if already set
        let _ = self.client
            .signed_post(&self.signer, "/fapi/v1/marginType", margin_body)
            .await;

        // Then set leverage
        let leverage_body = serde_json::json!({
            "symbol": native,
            "leverage": leverage,
        });
        let _data = self.client
            .signed_post(&self.signer, "/fapi/v1/leverage", leverage_body)
            .await?;

        Ok(())
    }

    async fn fetch_positions(
        &self,
        symbol: Option<&str>,
    ) -> Result<Vec<Position>, ExchangeError> {
        if !self.is_perpetual() {
            return Err(ExchangeError::NotSupported(
                "Positions are only supported for perpetual futures".into(),
            ));
        }

        let mut params: Vec<(String, String)> = vec![];
        if let Some(sym) = symbol {
            params.push(("symbol".into(), Self::to_native_symbol(sym)));
        }

        let data = self.client
            .signed_get(&self.signer, "/fapi/v2/positionRisk", params)
            .await?;

        let arr = data.as_array()
            .ok_or_else(|| ExchangeError::Internal("Invalid positionRisk response from Binance".into()))?;

        let positions: Vec<Position> = arr.iter()
            .filter_map(|p| {
                let pos_amt = parse_f64(p, "positionAmt").unwrap_or(0.0);
                if pos_amt == 0.0 {
                    return None; // Skip positions with zero amount
                }

                let side = if pos_amt > 0.0 {
                    PositionSide::Long
                } else {
                    PositionSide::Short
                };
                let size = pos_amt.abs();

                let margin_type_str = parse_str(p, "marginType");
                let margin_mode = match margin_type_str.as_str() {
                    "isolated" => MarginMode::Isolated,
                    _ => MarginMode::Cross,
                };

                Some(Position {
                    symbol: Self::to_unified_symbol(&parse_str(p, "symbol")),
                    side,
                    size,
                    entry_price: parse_f64(p, "entryPrice").unwrap_or(0.0),
                    leverage: parse_u32(p, "leverage"),
                    unrealized_pnl: parse_f64(p, "unRealizedProfit").unwrap_or(0.0),
                    margin_mode,
                    liquidation_price: parse_f64(p, "liquidationPrice"),
                    info: p.clone(),
                })
            })
            .collect();

        Ok(positions)
    }

    async fn fetch_funding_rate(
        &self,
        symbol: &str,
    ) -> Result<FundingRate, ExchangeError> {
        if !self.is_perpetual() {
            return Err(ExchangeError::NotSupported(
                "Funding rate is only supported for perpetual futures".into(),
            ));
        }

        let native = Self::to_native_symbol(symbol);
        let data = self.client
            .public_get("/fapi/v1/fundingRate", &[("symbol", native.as_str())])
            .await?;

        let arr = data.as_array()
            .ok_or_else(|| ExchangeError::Internal("Invalid fundingRate response from Binance".into()))?;

        // Get the last (most recent) entry
        let last = arr.last()
            .ok_or_else(|| ExchangeError::no_data(format!(
                "No funding rate data for {} on Binance", symbol
            )))?;

        let rate = parse_f64(last, "fundingRate").unwrap_or(0.0);
        let funding_time = last.get("fundingTime")
            .and_then(|t| t.as_i64())
            .map(|ts| {
                chrono::DateTime::from_timestamp_millis(ts)
                    .unwrap_or_else(Utc::now)
            });

        Ok(FundingRate {
            symbol: symbol.to_string(),
            rate,
            next_funding_time: funding_time,
            info: last.clone(),
        })
    }

    async fn fetch_funding_history(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<FundingHistoryEntry>, ExchangeError> {
        if !self.is_perpetual() {
            return Err(ExchangeError::NotSupported(
                "Funding rate is only supported for perpetual futures".into(),
            ));
        }

        let native = Self::to_native_symbol(symbol);
        let mut all_entries = Vec::new();
        let mut current_start = start_time;

        // Binance returns max 1000 entries per request, paginate if needed
        loop {
            let data = self.client
                .public_get("/fapi/v1/fundingRate", &[
                    ("symbol", native.as_str()),
                    ("startTime", &current_start.to_string()),
                    ("endTime", &end_time.to_string()),
                    ("limit", "1000"),
                ])
                .await?;

            let arr = data.as_array()
                .ok_or_else(|| ExchangeError::Internal("Invalid fundingRate history response from Binance".into()))?;

            if arr.is_empty() {
                break;
            }

            for item in arr {
                let funding_time = item.get("fundingTime")
                    .and_then(|t| t.as_i64())
                    .unwrap_or(0);
                let rate = parse_f64(item, "fundingRate").unwrap_or(0.0);
                all_entries.push(FundingHistoryEntry { funding_time, rate });
            }

            if arr.len() < 1000 {
                break;
            }

            // Move start time past the last entry to avoid duplicates
            if let Some(last) = arr.last() {
                current_start = last.get("fundingTime")
                    .and_then(|t| t.as_i64())
                    .unwrap_or(end_time) + 1;
            } else {
                break;
            }
        }

        Ok(all_entries)
    }

    async fn ping(&self) -> Result<bool, ExchangeError> {
        let path = format!("{}/ping", self.api_prefix());
        let data = self.client.public_get(&path, &[]).await?;
        Ok(!data.is_null())
    }

    async fn load_markets(&mut self) -> Result<(), ExchangeError> {
        info!("Loading Binance markets (type={:?})...", self.market_type);
        self.markets = Some(self.fetch_markets().await?);
        info!("Loaded {} Binance markets", self.markets.as_ref().unwrap().len());
        Ok(())
    }

    fn markets(&self) -> &Option<Vec<MarketInfo>> {
        &self.markets
    }
}

/// Parse order book bids/asks from exchange response.
fn parse_order_book_side(data: &serde_json::Value, side: &str) -> Vec<(f64, f64)> {
    data.get(side)
        .and_then(|b| b.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|b| {
                    let a = b.as_array()?;
                    Some((
                        a[0].as_str()?.parse().ok()?,
                        a[1].as_str()?.parse().ok()?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}
