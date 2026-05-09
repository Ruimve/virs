//! Trading infrastructure module.
//!
//! This module contains all trading-related components:
//!
//! - `ports` — Generic trading interfaces (OrderExecutor trait)
//! - `paper` — Paper trading executor (simulated order matching)
//! - `ccxt` — Low-level exchange API (Binance, OKX, Bybit)
//! - `exchange` — Application-level exchange abstraction (Exchange trait, Registry)
//!
//! Architecture:
//! ```text
//! trading/
//! ├── ports.rs           ← OrderExecutor trait (generic)
//! ├── paper/             ← PaperOrderExecutor (simulated)
//! ├── ccxt/              ← Exchange API (HTTP/WebSocket)
//! └── exchange/          ← Exchange trait, Registry (application-level)
//! ```

pub mod ports;

pub mod paper;
pub mod ccxt;
pub mod exchange;
