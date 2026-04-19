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
//! - Ticker, OHLCV, OrderBook, Balance
//! - Create/Cancel/Fetch orders

use async_trait::async_trait;
use chrono::Utc;
use tracing::info;

use super::types::*;
use super::errors::ExchangeError;
use super::auth::{OkxSigner, Signer};
use super::{Exchange, ExchangeClient, parse_f64, parse_str, parse_str_opt};

/// OKX exchange implementation.
pub struct OkxExchange {
    client: ExchangeClient,
    signer: OkxSigner,
    markets: Option<Vec<MarketInfo>>,
}

impl OkxExchange {
    /// Create a new OKX exchange instance.
    pub fn new(
        api_key: &str,
        api_secret: &str,
        passphrase: &str,
        proxy_url: Option<&str>,
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
        })
    }

    /// Convert unified symbol (e.g. "BTC/USDT") to OKX format (e.g. "BTC-USDT").
    fn to_native_symbol(symbol: &str) -> String {
        symbol.replace('/', "-")
    }

    /// Convert OKX symbol to unified format.
    fn to_unified_symbol(native: &str) -> String {
        native.replace('-', "/")
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
        let inst_id = Self::to_native_symbol(symbol);
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
        let inst_id = Self::to_native_symbol(symbol);
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
        let mut klines: Vec<Kline> = arr.iter().map(|k| {
            let a = k.as_array().unwrap();
            Kline {
                timestamp: a[0].as_str().unwrap_or("0").parse().unwrap_or(0),
                open: a[1].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                high: a[2].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                low: a[3].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                close: a[4].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                volume: a[5].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                quote_volume: a[6].as_str().unwrap_or("0").parse().ok(),
                trades: None,
            }
        }).collect();

        // Reverse to chronological order
        klines.reverse();
        Ok(klines)
    }

    async fn fetch_order_book(&self, symbol: &str, limit: u32) -> Result<OrderBook, ExchangeError> {
        let inst_id = Self::to_native_symbol(symbol);
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
            .public_get("/api/v5/public/instruments", &[("instType", "SPOT")])
            .await?;

        Self::check_okx_code(&data)?;

        let arr = Self::extract_data_array(&data)
            .ok_or_else(|| ExchangeError::Internal("Invalid instruments response".into()))?;

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
                    market_type: MarketType::Spot,
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
        let inst_id = Self::to_native_symbol(&params.symbol);
        let mut body = serde_json::json!({
            "instId": inst_id,
            "tdMode": "cash",
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

        let data = self.client
            .signed_post(&self.signer, "/api/v5/trade/order", body)
            .await?;

        Self::check_okx_code(&data)?;

        let order_data = Self::extract_first_data(&data)
            .ok_or_else(|| ExchangeError::Internal("No order data in OKX response".into()))?;

        Ok(Order {
            id: parse_str(order_data, "ordId"),
            client_order_id: parse_str_opt(order_data, "clOrdId"),
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
        let inst_id = Self::to_native_symbol(symbol);
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
            client_order_id: parse_str_opt(order_data, "clOrdId"),
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
        let inst_id = Self::to_native_symbol(symbol);
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
            id: parse_str(order_data, "ordId"),
            client_order_id: parse_str_opt(order_data, "clOrdId"),
            symbol: symbol.to_string(),
            side: if parse_str(order_data, "side") == "buy" { Side::Buy } else { Side::Sell },
            order_type: match parse_str(order_data, "ordType").as_str() {
                "market" => OrderType::Market,
                "limit" => OrderType::Limit,
                _ => OrderType::Market,
            },
            price: order_data.get("avgPx").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            amount,
            cost: None,
            filled,
            remaining: amount - filled,
            status: Self::parse_order_status(&parse_str(order_data, "state")),
            fee: None,
            created_at: None,
            updated_at: Some(Utc::now()),
            info: order_data.clone(),
        })
    }

    async fn fetch_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>, ExchangeError> {
        let mut params: Vec<(String, String)> = vec![("instType".into(), "SPOT".into())];
        if let Some(sym) = symbol {
            params.push(("instId".into(), Self::to_native_symbol(sym)));
        }

        let data = self.client
            .signed_get(&self.signer, "/api/v5/trade/orders-pending", params)
            .await?;

        Self::check_okx_code(&data)?;

        let arr = Self::extract_data_array(&data).cloned().unwrap_or_default();
        let orders = arr.iter().map(|o| {
            let filled = o.get("fillSz").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let amount = o.get("sz").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            Order {
                id: parse_str(o, "ordId"),
                client_order_id: parse_str_opt(o, "clOrdId"),
                symbol: Self::to_unified_symbol(&parse_str(o, "instId")),
                side: if parse_str(o, "side") == "buy" { Side::Buy } else { Side::Sell },
                order_type: match parse_str(o, "ordType").as_str() {
                    "market" => OrderType::Market,
                    "limit" => OrderType::Limit,
                    _ => OrderType::Market,
                },
                price: o.get("px").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                amount,
                cost: None,
                filled,
                remaining: amount - filled,
                status: Self::parse_order_status(&parse_str(o, "state")),
                fee: None,
                created_at: None,
                updated_at: None,
                info: o.clone(),
            }
        }).collect();

        Ok(orders)
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
