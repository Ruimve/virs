//! Binance exchange implementation.
//!
//! API endpoints are organized by Binance's path prefixes:
//! - `api.rs`  — /api/v3  (Spot market: ticker, klines, orders, etc.)
//! - `sapi.rs` — /sapi/v1 (Account & funds: balance, apiRestrictions, etc.)
//! - `fapi.rs` — /fapi/v1 (USDT-M Futures: perpetual trading, positions, funding, etc.)
//!
//! The `BinanceExchange` struct dispatches to the appropriate module
//! based on `market_type` (Spot → api, Perpetual → fapi).
//! Account endpoints (sapi) are shared across market types.

pub mod api;
pub mod fapi;
pub mod sapi;
pub mod kline_ws;
pub mod order_ws;

use async_trait::async_trait;
use tracing::info;

use crate::types::*;
use crate::errors::ExchangeError;
use crate::auth::{Signer, SignedRequest, hmac_sha256_hex, insert_header};
use crate::{Exchange, ExchangeClient};

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
        let timestamp = chrono::Utc::now().timestamp_millis();
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
            chrono::Utc::now().timestamp_millis().to_string(),
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
///
/// Dispatches to `api` (spot) or `fapi` (perpetual) modules based on `market_type`.
/// Account endpoints (`sapi`) are available regardless of market type.
pub struct BinanceExchange {
    client: ExchangeClient,
    signer: BinanceSigner,
    markets: Option<Vec<MarketInfo>>,
    #[allow(dead_code)]
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
        let max_concurrent: u32 = match market_type {
            MarketType::Spot => 20,
            MarketType::Perpetual => 40,
        };
        let client = ExchangeClient::new(max_concurrent, proxy_url)?;
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
    pub fn to_native_symbol(symbol: &str) -> String {
        symbol.replace('/', "").replace('-', "")
    }

    /// Convert Binance symbol to unified format.
    pub fn to_unified_symbol(native: &str) -> String {
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

    /// Parse Binance order status string to unified CcxtOrderStatus.
    pub fn parse_order_status(status: &str) -> CcxtOrderStatus {
        match status {
            "NEW" => CcxtOrderStatus::Open,
            "PARTIALLY_FILLED" => CcxtOrderStatus::PartiallyFilled,
            "FILLED" => CcxtOrderStatus::Filled,
            "CANCELED" | "CANCELLED" | "EXPIRED" => CcxtOrderStatus::Canceled,
            "REJECTED" => CcxtOrderStatus::Rejected,
            "PENDING_CANCEL" => CcxtOrderStatus::Open,
            _ => CcxtOrderStatus::Open,
        }
    }

    /// Parse Binance order type string to unified OrderType.
    pub fn parse_order_type(order_type: &str) -> OrderType {
        match order_type {
            "MARKET" => OrderType::Market,
            "LIMIT" => OrderType::Limit,
            "STOP_MARKET" | "STOP_LOSS" => OrderType::StopMarket,
            "STOP_LOSS_LIMIT" | "TAKE_PROFIT_LIMIT" => OrderType::StopLimit,
            "TAKE_PROFIT_MARKET" => OrderType::TakeProfitMarket,
            _ => OrderType::Market,
        }
    }

    /// Convert unified Side to Binance string.
    pub fn side_str(side: &Side) -> &'static str {
        match side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        }
    }

    /// Convert unified OrderType to Binance string.
    pub fn order_type_str(order_type: &OrderType) -> &'static str {
        match order_type {
            OrderType::Market => "MARKET",
            OrderType::Limit => "LIMIT",
            OrderType::StopMarket => "STOP_MARKET",
            OrderType::StopLimit => "STOP_LIMIT",
            OrderType::TakeProfitMarket => "TAKE_PROFIT_MARKET",
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

    // ---- Market data (dispatch by market_type) ----

    async fn fetch_ticker(&self, symbol: &str) -> Result<CcxtTicker, ExchangeError> {
        if self.is_perpetual() {
            fapi::fetch_ticker(&self.client, symbol).await
        } else {
            api::fetch_ticker(&self.client, symbol).await
        }
    }

    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: &str,
        limit: u32,
        since: Option<i64>,
    ) -> Result<Vec<CcxtKline>, ExchangeError> {
        if self.is_perpetual() {
            fapi::fetch_ohlcv(&self.client, symbol, timeframe, limit, since).await
        } else {
            api::fetch_ohlcv(&self.client, symbol, timeframe, limit, since).await
        }
    }

    async fn fetch_order_book(&self, symbol: &str, limit: u32) -> Result<CcxtOrderBook, ExchangeError> {
        if self.is_perpetual() {
            fapi::fetch_order_book(&self.client, symbol, limit).await
        } else {
            api::fetch_order_book(&self.client, symbol, limit).await
        }
    }

    async fn fetch_balance(&self) -> Result<Vec<Balance>, ExchangeError> {
        tracing::info!("[BinanceExchange::fetch_balance] is_perpetual={}", self.is_perpetual());
        if self.is_perpetual() {
            fapi::fetch_balance(&self.client, &self.signer).await
        } else {
            api::fetch_balance(&self.client, &self.signer).await
        }
    }

    async fn fetch_markets(&self) -> Result<Vec<MarketInfo>, ExchangeError> {
        if self.is_perpetual() {
            fapi::fetch_markets(&self.client).await
        } else {
            api::fetch_markets(&self.client).await
        }
    }

    // ---- Trading (dispatch by market_type) ----

    async fn create_order(&self, params: PlaceOrderParams) -> Result<CcxtOrder, ExchangeError> {
        if self.is_perpetual() {
            fapi::create_order(&self.client, &self.signer, params).await
        } else {
            api::create_order(&self.client, &self.signer, params).await
        }
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<CcxtOrder, ExchangeError> {
        if self.is_perpetual() {
            fapi::cancel_order(&self.client, &self.signer, symbol, order_id).await
        } else {
            api::cancel_order(&self.client, &self.signer, symbol, order_id).await
        }
    }

    async fn fetch_order(&self, symbol: &str, order_id: &str) -> Result<CcxtOrder, ExchangeError> {
        if self.is_perpetual() {
            fapi::fetch_order(&self.client, &self.signer, symbol, order_id).await
        } else {
            api::fetch_order(&self.client, &self.signer, symbol, order_id).await
        }
    }

    async fn fetch_open_orders(&self, symbol: Option<&str>) -> Result<Vec<CcxtOrder>, ExchangeError> {
        if self.is_perpetual() {
            fapi::fetch_open_orders(&self.client, &self.signer, symbol).await
        } else {
            api::fetch_open_orders(&self.client, &self.signer, symbol).await
        }
    }

    // ---- Perpetual-only ----

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
        fapi::set_margin_type(&self.client, &self.signer, symbol, margin_mode).await?;
        fapi::set_leverage(&self.client, &self.signer, symbol, leverage).await
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
        fapi::fetch_positions(&self.client, &self.signer, symbol).await
    }

    async fn get_position_mode(&self) -> Result<PositionMode, ExchangeError> {
        if !self.is_perpetual() {
            return Err(ExchangeError::NotSupported(
                "Position mode is only supported for perpetual futures".into(),
            ));
        }
        fapi::get_position_mode(&self.client, &self.signer).await
    }

    async fn fetch_funding_rate(
        &self,
        symbol: &str,
    ) -> Result<CcxtFundingRate, ExchangeError> {
        if !self.is_perpetual() {
            return Err(ExchangeError::NotSupported(
                "Funding rate is only supported for perpetual futures".into(),
            ));
        }
        fapi::fetch_funding_rate(&self.client, symbol).await
    }

    async fn fetch_funding_history(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<CcxtFundingHistoryEntry>, ExchangeError> {
        if !self.is_perpetual() {
            return Err(ExchangeError::NotSupported(
                "Funding history is only supported for perpetual futures".into(),
            ));
        }
        fapi::fetch_funding_history(&self.client, symbol, start_time, end_time).await
    }

    // ---- User data stream ----

    async fn create_listen_key(&self) -> Result<String, ExchangeError> {
        if self.is_perpetual() {
            fapi::create_listen_key(&self.client, &self.signer).await
        } else {
            api::create_listen_key(&self.client, &self.signer).await
        }
    }

    async fn keepalive_listen_key(&self, listen_key: &str) -> Result<(), ExchangeError> {
        if self.is_perpetual() {
            fapi::keepalive_listen_key(&self.client, &self.signer, listen_key).await
        } else {
            api::keepalive_listen_key(&self.client, &self.signer, listen_key).await
        }
    }

    // ---- Account (sapi) ----

    async fn fetch_api_restrictions(&self) -> Result<ApiRestrictions, ExchangeError> {
        sapi::fetch_api_restrictions(&self.client, &self.signer).await
    }

    // ---- Misc ----

    async fn ping(&self) -> Result<bool, ExchangeError> {
        if self.is_perpetual() {
            fapi::ping(&self.client).await
        } else {
            api::ping(&self.client).await
        }
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
