use crate::grid::types::GridLevel;
use uuid::Uuid;

fn make_level() -> GridLevel {
    GridLevel {
        level: 3,
        price: 105.0,
        side: "buy".to_string(),
        buy_price: 105.0,
        sell_price: 106.0,
        quantity: 1.5,
        buy_order_id: Some(Uuid::new_v4()),
        sell_order_id: Some(Uuid::new_v4()),
        buy_filled: true,
        sell_filled: false,
        hold_quantity: 1.5,
        avg_buy_price: 104.5,
        last_fill_price: Some(105.0),
        open_client_order_id: Some("test-order-id".to_string()),
    }
}

#[test]
fn t1_1_reset_for_relist_clears_state() {
    let level = make_level();
    let reset = level.reset_for_relist();
    assert!(reset.buy_order_id.is_none());
    assert!(reset.sell_order_id.is_none());
    assert!(!reset.buy_filled);
    assert!(!reset.sell_filled);
    assert!((reset.hold_quantity - 0.0).abs() < 1e-10);
    assert!((reset.avg_buy_price - 0.0).abs() < 1e-10);
    assert!(reset.last_fill_price.is_none());
    assert!(reset.open_client_order_id.is_none());
}

#[test]
fn t1_2_reset_for_relist_preserves_config() {
    let level = make_level();
    let reset = level.reset_for_relist();
    assert_eq!(reset.level, 3);
    assert!((reset.price - 105.0).abs() < 1e-10);
    assert_eq!(reset.side, "buy");
    assert!((reset.buy_price - 105.0).abs() < 1e-10);
    assert!((reset.sell_price - 106.0).abs() < 1e-10);
    assert!((reset.quantity - 1.5).abs() < 1e-10);
}
