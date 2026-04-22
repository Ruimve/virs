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
use chrono::Utc;
use tracing::info;

use super::types::*;
use super::errors::ExchangeError;
use super::auth::BybitSigner;
use super::{Exchange, ExchangeClient, parse_str, parse_str_opt};

/// Bybit exchange implementation.
pub struct BybitExchange {
    client: ExchangeClient,
    signer: BybitSigner,
    markets: Option<Vec<MarketInfo>>,
}

impl BybitExchange {
    /// Create a new Bybit exchange instance.
    pub fn new(
        api_key: &str,
        api_secret: &str,
        proxy_url: Option<&str>,
    ) -> Result<Self, ExchangeError> {
        let base_url = "https://api.bybit.com";
        let client = ExchangeClient::new(base_url, 20, proxy_url)?;
        let signer = BybitSigner::new(api_key.to_string(), api_secret.to_string());

        Ok(Self {
            client,
            signer,
            markets: None,
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
            .public_get("/v5/market/tickers", &[("category", "spot"), ("symbol", native.as_str())])
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
            ("category", "spot".into()),
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
        let mut klines: Vec<Kline> = list.iter().map(|k| {
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

        klines.reverse();
        Ok(klines)
    }

    async fn fetch_order_book(&self, symbol: &str, limit: u32) -> Result<OrderBook, ExchangeError> {
        let native = Self::to_native_symbol(symbol);
        let data = self.client
            .public_get("/v5/market/orderbook", &[
                ("category", "spot"),
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
            .public_get("/v5/market/instruments-info", &[("category", "spot")])
            .await?;

        Self::check_ret_code(&data)?;

        let list = data.pointer("/result/list").and_then(|l| l.as_array())
            .ok_or_else(|| ExchangeError::Internal("Invalid instruments response".into()))?;

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
                    id: parse_str(inst, "symbol"),
                    symbol: format!("{}/{}", base, quote),
                    base: base.to_string(),
                    quote: quote.to_string(),
                    active: true,
                    market_type: MarketType::Spot,
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
            "category": "spot",
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

        let data = self.client
            .signed_post(&self.signer, "/v5/order/create", body)
            .await?;

        Self::check_ret_code(&data)?;

        let result = data.pointer("/result")
            .ok_or_else(|| ExchangeError::Internal("No order result from Bybit".into()))?;

        Ok(Order {
            id: parse_str(result, "orderId"),
            client_order_id: parse_str_opt(result, "orderLinkId"),
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
            "category": "spot",
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
            client_order_id: parse_str_opt(result, "orderLinkId"),
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
            ("category".into(), "spot".into()),
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
            id: parse_str(list, "orderId"),
            client_order_id: parse_str_opt(list, "orderLinkId"),
            symbol: symbol.to_string(),
            side: if parse_str(list, "side") == "Buy" { Side::Buy } else { Side::Sell },
            order_type: match parse_str(list, "orderType").as_str() {
                "Market" => OrderType::Market,
                "Limit" => OrderType::Limit,
                _ => OrderType::Market,
            },
            price: list.get("avgPrice").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
            amount,
            cost: None,
            filled,
            remaining: amount - filled,
            status: Self::parse_order_status(&parse_str(list, "orderStatus")),
            fee: None,
            created_at: None,
            updated_at: Some(Utc::now()),
            info: list.clone(),
        })
    }

    async fn fetch_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>, ExchangeError> {
        let mut params: Vec<(String, String)> = vec![("category".into(), "spot".into())];
        if let Some(sym) = symbol {
            params.push(("symbol".into(), Self::to_native_symbol(sym)));
        }

        let data = self.client
            .signed_get(&self.signer, "/v5/order/realtime", params)
            .await?;

        Self::check_ret_code(&data)?;

        let list = data.pointer("/result/list").and_then(|l| l.as_array())
            .cloned().unwrap_or_default();

        let orders = list.iter().map(|o| {
            let filled = o.get("cumExecQty").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let amount = o.get("qty").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            Order {
                id: parse_str(o, "orderId"),
                client_order_id: parse_str_opt(o, "orderLinkId"),
                symbol: Self::to_unified_symbol(&parse_str(o, "symbol")),
                side: if parse_str(o, "side") == "Buy" { Side::Buy } else { Side::Sell },
                order_type: match parse_str(o, "orderType").as_str() {
                    "Market" => OrderType::Market,
                    "Limit" => OrderType::Limit,
                    _ => OrderType::Market,
                },
                price: o.get("price").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                amount,
                cost: None,
                filled,
                remaining: amount - filled,
                status: Self::parse_order_status(&parse_str(o, "orderStatus")),
                fee: None,
                created_at: None,
                updated_at: None,
                info: o.clone(),
            }
        }).collect();

        Ok(orders)
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
