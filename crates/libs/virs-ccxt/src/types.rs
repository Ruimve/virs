use serde::{Deserialize, Serialize};

// MarketInfo 已迁移至 virs_type，此处通过 re-export 保持兼容
pub use virs_type::MarketInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFee {
    pub cost: f64,
    pub currency: String,
    pub rate: Option<f64>,
}
