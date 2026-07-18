use virs_models as models;
use virs_types::enums::*;
use virs_types::market::ExchangePosition;

use virs_error::ExchangeError;

use crate::pe_adapter::{
    convert_exchange_position, convert_order_type, convert_position_side,
    convert_to_models_side, convert_virs_position_side, no_exchange_error,
};

#[test]
fn pe2_1_buy() {
    assert_eq!(convert_to_models_side(&Side::Buy), models::Side::Buy);
}

#[test]
fn pe2_2_sell() {
    assert_eq!(convert_to_models_side(&Side::Sell), models::Side::Sell);
}

#[test]
fn pe3_1_long() {
    assert_eq!(
        convert_position_side(&Some(PositionSide::Long)),
        Some(models::PositionSide::Long)
    );
}

#[test]
fn pe3_2_short() {
    assert_eq!(
        convert_position_side(&Some(PositionSide::Short)),
        Some(models::PositionSide::Short)
    );
}

#[test]
fn pe3_4_none() {
    assert_eq!(convert_position_side(&None), None);
}

#[test]
fn pe4_1_market() {
    assert_eq!(
        convert_order_type(&OrderType::Market),
        models::OrderType::Market
    );
}

#[test]
fn pe4_2_limit() {
    assert_eq!(
        convert_order_type(&OrderType::Limit),
        models::OrderType::Limit
    );
}

#[test]
fn pe4_3_stop_market() {
    assert_eq!(
        convert_order_type(&OrderType::StopMarket),
        models::OrderType::StopMarket
    );
}

#[test]
fn pe4_5_take_profit_market() {
    assert_eq!(
        convert_order_type(&OrderType::TakeProfitMarket),
        models::OrderType::TakeProfitMarket
    );
}

#[test]
fn pe6_1_long() {
    assert_eq!(
        convert_virs_position_side(&models::PositionSide::Long),
        PositionSide::Long
    );
}

#[test]
fn pe6_2_short() {
    assert_eq!(
        convert_virs_position_side(&models::PositionSide::Short),
        PositionSide::Short
    );
}

#[test]
fn pe9_1_position_normal() {
    let ep = models::ExchangePosition {
        symbol: "BTC/USDT".into(),
        side: models::PositionSide::Long,
        quantity: 1.0,
        entry_price: 50000.0,
    };
    let result: ExchangePosition = convert_exchange_position(&ep);
    assert_eq!(result.symbol, "BTC/USDT");
    assert_eq!(result.side, PositionSide::Long);
    assert_eq!(result.quantity, 1.0);
    assert_eq!(result.entry_price, 50000.0);
}

#[test]
fn pe9_2_position_no_liquidation() {
    let ep = models::ExchangePosition {
        symbol: "ETH/USDT".into(),
        side: models::PositionSide::Short,
        quantity: 5.0,
        entry_price: 3000.0,
    };
    let result = convert_exchange_position(&ep);
    assert_eq!(result.side, PositionSide::Short);
}

#[test]
fn pe11_1_no_exchange_error() {
    let err = no_exchange_error();
    match err {
        ExchangeError::Internal(msg) => {
            assert!(msg.contains("No perpetual exchange"));
        }
        _ => panic!("Expected Internal variant"),
    }
}
