use serde::{Deserialize, Serialize};


pub use virs_type::MarketInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFee {
    pub cost: f64,
    pub currency: String,
    pub rate: Option<f64>,
}
