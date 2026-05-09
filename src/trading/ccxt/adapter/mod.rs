//! Exchange adapters.
//!
//! Each subdirectory contains the implementation for a specific exchange:
//! - `binance/` — Binance REST API + WebSocket clients
//! - `okx/` — OKX REST API
//! - `bybit/` — Bybit REST API

pub mod binance;
pub mod okx;
pub mod bybit;

pub use binance::BinanceExchange;
pub use okx::OkxExchange;
pub use bybit::BybitExchange;

// Re-export WebSocket types for convenience
pub use binance::kline_ws::{BinanceKlineWs, Candle, WsCandleUpdate, WsEvent};
pub use binance::order_ws::{BinanceOrderWs, OrderStatus, WsFeedEvent};
