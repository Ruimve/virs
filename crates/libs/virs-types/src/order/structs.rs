use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use virs_error::ExchangeError;

use crate::order::{OrderStatus, OrderType};
use crate::position::PositionSide;
use crate::order::Side;


// 下单/撤单返回结果 (仅提取 orderId + clientOrderId)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResult {
    pub order_id: String,
    pub client_order_id: String,
}


// WS ORDER_TRADE_UPDATE 订单结构体
// 完整映射币安官方文档中 o 对象的 37 个字段
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CcxtOrder {
    // --- 订单标识 ---
    pub order_id: i64,
    pub client_order_id: String,

    // --- 订单基本信息 ---
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub position_side: PositionSide,
    pub original_order_type: Option<String>,
    pub status: crate::order::enums::CcxtOrderStatus,
    pub execution_type: crate::order::enums::ExecutionType,

    // --- 价格与数量 (币安返回字符串，保持原样) ---
    pub orig_qty: String,
    pub original_price: String,
    pub avg_fill_price: Option<String>,
    pub filled_qty: String,
    pub last_fill_qty: String,
    pub last_fill_price: String,
    pub stop_price: Option<String>,

    // --- 手续费与盈亏 ---
    pub commission: String,
    pub commission_asset: String,
    pub realized_pnl: Option<String>,

    // --- 订单属性 ---
    pub reduce_only: bool,
    pub is_maker: bool,
    pub close_position: Option<bool>,
    pub time_in_force: String,
    pub working_type: Option<String>,

    // --- 名义价值 ---
    pub bids_notional: Option<String>,
    pub ask_notional: Option<String>,

    // --- 追踪止损 ---
    pub activation_price: Option<String>,
    pub callback_rate: Option<String>,

    // --- 价格保护与模式 ---
    pub price_protection: Option<bool>,
    pub stp_mode: Option<String>,
    pub price_match_mode: Option<String>,
    pub gtd_auto_cancel_time: Option<i64>,
    pub expiry_reason: Option<String>,

    // --- 忽略字段 ---
    pub si: Option<i64>,
    pub ss: Option<i64>,

    // --- 时间与成交ID ---
    pub trade_time: i64,
    pub trade_id: i64,

    // --- 改单标识 ---
    pub modify_id: Option<String>,

    // --- 信封字段 ---
    pub envelope_event_type: String,
    pub envelope_event_time: i64,
    pub envelope_transaction_time: i64,
}

impl CcxtOrder {
    /// 校验订单必需字段的合法性。WS 事件入口和 DB 读取路径共用此函数。
    ///
    /// 在转换为 CcxtOrder 之前对原始字符串做校验：
    /// - side: 必需 BUY/SELL
    /// - position_side: Hedge 模式下必需 LONG/SHORT
    /// - status: 必需已知状态
    pub fn validate_fields(
        side: &str,
        position_side: Option<&str>,
        status: &str,
    ) -> Result<(), ExchangeError> {
        Self::validate_side(side)?;
        Self::validate_position_side(position_side)?;
        Self::validate_status(status)?;
        Ok(())
    }

    /// 校验 side (S) 字段：必需 BUY/SELL。
    fn validate_side(side: &str) -> Result<(), ExchangeError> {
        match side {
            "BUY" | "SELL" => Ok(()),
            other => Err(ExchangeError::InvalidOrderField(format!(
                "side 非法 (非 BUY/SELL): {other}"
            ))),
        }
    }

    /// 校验 position_side (ps) 字段：Hedge 模式下必需 LONG/SHORT。
    pub fn validate_position_side(position_side: Option<&str>) -> Result<(), ExchangeError> {
        match position_side {
            Some("LONG") | Some("SHORT") => Ok(()),
            Some(other) => Err(ExchangeError::InvalidOrderField(format!(
                "position_side 非法 (非 LONG/SHORT): {other}"
            ))),
            None => Err(ExchangeError::InvalidOrderField(
                "position_side 为 None (OneWay 模式?)".to_string(),
            )),
        }
    }

    /// 校验 status (X) 字段：必需已知状态。
    fn validate_status(status: &str) -> Result<(), ExchangeError> {
        match status {
            "NEW" | "PARTIALLY_FILLED" | "FILLED" | "CANCELED" | "CANCELLED" | "EXPIRED"
            | "EXPIRED_IN_MATCH" => Ok(()),
            other => Err(ExchangeError::InvalidOrderField(format!(
                "status 未知: {other}"
            ))),
        }
    }
}


/// 订单 DB 行映射（原 virs-models::trading::Order）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub client_order_id: Option<String>,
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<f64>,
    pub amount: f64,
    pub cost: Option<f64>,
    pub filled: f64,
    pub remaining: f64,
    pub status: OrderStatus,
    pub fee: f64,
    pub fee_currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
