use serde::{Deserialize, Serialize};
use std::fmt;


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    #[serde(rename = "1m")]
    M1,
    #[serde(rename = "5m")]
    M5,
    #[serde(rename = "15m")]
    M15,
    #[serde(rename = "1h")]
    H1,
    #[serde(rename = "4h")]
    H4,
    #[serde(rename = "1d")]
    D1,
}

impl Timeframe {
    pub fn all() -> &'static [Timeframe] {
        &[
            Timeframe::M1,
            Timeframe::M5,
            Timeframe::M15,
            Timeframe::H1,
            Timeframe::H4,
            Timeframe::D1,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Timeframe::M1 => "1m",
            Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::H1 => "1h",
            Timeframe::H4 => "4h",
            Timeframe::D1 => "1d",
        }
    }

    pub fn ms(&self) -> i64 {
        match self {
            Timeframe::M1 => 60_000,
            Timeframe::M5 => 300_000,
            Timeframe::M15 => 900_000,
            Timeframe::H1 => 3_600_000,
            Timeframe::H4 => 14_400_000,
            Timeframe::D1 => 86_400_000,
        }
    }

    pub fn default_limit(&self) -> usize {
        1000
    }
}

impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KlineEventType {
    Update,
    Closed,
    Backfilled,
}
