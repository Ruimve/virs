//! Exchange adapter modules.
//!
//! Each exchange implementation lives in its own module and implements
//! the unified `Exchange` trait. To add a new exchange:
//!
//! 1. Create a module here (e.g., `pub mod bybit;`)
//! 2. Implement `Exchange` trait for your exchange struct
//! 3. Add a match arm in `crate::create_exchange()`

pub mod binance;
// pub mod okx;   // Planned — architecture ready for quick implementation
// pub mod bybit; // Planned — architecture ready for quick implementation
