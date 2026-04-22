//! Binance exchange implementation.
//!
//! Implements the full CCXT-style Exchange trait for Binance:
//! - REST API: https://api.binance.com
//! - Testnet: https://testnet.binance.vision
//! - Auth: HMAC-SHA256 via query string (GET) or form body (POST)
//! - Rate limit: 1200 req/min (weight-based)
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
use super::auth::BinanceSigner;
use super::{Exchange, ExchangeClient, parse_f64, parse_str, parse_str_opt, parse_u32};

/// Binance exchange implementation.
pub struct BinanceExchange {
    client: ExchangeClient,
    signer: BinanceSigner,
    markets: Option<Vec<MarketInfo>>,
    testnet: bool,
}

impl BinanceExchange {
    /// Create a new Binance exchange instance.
    pub fn new(
        api_key: &str,
        api_secret: &str,
        proxy_url: Option<&str>,
    ) -> Result<Self, ExchangeError> {
        let base_url = "https://api.binance.com";
        let client = ExchangeClient::new(base_url, 20, proxy_url)?;
        let signer = BinanceSigner::new(api_key.to_string(), api_secret.to_string());

        Ok(Self {
            client,
            signer,
            markets: None,
            testnet: false,
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
                perpetual: false,
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
        let data = self.client
            .public_get("/api/v3/ticker/24hr", &[("symbol", native.as_str())])
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

        let data = self.client
            .public_get("/api/v3/klines", &params.iter().map(|(k, v)| (*k, v.as_str())).collect::<Vec<_>>())
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
        let data = self.client
            .public_get("/api/v3/depth", &[("symbol", native.as_str()), ("limit", &limit.to_string())])
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

    async fn fetch_markets(&self) -> Result<Vec<MarketInfo>, ExchangeError> {
        let data = self.client.public_get("/api/v3/exchangeInfo", &[]).await?;

        let symbols = data.get("symbols").and_then(|s| s.as_array())
            .ok_or_else(|| ExchangeError::Internal("Invalid exchangeInfo response".into()))?;

        let markets: Vec<MarketInfo> = symbols.iter()
            .filter_map(|s| {
                let status = parse_str(s, "status");
                if status != "TRADING" {
                    return None;
                }

                let base = parse_str(s, "baseAsset");
                let quote = parse_str(s, "quoteAsset");
                let symbol = format!("{}/{}", base, quote);

                Some(MarketInfo {
                    id: parse_str(s, "symbol"),
                    symbol,
                    base,
                    quote,
                    active: true,
                    market_type: MarketType::Spot,
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

        let data = self.client
            .signed_post(&self.signer, "/api/v3/order", body)
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

        let data = self.client
            .signed_post(&self.signer, "/api/v3/order", body)
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

        let data = self.client
            .signed_get(&self.signer, "/api/v3/order", params)
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

        let data = self.client
            .signed_get(&self.signer, "/api/v3/openOrders", params)
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

    async fn ping(&self) -> Result<bool, ExchangeError> {
        let data = self.client.public_get("/api/v3/ping", &[]).await?;
        Ok(!data.is_null())
    }

    async fn load_markets(&mut self) -> Result<(), ExchangeError> {
        info!("Loading Binance markets...");
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
