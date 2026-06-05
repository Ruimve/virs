//! Exchange adapters.
//!
//! Each subdirectory contains the implementation for a specific exchange:
//! - `binance/` — Binance REST API + WebSocket clients
//! - `okx/` — OKX REST API
//! - `bybit/` — Bybit REST API

pub mod binance;
pub mod okx;
pub mod bybit;
