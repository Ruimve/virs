//! Auto trading bot module.

pub mod adapters;
pub mod ai;
pub mod engine;
pub mod ports;
pub mod strategy;
pub mod types;
pub mod worker;

pub use engine::AutoEngine;
