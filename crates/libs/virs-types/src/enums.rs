use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
    #[serde(untagged)]
    Unknown(String),
}


#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionSide {
    #[serde(rename = "LONG")]
    Long,
    #[serde(rename = "SHORT")]
    Short,
    #[serde(untagged)]
    Unknown(String),
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionMode {
    Hedge,
}


#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderType {
    #[serde(rename = "LIMIT")]
    Limit,               // 限价单
    #[serde(rename = "MARKET")]
    Market,              // 市价单
    #[serde(rename = "STOP")]
    Stop,                // 止损限价单
    #[serde(rename = "STOP_MARKET")]
    StopMarket,          // 止损市价单
    #[serde(rename = "TAKE_PROFIT")]
    TakeProfit,          // 止盈限价单
    #[serde(rename = "TAKE_PROFIT_MARKET")]
    TakeProfitMarket,    // 止盈市价单
    #[serde(rename = "TRAILING_STOP_MARKET")]
    TrailingStopMarket,  // 跟踪止损单
    #[serde(rename = "LIQUIDATION")]
    Liquidation,         // 爆仓
    #[serde(untagged)]
    Unknown(String),
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Canceled,
    Failed,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionStatus {
    Opening,
    Open,
    Closing,
    Closed,
}

impl PositionStatus {
    pub fn is_open(&self) -> bool {
        self == &Self::Open
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketType {
    Perpetual,
}

impl std::fmt::Display for MarketType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketType::Perpetual => write!(f, "perpetual"),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineState {
    Created,
    Running,
    ShuttingDown,
    Stopped,
}

impl EngineState {
    pub fn is_running(&self) -> bool {
        self == &Self::Running
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeType {
    Open,
    Close,
}


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


#[cfg(feature = "sqlx")]
mod sqlx_impls {
    use super::*;

    impl<'r> sqlx::Decode<'r, sqlx::Postgres> for Side {
        fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
            let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
            match s {
                "buy" => Ok(Side::Buy),
                "sell" => Ok(Side::Sell),
                other => Ok(Side::Unknown(other.to_string())),
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
                Side::Unknown(other) => other.as_str(),
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
                other => Ok(PositionSide::Unknown(other.to_string())),
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
                PositionSide::Unknown(other) => other.as_str(),
            };
            <&str as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&s, buf)
        }
    }

    impl<'r> sqlx::Decode<'r, sqlx::Postgres> for MarketType {
        fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
            let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
            match s {
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
                MarketType::Perpetual => "perpetual",
            };
            <&str as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&s, buf)
        }
    }
}
