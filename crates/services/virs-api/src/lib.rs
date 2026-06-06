//! virs-api — API Gateway service.
//!
//! HTTP/WebSocket API layer, routing, middleware, and SPA fallback.

pub mod router;
pub mod state;
pub mod handlers;
pub mod middleware;
pub mod ws;

// Re-export
pub use router::build_router;
pub use state::{AppState, WsBroadcaster};
