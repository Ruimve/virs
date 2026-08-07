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


/* 市场快照：包含当前价格、资金费率、最小交易量和指标 JSON，用于策略决策 */
#[derive(Debug, Clone)]
pub struct MarketSnapshot {
    pub current_price: f64,
    pub funding_rate: f64,
    pub funding_next_time: String,
    pub min_qty: f64,

    /* 指标 JSON：技术指标的序列化结果，结构由策略层定义 */
    pub indicators_json: serde_json::Value,
}
