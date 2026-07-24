use serde::Deserialize;
use virs_types::{PositionSide, WsFeedEvent};

use crate::types::{CcxtOrder, CcxtOrderStatus, ExecutionType as CcxtExecutionType};

// 执行类型枚举(NEW/TRADE/CANCELED/CALCULATED/EXPIRED/AMENDMENT)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionType {
    New,

    Trade,

    Canceled,

    Calculated, // CALCULATED类型为强平事件

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
    pub event_type: String, // e→事件类型

    #[serde(rename = "E")]
    pub event_time: i64, // E→事件时间(ms)

    #[serde(rename = "T")]
    pub transaction_time: i64, // T→事务时间(ms)

    #[serde(rename = "o")]
    pub order: OrderTradeUpdateData, // o→订单成交详情
}

// 订单成交详情，字段映射见各字段注释
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OrderTradeUpdateData {
    #[serde(rename = "s")]
    pub symbol: String, // s→交易对

    #[serde(rename = "c")]
    pub client_order_id: String, // c→客户端订单ID

    #[serde(rename = "S")]
    pub side: String, // S→买卖方向

    #[serde(rename = "o")]
    pub order_type: String, // o→订单类型

    #[serde(rename = "f")]
    pub time_in_force: String, // f→有效期类型

    #[serde(rename = "q")]
    pub orig_qty: String, // q→原始数量

    #[serde(rename = "p")]
    pub original_price: String, // p→原始价格

    #[serde(rename = "ap")]
    pub avg_fill_price: Option<String>, // ap→平均成交价

    #[serde(rename = "sp")]
    pub stop_price: Option<String>, // sp→触发价格

    #[serde(rename = "x")]
    pub execution_type: String, // x→执行类型

    #[serde(rename = "X")]
    pub status: String, // X→订单状态

    #[serde(rename = "i")]
    pub order_id: i64, // i→订单ID

    #[serde(rename = "l")]
    pub last_fill_qty: String, // l→最新成交数量

    #[serde(rename = "z")]
    pub filled_qty: String, // z→累计成交数量

    #[serde(rename = "L")]
    pub last_fill_price: String, // L→最新成交价

    #[serde(rename = "N")]
    pub commission_asset: String, // N→手续费资产

    #[serde(rename = "n")]
    pub commission: String, // n→手续费

    #[serde(rename = "T")]
    pub trade_time: i64, // T→成交时间(ms)

    #[serde(rename = "t")]
    pub trade_id: i64, // t→成交ID

    #[serde(rename = "b")]
    pub bids_notional: Option<String>, // b→买方名义价值

    #[serde(rename = "a")]
    pub ask_notional: Option<String>, // a→卖方名义价值

    #[serde(rename = "m")]
    pub is_maker: bool, // m→是否为maker

    #[serde(rename = "R")]
    pub reduce_only: bool, // R→是否仅减仓 (exchange-native, 业务层不使用)

    #[serde(rename = "wt")]
    pub working_type: Option<String>, // wt→工作类型

    #[serde(rename = "ot")]
    pub original_order_type: Option<String>, // ot→原始订单类型

    #[serde(rename = "ps")]
    pub position_side: Option<String>, // ps→持仓方向

    #[serde(rename = "cp")]
    pub is_close_all: Option<bool>, // cp→是否全部平仓

    #[serde(rename = "AP")]
    pub activation_price: Option<String>, // AP→触发价格(追踪止损)

    #[serde(rename = "cr")]
    pub callback_rate: Option<String>, // cr→回调比率

    #[serde(rename = "pP")]
    pub price_protection: Option<bool>, // pP→价格保护

    #[serde(rename = "rp")]
    pub realized_pnl: Option<String>, // rp→已实现盈亏

    #[serde(rename = "V")]
    pub stp_mode: Option<String>, // V→STP模式

    #[serde(rename = "pm")]
    pub price_match_mode: Option<String>, // pm→价格匹配模式

    #[serde(rename = "gtd")]
    pub gtd_auto_cancel_time: Option<i64>, // gtd→GTD自动撤单时间

    #[serde(rename = "er")]
    pub expiry_reason: Option<String>, // er→过期原因

    #[serde(rename = "si")]
    pub si: Option<i64>, // si→忽略

    #[serde(rename = "ss")]
    pub ss: Option<i64>, // ss→忽略
}

impl OrderTradeUpdateData {
    // 是否强平: execution_type=="CALCULATED"且client_order_id以"autoclose-"开头
    pub fn is_liquidation(&self) -> bool {
        self.execution_type == "CALCULATED" && self.client_order_id.starts_with("autoclose-")
    }

    // 是否ADL(自动减仓): execution_type=="CALCULATED"且client_order_id=="adl_autoclose"
    pub fn is_adl(&self) -> bool {
        self.execution_type == "CALCULATED" && self.client_order_id == "adl_autoclose"
    }

    /// WS 事件合法性校验：在转换为 CcxtOrder 之前，对影响业务逻辑的必需字段做原始字符串校验。
    /// 校验逻辑由 `virs_types::validate_order_fields` 共享函数提供，WS 路径和 DB 读取路径共用。
    ///
    /// 返回 false 时已记录 error 日志，调用方应跳过该订单（return None）。
    pub fn validate(&self) -> bool {
        if let Err(e) = virs_types::validate_order_fields(
            &self.side,
            self.position_side.as_deref(),
            &self.status,
        ) {
            tracing::error!(
                symbol = %self.symbol,
                client_order_id = %self.client_order_id,
                order_id = self.order_id,
                error = %e,
                "WS ORDER_TRADE_UPDATE 字段校验失败，跳过该订单"
            );
            false
        } else {
            true
        }
    }

    // 转换为WsFeedEvent::OrderUpdate
    // 先做合法性校验，通过后再转换为 CcxtOrder
    pub fn to_ws_feed_event(&self) -> Option<WsFeedEvent> {
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

        // 合法性校验：side/position_side/status 不合法则跳过该订单
        if !self.validate() {
            return None;
        }

        let ccxt_order = self.to_ccxt_order();
        Some(WsFeedEvent::OrderUpdate { order: ccxt_order })
    }

    // 转换为 CcxtOrder，字段类型与币安原生返回保持一致
    // 不做任何默认值填充: Option<String> 保持 Option, 未知枚举值保留原始字符串
    pub fn to_ccxt_order(&self) -> CcxtOrder {
        let side = match self.side.as_str() {
            "BUY" => virs_types::Side::Buy,
            "SELL" => virs_types::Side::Sell,
            other => virs_types::Side::Unknown(other.to_string()),
        };

        let order_type =
            crate::adapter::binance::BinanceExchange::parse_order_type(&self.order_type);

        let position_side = match self.position_side.as_deref() {
            Some("LONG") => PositionSide::Long,
            Some("SHORT") => PositionSide::Short,
            Some(other) => PositionSide::Unknown(other.to_string()),
            None => PositionSide::Unknown("None".to_string()),
        };

        let status = CcxtOrderStatus::from_str(&self.status);

        let execution_type = CcxtExecutionType::from_str(&self.execution_type);

        CcxtOrder {
            order_id: self.order_id,
            client_order_id: self.client_order_id.clone(),
            symbol: self.symbol.clone(),
            side,
            order_type,
            position_side,
            original_order_type: self.original_order_type.clone(),
            status,
            execution_type,
            orig_qty: self.orig_qty.clone(),
            original_price: self.original_price.clone(),
            avg_fill_price: self.avg_fill_price.clone(),
            filled_qty: self.filled_qty.clone(),
            last_fill_qty: self.last_fill_qty.clone(),
            last_fill_price: self.last_fill_price.clone(),
            stop_price: self.stop_price.clone(),
            commission: self.commission.clone(),
            commission_asset: self.commission_asset.clone(),
            realized_pnl: self.realized_pnl.clone(),
            reduce_only: self.reduce_only,
            is_maker: self.is_maker,
            close_position: self.is_close_all,
            time_in_force: self.time_in_force.clone(),
            working_type: self.working_type.clone(),
            bids_notional: self.bids_notional.clone(),
            ask_notional: self.ask_notional.clone(),
            activation_price: self.activation_price.clone(),
            callback_rate: self.callback_rate.clone(),
            price_protection: self.price_protection,
            stp_mode: self.stp_mode.clone(),
            price_match_mode: self.price_match_mode.clone(),
            gtd_auto_cancel_time: self.gtd_auto_cancel_time,
            expiry_reason: self.expiry_reason.clone(),
            si: self.si,
            ss: self.ss,
            trade_time: self.trade_time,
            trade_id: self.trade_id,
        }
    }
}

// 入口函数: 反序列化JSON并转换为WsFeedEvent
pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: OrderTradeUpdateEvent = serde_json::from_str(json).ok()?;
    event.order.to_ws_feed_event()
}
