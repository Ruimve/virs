use chrono::{DateTime, Utc};
use serde::Deserialize;
use virs_types::{OrderStatus, PositionSide, WsFeedEvent};


// 执行类型枚举(NEW/TRADE/CANCELED/CALCULATED/EXPIRED/AMENDMENT)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionType {

    New,

    Trade,

    Canceled,

    Calculated,  // CALCULATED类型为强平事件

    Expired,

    Amendment,

    Unknown(String),
}

impl ExecutionType {
    // 从字符串解析执行类型
    pub fn from_str(s: &str) -> Self {
        match s {
            "NEW" => Self::New,
            "TRADE" => Self::Trade,
            "CANCELED" => Self::Canceled,
            "CALCULATED" => Self::Calculated,
            "EXPIRED" => Self::Expired,
            "AMENDMENT" => Self::Amendment,
            other => Self::Unknown(other.to_string()),
        }
    }

    // CALCULATED类型为强平事件
    pub fn is_liquidation(&self) -> bool {
        matches!(self, Self::Calculated)
    }
}


// ORDER_TRADE_UPDATE事件外层结构
#[derive(Debug, Clone, Deserialize)]
pub struct OrderTradeUpdateEvent {

    #[serde(rename = "e")]
    pub event_type: String,  // e→事件类型

    #[serde(rename = "E")]
    pub event_time: i64,  // E→事件时间(ms)

    #[serde(rename = "T")]
    pub transaction_time: i64,  // T→事务时间(ms)

    #[serde(rename = "o")]
    pub order: OrderTradeUpdateData,  // o→订单成交详情
}


// 订单成交详情，字段映射见各字段注释
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OrderTradeUpdateData {

    #[serde(rename = "s")]
    pub symbol: String,  // s→交易对

    #[serde(rename = "c")]
    pub client_order_id: String,  // c→客户端订单ID

    #[serde(rename = "S")]
    pub side: String,  // S→买卖方向

    #[serde(rename = "o")]
    pub order_type: String,  // o→订单类型

    #[serde(rename = "f")]
    pub time_in_force: String,  // f→有效期类型

    #[serde(rename = "q")]
    pub orig_qty: String,  // q→原始数量

    #[serde(rename = "p")]
    pub original_price: String,  // p→原始价格

    #[serde(rename = "ap")]
    pub avg_fill_price: Option<String>,  // ap→平均成交价

    #[serde(rename = "sp")]
    pub stop_price: Option<String>,  // sp→触发价格

    #[serde(rename = "x")]
    pub execution_type: String,  // x→执行类型

    #[serde(rename = "X")]
    pub status: String,  // X→订单状态

    #[serde(rename = "i")]
    pub order_id: i64,  // i→订单ID

    #[serde(rename = "l")]
    pub last_fill_qty: String,  // l→最新成交数量

    #[serde(rename = "z")]
    pub filled_qty: String,  // z→累计成交数量

    #[serde(rename = "L")]
    pub last_fill_price: String,  // L→最新成交价

    #[serde(rename = "N")]
    pub commission_asset: String,  // N→手续费资产

    #[serde(rename = "n")]
    pub commission: String,  // n→手续费

    #[serde(rename = "T")]
    pub trade_time: i64,  // T→成交时间(ms)

    #[serde(rename = "t")]
    pub trade_id: i64,  // t→成交ID

    #[serde(rename = "b")]
    pub bids_notional: Option<String>,  // b→买方名义价值

    #[serde(rename = "a")]
    pub ask_notional: Option<String>,  // a→卖方名义价值

    #[serde(rename = "m")]
    pub is_maker: bool,  // m→是否为maker

    #[serde(rename = "R")]
    pub is_reduce_only: bool,  // R→是否仅减仓

    #[serde(rename = "wt")]
    pub working_type: Option<String>,  // wt→工作类型

    #[serde(rename = "ot")]
    pub original_order_type: Option<String>,  // ot→原始订单类型

    #[serde(rename = "ps")]
    pub position_side: Option<String>,  // ps→持仓方向

    #[serde(rename = "cp")]
    pub is_close_all: Option<bool>,  // cp→是否全部平仓

    #[serde(rename = "AP")]
    pub activation_price: Option<String>,  // AP→触发价格(追踪止损)

    #[serde(rename = "cr")]
    pub callback_rate: Option<String>,  // cr→回调比率

    #[serde(rename = "pP")]
    pub price_protection: Option<bool>,  // pP→价格保护

    #[serde(rename = "rp")]
    pub realized_pnl: Option<String>,  // rp→已实现盈亏

    #[serde(rename = "V")]
    pub stp_mode: Option<String>,  // V→STP模式

    #[serde(rename = "pm")]
    pub price_match_mode: Option<String>,  // pm→价格匹配模式

    #[serde(rename = "gtd")]
    pub gtd_auto_cancel_time: Option<i64>,  // gtd→GTD自动撤单时间

    #[serde(rename = "er")]
    pub expiry_reason: Option<String>,  // er→过期原因
}

impl OrderTradeUpdateData {

    // 订单状态映射: NEW→Open, PARTIALLY_FILLED→PartiallyFilled, FILLED→Filled,
    // CANCELED/EXPIRED/EXPIRED_IN_MATCH→Canceled, REJECTED→Failed
    fn to_order_status(&self) -> Option<OrderStatus> {
        match self.status.as_str() {
            "NEW" => Some(OrderStatus::Open),
            "PARTIALLY_FILLED" => Some(OrderStatus::PartiallyFilled),
            "FILLED" => Some(OrderStatus::Filled),
            "CANCELED" => Some(OrderStatus::Canceled),
            "EXPIRED" => Some(OrderStatus::Canceled),
            "EXPIRED_IN_MATCH" => Some(OrderStatus::Canceled),
            "REJECTED" => Some(OrderStatus::Failed),
            _ => None,
        }
    }

    // 解析持仓方向 LONG/SHORT
    fn to_position_side(&self) -> Option<PositionSide> {
        self.position_side.as_ref().and_then(|ps| match ps.as_str() {
            "LONG" => Some(PositionSide::Long),
            "SHORT" => Some(PositionSide::Short),
            _ => None,
        })
    }

    // 是否强平: execution_type=="CALCULATED"且client_order_id以"autoclose-"开头
    pub fn is_liquidation(&self) -> bool {
        self.execution_type == "CALCULATED" && self.client_order_id.starts_with("autoclose-")
    }

    // 是否ADL(自动减仓): execution_type=="CALCULATED"且client_order_id=="adl_autoclose"
    pub fn is_adl(&self) -> bool {
        self.execution_type == "CALCULATED" && self.client_order_id == "adl_autoclose"
    }

    // 转换为WsFeedEvent::OrderUpdate，价格优先用avg_fill_price，回退last_fill_price
    pub fn to_ws_feed_event(&self) -> Option<WsFeedEvent> {
        let status = self.to_order_status()?;
        let execution_type = ExecutionType::from_str(&self.execution_type);
        let position_side = self.to_position_side();

        // 检测强平和ADL事件并记录日志
        if self.is_liquidation() {
            tracing::error!(
                symbol = %self.symbol,
                order_id = self.order_id,
                client_order_id = %self.client_order_id,
                "强制平仓事件 — 仓位已被交易所强平"
            );
        } else if self.is_adl() {
            tracing::error!(
                symbol = %self.symbol,
                order_id = self.order_id,
                client_order_id = %self.client_order_id,
                "ADL 事件 — 仓位被自动减仓"
            );
        }

        // 解析已成交数量，失败则跳过事件
        let filled = self.filled_qty.parse::<f64>().unwrap_or_else(|e| {
            tracing::error!(
                filled_qty = %self.filled_qty,
                error = %e,
                "解析 filled_qty 失败 — 跳过事件以避免 0.0 传播"
            );
            f64::NAN
        });
        if filled.is_nan() {
            return None;
        }

        // 解析原始订单数量，失败则跳过事件
        let amount = self.orig_qty.parse::<f64>().unwrap_or_else(|e| {
            tracing::error!(
                orig_qty = %self.orig_qty,
                error = %e,
                "解析 orig_qty 失败 — 跳过事件以避免 0.0 传播"
            );
            f64::NAN
        });
        if amount.is_nan() {
            return None;
        }

        // 剩余数量 = 原始数量 - 已成交数量
        let remaining = (amount - filled).max(0.0);

        // 成交价: 优先avg_fill_price，回退last_fill_price
        let price = match self
            .avg_fill_price
            .as_ref()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|&p| p > 0.0)
        {
            Some(p) => p,
            None => match self.last_fill_price.parse::<f64>() {
                Ok(p) if p > 0.0 => p,
                Ok(_) => {
                    tracing::warn!(
                        last_fill_price = %self.last_fill_price,
                        symbol = %self.symbol,
                        "last_fill_price 为 0.0 — 订单可能尚未成交"
                    );
                    0.0
                }
                Err(e) => {
                    tracing::error!(
                        last_fill_price = %self.last_fill_price,
                        error = %e,
                        "解析 last_fill_price 失败 — 跳过事件以避免 0.0 传播"
                    );
                    return None;
                }
            },
        };

        // 手续费，解析失败则跳过事件
        let commission = match self.commission.parse::<f64>() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(
                    commission = %self.commission,
                    error = %e,
                    "解析 commission 失败 — 跳过事件以避免 0.0 传播"
                );
                return None;
            }
        };

        // 以下字段待WsFeedEvent扩展后传递到下游
        let last_fill_qty = self.last_fill_qty.parse::<f64>().unwrap_or(0.0);

        let realized_pnl = self
            .realized_pnl
            .as_ref()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        let trade_id = if self.trade_id > 0 {
            Some(self.trade_id)
        } else {
            None
        };

        let timestamp = DateTime::from_timestamp_millis(self.trade_time).unwrap_or_else(|| {
            tracing::warn!(
                trade_time = self.trade_time,
                symbol = %self.symbol,
                order_id = self.order_id,
                "WS order trade_time 无效 — 使用本地时间作为 fallback"
            );
            Utc::now()
        });

        tracing::debug!(
            client_order_id = %self.client_order_id,
            execution_type = ?execution_type,
            last_fill_qty = last_fill_qty,
            realized_pnl = realized_pnl,
            trade_id = ?trade_id,
            is_reduce_only = self.is_reduce_only,
            is_maker = self.is_maker,
            "ORDER_TRADE_UPDATE 额外字段（待 WsFeedEvent 扩展后传递到下游）"
        );

        Some(WsFeedEvent::OrderUpdate {
            exchange_order_id: self.order_id.to_string(),
            client_order_id: Some(self.client_order_id.clone()),
            symbol: self.symbol.clone(),
            status,
            filled,
            remaining,
            price,
            amount,
            commission,
            timestamp,
            position_side,
        })
    }
}


// 入口函数: 反序列化JSON并转换为WsFeedEvent
pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: OrderTradeUpdateEvent = serde_json::from_str(json).ok()?;
    event.order.to_ws_feed_event()
}
