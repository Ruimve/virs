use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarginMode {
    Cross,
    Isolated,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionMode {
    Hedge,
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


// ── sqlx 编解码：MarketType ──

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
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let s = match self {
            MarketType::Perpetual => "perpetual",
        };
        <&str as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&s, buf)
    }
}
