//! Trading infrastructure module.
//!
//! This module contains all trading-related components:
//!
//! - `ports` — Generic trading interfaces (OrderExecutor trait)
//! - `paper` — Paper trading adapter (PaperExchangeAdapter for Position Engine)
//! - `ccxt` — Low-level exchange API (Binance, OKX, Bybit)
//! - `exchange` — Application-level exchange abstraction (Exchange trait, Registry)
//!
//! Architecture:
//! ```text
//! trading/
//! ├── ports.rs           ← OrderExecutor trait (generic)
//! ├── paper/             ← PaperExchangeAdapter (simulated, for Position Engine)
//! ├── ccxt/              ← Exchange API (HTTP/WebSocket)
//! └── exchange/          ← Exchange trait, Registry (application-level)
//! ```

pub mod ports;

pub mod paper;
pub mod ccxt;
pub mod exchange;
