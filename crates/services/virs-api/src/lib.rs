//! virs-api — API Gateway service.
//!
//! HTTP/WebSocket API layer, routing, and SPA fallback.

pub mod handlers;
pub mod router;
pub mod state;
pub mod ws;

// Re-export
pub use router::build_router;
pub use state::{AppState, EngineManager};

#[cfg(test)]
mod ws_tests;
