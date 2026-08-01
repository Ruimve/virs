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


#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeInForce {
    Gtc,
    Ioc,
    Fok,
    Poc,
}


// ── sqlx 编解码：Side ──

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


// ── WS 事件枚举 ──

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

impl ExecutionType {
    pub fn from_str(s: &str) -> Self {
        match s {
            "NEW" => Self::New,
            "TRADE" => Self::Trade,
            "CANCELED" => Self::Canceled,
            "CALCULATED" => Self::Calculated,
            "EXPIRED" => Self::Expired,
            "AMENDMENT" => Self::Amendment,
            other => Self::Unknown(other.to_string()),
        }
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

impl CcxtOrderStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "NEW" => Self::New,
            "PARTIALLY_FILLED" => Self::PartiallyFilled,
            "FILLED" => Self::Filled,
            "CANCELED" | "CANCELLED" => Self::Canceled,
            "EXPIRED" => Self::Expired,
            "EXPIRED_IN_MATCH" => Self::ExpiredInMatch,
            other => Self::Unknown(other.to_string()),
        }
    }
}

impl From<CcxtOrderStatus> for OrderStatus {
    fn from(s: CcxtOrderStatus) -> Self {
        match s {
            CcxtOrderStatus::New => OrderStatus::Open,
            CcxtOrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
            CcxtOrderStatus::Filled => OrderStatus::Filled,
            CcxtOrderStatus::Canceled => OrderStatus::Canceled,
            CcxtOrderStatus::Expired => OrderStatus::Canceled,
            CcxtOrderStatus::ExpiredInMatch => OrderStatus::Canceled,
            CcxtOrderStatus::Unknown(_) => {
                unreachable!("CcxtOrder::validate_fields ensures status is known before CcxtOrderStatus is created")
            }
        }
    }
}
