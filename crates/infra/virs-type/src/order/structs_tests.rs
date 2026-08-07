use chrono::Utc;

use super::*;
use crate::order::{OrderStatus, OrderType, Side};


#[test]
fn s3_1_order_roundtrip() {
    let now = Utc::now();
    let order = Order {
        id: "order_123".into(),
        client_order_id: Some("client_456".into()),
        symbol: "BTCUSDT".into(),
        side: Side::Buy,
        order_type: OrderType::Limit,
        price: Some(50000.0),
        amount: 1.0,
        cost: Some(50000.0),
        filled: 0.5,
        remaining: 0.5,
        status: OrderStatus::PartiallyFilled,
        fee: 0.075,
        fee_currency: "BTC".into(),
        created_at: now,
        updated_at: now,
    };
    let json = serde_json::to_string(&order).unwrap();
    let de: Order = serde_json::from_str(&json).unwrap();
    assert_eq!(de, order);
}
