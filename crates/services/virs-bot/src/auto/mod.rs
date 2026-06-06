//! Auto trading bot module.

pub mod engine;
pub mod ports;
pub mod types;
pub mod worker;
pub mod ai;
pub mod strategy;
pub mod adapters;

pub use engine::AutoEngine;
