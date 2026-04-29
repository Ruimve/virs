//! 公共测试工具模块

use crate::engine::position::types::*;
use chrono::Utc;
use uuid::Uuid;

/// 创建一个用于测试的 Position 实例。
///
/// `margin` 参数为该仓位的保证金（即 `size * entry_price / leverage`）。
pub fn make_position(
    symbol: &str,
    size: f64,
    entry_price: f64,
    leverage: u32,
) -> Position {
    Position {
        id: Uuid::new_v4(),
        engine_id: "test-engine".to_string(),
        strategy_id: None,
        exchange: "test-exchange".to_string(),
        symbol: symbol.to_string(),
        side: PositionSide::Long,
        status: PositionStatus::Open,
        size,
        entry_price,
        current_price: entry_price,
        leverage,
        margin: size * entry_price / leverage as f64,
        unrealized_pnl: 0.0,
        realized_pnl: 0.0,
        stop_loss: None,
        take_profit: None,
        liquidation_price: None,
        opened_at: Utc::now(),
        updated_at: Utc::now(),
        closed_at: None,
        metadata: serde_json::Value::Null,
    }
}

/// 创建一个带强平价的测试 Position。
pub fn make_position_with_liquidation(
    symbol: &str,
    size: f64,
    entry_price: f64,
    leverage: u32,
    liquidation_price: Option<f64>,
    current_price: f64,
) -> Position {
    let mut pos = make_position(symbol, size, entry_price, leverage);
    pos.liquidation_price = liquidation_price;
    pos.current_price = current_price;
    pos
}

/// 创建一个指定方向的 Position 实例。
pub fn make_position_side(
    symbol: &str,
    side: PositionSide,
    size: f64,
    entry_price: f64,
    leverage: u32,
) -> Position {
    let mut pos = make_position(symbol, size, entry_price, leverage);
    pos.side = side;
    pos
}

/// 创建一个用于测试的 Trade 实例。
pub fn make_trade(
    position_id: Uuid,
    order_id: Uuid,
    side: Side,
    price: f64,
    amount: f64,
    trade_type: &str,
) -> Trade {
    Trade {
        id: Uuid::new_v4(),
        position_id,
        order_id,
        exchange: "test-exchange".to_string(),
        symbol: "BTC/USDT".to_string(),
        side,
        price,
        amount,
        fee: price * amount * 0.0005,
        fee_currency: "USDT".to_string(),
        pnl: 0.0,
        trade_type: trade_type.to_string(),
        created_at: Utc::now(),
    }
}
