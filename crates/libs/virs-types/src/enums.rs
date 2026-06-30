//! Core enumerations used across the VIRS platform.

use serde::{Deserialize, Serialize};

/// Trade direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

/// Position side (for hedge mode)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionSide {
    Long,
    Short,
    Both,
}

/// Position mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionMode {
    OneWay,
    Hedge,
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderType {
    Limit,
    Market,
    StopMarket,
    StopLimit,
    TakeProfitMarket,
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Canceled,
    Failed,
}

/// Position status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionStatus {
    Empty,
    Opening,
    Open,
    Closing,
    Closed,
}

/// Market type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketType {
    Spot,
    Perpetual,
}

impl std::fmt::Display for MarketType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketType::Spot => write!(f, "spot"),
            MarketType::Perpetual => write!(f, "perpetual"),
        }
    }
}

impl MarketType {
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "perpetual" | "swap" | "future" => MarketType::Perpetual,
            _ => MarketType::Spot,
        }
    }
}

/// Engine state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineState {
    Created,
    Running,
    Paused,
    ShuttingDown,
    Stopped,
}

/// Trade type (open vs close)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeType {
    Open,
    Close,
}

/// Strategy status (for grid bots)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(type_name = "text", rename_all = "lowercase"))]
#[serde(rename_all = "lowercase")]
pub enum StrategyStatus {
    Draft,
    Running,
    Paused,
    Stopped,
    Error,
}

/// User role
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(type_name = "text", rename_all = "lowercase"))]
pub enum UserRole {
    Admin,
    Manager,
    User,
    Viewer,
}

// sqlx Type implementations for enums that need DB mapping
#[cfg(feature = "sqlx")]
mod sqlx_impls {
    use super::*;

    impl<'r> sqlx::Decode<'r, sqlx::Postgres> for Side {
        fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
            let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
            match s {
                "buy" => Ok(Side::Buy),
                "sell" => Ok(Side::Sell),
                _ => Err(format!("unknown Side variant: {}", s).into()),
            }
        }
    }

    impl sqlx::Type<sqlx::Postgres> for Side {
        fn type_info() -> sqlx::postgres::PgTypeInfo {
            sqlx::postgres::PgTypeInfo::with_name("text")
        }
    }

    impl<'q> sqlx::Encode<'q, sqlx::Postgres> for Side {
        fn encode_by_ref(
            &self,
            buf: &mut sqlx::postgres::PgArgumentBuffer,
        ) -> sqlx::encode::IsNull {
            let s = match self {
                Side::Buy => "buy",
                Side::Sell => "sell",
            };
            <&str as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&s, buf)
        }
    }

    impl<'r> sqlx::Decode<'r, sqlx::Postgres> for PositionSide {
        fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
            let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
            match s {
                "long" => Ok(PositionSide::Long),
                "short" => Ok(PositionSide::Short),
                _ => Err(format!("unknown PositionSide variant: {}", s).into()),
            }
        }
    }

    impl sqlx::Type<sqlx::Postgres> for PositionSide {
        fn type_info() -> sqlx::postgres::PgTypeInfo {
            sqlx::postgres::PgTypeInfo::with_name("text")
        }
    }

    impl<'q> sqlx::Encode<'q, sqlx::Postgres> for PositionSide {
        fn encode_by_ref(
            &self,
            buf: &mut sqlx::postgres::PgArgumentBuffer,
        ) -> sqlx::encode::IsNull {
            let s = match self {
                PositionSide::Long => "long",
                PositionSide::Short => "short",
                PositionSide::Both => "both",
            };
            <&str as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&s, buf)
        }
    }

    impl<'r> sqlx::Decode<'r, sqlx::Postgres> for MarketType {
        fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
            let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
            match s {
                "spot" => Ok(MarketType::Spot),
                "perpetual" => Ok(MarketType::Perpetual),
                _ => Err(format!("unknown MarketType variant: {}", s).into()),
            }
        }
    }

    impl sqlx::Type<sqlx::Postgres> for MarketType {
        fn type_info() -> sqlx::postgres::PgTypeInfo {
            sqlx::postgres::PgTypeInfo::with_name("text")
        }
    }

    impl<'q> sqlx::Encode<'q, sqlx::Postgres> for MarketType {
        fn encode_by_ref(
            &self,
            buf: &mut sqlx::postgres::PgArgumentBuffer,
        ) -> sqlx::encode::IsNull {
            let s = match self {
                MarketType::Spot => "spot",
                MarketType::Perpetual => "perpetual",
            };
            <&str as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&s, buf)
        }
    }
}
