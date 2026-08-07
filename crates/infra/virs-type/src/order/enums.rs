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
pub enum OrderType {
    #[serde(rename = "LIMIT")]
    Limit,
    #[serde(rename = "MARKET")]
    Market,
    #[serde(rename = "STOP")]
    Stop,
    #[serde(rename = "STOP_MARKET")]
    StopMarket,
    #[serde(rename = "TAKE_PROFIT")]
    TakeProfit,
    #[serde(rename = "TAKE_PROFIT_MARKET")]
    TakeProfitMarket,
    #[serde(rename = "TRAILING_STOP_MARKET")]
    TrailingStopMarket,
    #[serde(rename = "LIQUIDATION")]
    Liquidation,
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
    Expired,
    Failed,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeInForce {

    #[serde(rename = "GTC")]
    Gtc,

    #[serde(rename = "IOC")]
    Ioc,

    #[serde(rename = "FOK")]
    Fok,

    #[serde(rename = "GTX")]
    Gtx,

    #[serde(rename = "GTD")]
    Gtd,
}


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
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let s = match self {
            Side::Buy => "buy",
            Side::Sell => "sell",
            Side::Unknown(other) => other.as_str(),
        };
        <&str as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&s, buf)
    }
}


impl<'r> sqlx::Decode<'r, sqlx::Postgres> for OrderType {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Ok(match s {
            "LIMIT" => OrderType::Limit,
            "MARKET" => OrderType::Market,
            "STOP" => OrderType::Stop,
            "STOP_MARKET" => OrderType::StopMarket,
            "TAKE_PROFIT" => OrderType::TakeProfit,
            "TAKE_PROFIT_MARKET" => OrderType::TakeProfitMarket,
            "TRAILING_STOP_MARKET" => OrderType::TrailingStopMarket,
            "LIQUIDATION" => OrderType::Liquidation,
            other => OrderType::Unknown(other.to_string()),
        })
    }
}

impl sqlx::Type<sqlx::Postgres> for OrderType {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("text")
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for OrderType {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let s = match self {
            OrderType::Limit => "LIMIT",
            OrderType::Market => "MARKET",
            OrderType::Stop => "STOP",
            OrderType::StopMarket => "STOP_MARKET",
            OrderType::TakeProfit => "TAKE_PROFIT",
            OrderType::TakeProfitMarket => "TAKE_PROFIT_MARKET",
            OrderType::TrailingStopMarket => "TRAILING_STOP_MARKET",
            OrderType::Liquidation => "LIQUIDATION",
            OrderType::Unknown(other) => other.as_str(),
        };
        <&str as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&s, buf)
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutionType {
    #[serde(rename = "NEW")]
    New,
    #[serde(rename = "TRADE")]
    Trade,
    #[serde(rename = "CANCELED")]
    Canceled,
    #[serde(rename = "CALCULATED")]
    Calculated,
    #[serde(rename = "EXPIRED")]
    Expired,
    #[serde(rename = "AMENDMENT")]
    Amendment,
    #[serde(untagged)]
    Unknown(String),
}

impl std::str::FromStr for ExecutionType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "NEW" => Self::New,
            "TRADE" => Self::Trade,
            "CANCELED" => Self::Canceled,
            "CALCULATED" => Self::Calculated,
            "EXPIRED" => Self::Expired,
            "AMENDMENT" => Self::Amendment,
            other => Self::Unknown(other.to_string()),
        })
    }
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CcxtOrderStatus {
    #[serde(rename = "NEW")]
    New,
    #[serde(rename = "PARTIALLY_FILLED")]
    PartiallyFilled,
    #[serde(rename = "FILLED")]
    Filled,
    #[serde(rename = "CANCELED")]
    Canceled,
    #[serde(rename = "EXPIRED")]
    Expired,
    #[serde(rename = "EXPIRED_IN_MATCH")]
    ExpiredInMatch,
    #[serde(untagged)]
    Unknown(String),
}

impl std::str::FromStr for CcxtOrderStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "NEW" => Self::New,
            "PARTIALLY_FILLED" => Self::PartiallyFilled,
            "FILLED" => Self::Filled,
            "CANCELED" | "CANCELLED" => Self::Canceled,
            "EXPIRED" => Self::Expired,
            "EXPIRED_IN_MATCH" => Self::ExpiredInMatch,
            other => Self::Unknown(other.to_string()),
        })
    }
}

impl From<CcxtOrderStatus> for OrderStatus {
    fn from(s: CcxtOrderStatus) -> Self {
        match s {
            CcxtOrderStatus::New => OrderStatus::Open,
            CcxtOrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
            CcxtOrderStatus::Filled => OrderStatus::Filled,
            CcxtOrderStatus::Canceled => OrderStatus::Canceled,
            CcxtOrderStatus::Expired | CcxtOrderStatus::ExpiredInMatch => OrderStatus::Expired,
            CcxtOrderStatus::Unknown(_) => OrderStatus::Failed,
        }
    }
}
