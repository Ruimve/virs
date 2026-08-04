use crate::order::Side;


#[derive(Debug, Clone)]
pub struct OrderInfo {
    pub id: uuid::Uuid,
    pub position_id: Option<uuid::Uuid>,
    pub symbol: String,
    pub side: Side,
    pub fill_price: Option<f64>,
    pub request_price: Option<f64>,
    pub filled: f64,
    pub client_order_id: Option<String>,

    pub fee: f64,
}


#[derive(Debug, Clone)]
pub struct MarketSnapshot {
    pub current_price: f64,
    pub funding_rate: f64,
    pub funding_next_time: String,
    pub min_qty: f64,

    pub indicators_json: serde_json::Value,
}
