pub mod adapters;
pub mod ai;
pub mod engine;
pub mod types;
pub mod utils;
pub mod worker;

pub use engine::GridEngine;

#[cfg(test)]
mod ai_tests;
#[cfg(test)]
mod types_tests;
#[cfg(test)]
mod utils_tests;
