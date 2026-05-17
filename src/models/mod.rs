pub mod common;
pub mod grid;
pub mod market;
pub mod trading;
pub mod user;

// Re-export all types for backward compatibility
pub use common::{ApiResponse, MarketType, PaginationParams};
pub use grid::{GridBot, GridTrade, StrategyStatus};
pub use market::{Balance, ExchangePosition, FundingHistoryEntry, FundingRate, Kline, OrderBook, Ticker};
pub use trading::{Order, OrderStatus, OrderType, PositionMode, PositionSide, Side};
pub use user::{CreateUserRequest, LoginRequest, LoginResponse, User, UserRole, UserResponse};
