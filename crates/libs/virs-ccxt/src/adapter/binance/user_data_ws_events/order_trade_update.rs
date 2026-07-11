//! ORDER_TRADE_UPDATE — 订单交易更新推送
//!
//! 官方描述: 当有新订单创建、订单有新成交或者新的状态变化时会推送此类事件。
//! 推送机制: 每次状态变化推送一条事件（分次返回，非一次性返回整个 order）。
//!
//! 官方文档: https://developers.binance.com/zh-CN/docs/products/derivatives-trading-usds-futures/user-data-streams

use chrono::{DateTime, Utc};
use serde::Deserialize;
use virs_types::{OrderStatus, PositionSide, WsFeedEvent};

/// ORDER_TRADE_UPDATE 的执行类型 (x 字段)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionType {
    /// 新建订单
    New,
    /// 成交（每笔成交推送一次）
    Trade,
    /// 已撤
    Canceled,
    /// 订单 ADL 或爆仓
    Calculated,
    /// 订单失效
    Expired,
    /// 订单修改
    Amendment,
    /// 未知类型
    Unknown(String),
}

impl ExecutionType {
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

    /// 是否为强平/ADL 事件
    pub fn is_liquidation(&self) -> bool {
        matches!(self, Self::Calculated)
    }
}

/// ORDER_TRADE_UPDATE 完整事件
#[derive(Debug, Clone, Deserialize)]
pub struct OrderTradeUpdateEvent {
    /// 事件类型
    #[serde(rename = "e")]
    pub event_type: String,
    /// 事件时间
    #[serde(rename = "E")]
    pub event_time: i64,
    /// 撮合时间
    #[serde(rename = "T")]
    pub transaction_time: i64,
    /// 订单数据
    #[serde(rename = "o")]
    pub order: OrderTradeUpdateData,
}

/// ORDER_TRADE_UPDATE 中的订单数据 (o 字段)
///
/// 字段命名严格遵循币安官方文档，注释为官方中文释义。
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OrderTradeUpdateData {
    /// 订单符号 Symbol
    #[serde(rename = "s")]
    pub symbol: String,
    /// 客户端订单 ID Client Order Id
    #[serde(rename = "c")]
    pub client_order_id: String,
    /// 订单方向 Side: BUY / SELL
    #[serde(rename = "S")]
    pub side: String,
    /// 订单类型 Order Type: LIMIT / MARKET / STOP / ...
    #[serde(rename = "o")]
    pub order_type: String,
    /// 有效方式 Time in Force: GTC / IOC / FOK / GTX
    #[serde(rename = "f")]
    pub time_in_force: String,
    /// 原始订单数量 Original Quantity
    #[serde(rename = "q")]
    pub orig_qty: String,
    /// 原始价格 Original Price
    #[serde(rename = "p")]
    pub original_price: String,
    /// 累计成交均价 Average Price (累计维度)
    #[serde(rename = "ap")]
    pub avg_fill_price: Option<String>,
    /// 止损价 Stop Price
    #[serde(rename = "sp")]
    pub stop_price: Option<String>,
    /// 本次事件的具体执行类型 Execution Type: NEW / TRADE / CANCELED / CALCULATED / EXPIRED / AMENDMENT
    #[serde(rename = "x")]
    pub execution_type: String,
    /// 订单状态 Order Status: NEW / PARTIALLY_FILLED / FILLED / CANCELED / EXPIRED / EXPIRED_IN_MATCH
    #[serde(rename = "X")]
    pub status: String,
    /// 订单 ID Order Id
    #[serde(rename = "i")]
    pub order_id: i64,
    /// 本次成交数量 Order Last Filled Quantity (本笔维度)
    #[serde(rename = "l")]
    pub last_fill_qty: String,
    /// 累计已成交数量 Order Filled Accumulated Quantity (累计维度)
    #[serde(rename = "z")]
    pub filled_qty: String,
    /// 本次成交价 Last Filled Price (本笔维度)
    #[serde(rename = "L")]
    pub last_fill_price: String,
    /// 手续费资产 Commission Asset
    #[serde(rename = "N")]
    pub commission_asset: String,
    /// 本次手续费 Commission (本笔维度，非累计)
    #[serde(rename = "n")]
    pub commission: String,
    /// 订单成交时间 Order Trade Time
    #[serde(rename = "T")]
    pub trade_time: i64,
    /// 成交 ID Trade Id (用于去重)
    #[serde(rename = "t")]
    pub trade_id: i64,
    /// 买单名义价值 Bids Notional
    #[serde(rename = "b")]
    pub bids_notional: Option<String>,
    /// 卖单名义价值 Ask Notional
    #[serde(rename = "a")]
    pub ask_notional: Option<String>,
    /// 是否为 maker Is this trade the maker side?
    #[serde(rename = "m")]
    pub is_maker: bool,
    /// 是否只减仓 Is this reduce only
    #[serde(rename = "R")]
    pub is_reduce_only: bool,
    /// 止损价工作类型 Stop Price Working Type: MARK_PRICE / CONTRACT_PRICE
    #[serde(rename = "wt")]
    pub working_type: Option<String>,
    /// 原始订单类型 Original Order Type
    #[serde(rename = "ot")]
    pub original_order_type: Option<String>,
    /// 持仓方向 Position Side: LONG / SHORT / BOTH
    #[serde(rename = "ps")]
    pub position_side: Option<String>,
    /// 是否全平 If Close-All (条件单)
    #[serde(rename = "cp")]
    pub is_close_all: Option<bool>,
    /// 激活价格 Activation Price (仅 TRAILING_STOP_MARKET)
    #[serde(rename = "AP")]
    pub activation_price: Option<String>,
    /// 回调比例 Callback Rate (仅 TRAILING_STOP_MARKET)
    #[serde(rename = "cr")]
    pub callback_rate: Option<String>,
    /// 价格保护 If price protection is turned on
    #[serde(rename = "pP")]
    pub price_protection: Option<bool>,
    /// 本笔成交已实现盈亏 Realized Profit of the trade (本笔维度)
    #[serde(rename = "rp")]
    pub realized_pnl: Option<String>,
    /// STP 模式 STP mode
    #[serde(rename = "V")]
    pub stp_mode: Option<String>,
    /// 价格匹配模式 Price match mode
    #[serde(rename = "pm")]
    pub price_match_mode: Option<String>,
    /// GTD 订单自动取消时间 TIF GTD order auto cancel time
    #[serde(rename = "gtd")]
    pub gtd_auto_cancel_time: Option<i64>,
    /// 过期原因 Expiry Reason (0-9)
    #[serde(rename = "er")]
    pub expiry_reason: Option<String>,
}

impl OrderTradeUpdateData {
    /// 将币安订单状态映射为 Position Engine 的 OrderStatus
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

    /// 解析持仓方向
    fn to_position_side(&self) -> Option<PositionSide> {
        self.position_side.as_ref().and_then(|ps| match ps.as_str() {
            "LONG" => Some(PositionSide::Long),
            "SHORT" => Some(PositionSide::Short),
            _ => None,
        })
    }

    /// 判断是否为强平事件
    ///
    /// 官方文档: 若用户因保证金不足发生强平，c 为 "autoclose-XXX"，X 为 "NEW"
    pub fn is_liquidation(&self) -> bool {
        self.execution_type == "CALCULATED" && self.client_order_id.starts_with("autoclose-")
    }

    /// 判断是否为 ADL 事件
    ///
    /// 官方文档: 若用户保证金充足但被 ADL，c 为 "adl_autoclose"，X 为 "NEW"
    pub fn is_adl(&self) -> bool {
        self.execution_type == "CALCULATED" && self.client_order_id == "adl_autoclose"
    }

    /// 转换为 WsFeedEvent::OrderUpdate
    ///
    /// 关键数值字段（filled/amount/price/commission）解析失败时返回 None，
    /// 跳过该事件而非传播 0.0，避免订单状态判断错误和 PnL 计算偏差。
    ///
    /// 注意: `commission` (n 字段) 是**本笔手续费**（非累计），下游需累加。
    /// 注意: `last_fill_qty` (l 字段) 是**本笔成交量**（非累计）。
    /// 注意: `filled_qty` (z 字段) 是**累计成交量**。
    /// 注意: `avg_fill_price` (ap 字段) 是**累计成交均价**。
    pub fn to_ws_feed_event(&self) -> Option<WsFeedEvent> {
        let status = self.to_order_status()?;
        let execution_type = ExecutionType::from_str(&self.execution_type);
        let position_side = self.to_position_side();

        // 强平/ADL 事件特殊日志
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

        // 累计成交量 (z 字段) — 解析失败跳过事件
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

        // 原始订单数量 (q 字段) — 解析失败跳过事件
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

        let remaining = (amount - filled).max(0.0);

        // 累计成交均价 (ap 字段)，为 0 时 fallback 到本次成交价 (L 字段)
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

        // 本次手续费 (n 字段) — 本笔维度，非累计
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

        // 本笔成交量 (l 字段) — 本笔维度
        let last_fill_qty = self.last_fill_qty.parse::<f64>().unwrap_or(0.0);

        // 本笔已实现盈亏 (rp 字段) — 本笔维度
        let realized_pnl = self
            .realized_pnl
            .as_ref()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        // 成交 ID (t 字段) — 用于去重
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

        // 记录当前 WsFeedEvent 枚举尚不支持的字段（待 WsFeedEvent 扩展后传递）
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

/// 处理 ORDER_TRADE_UPDATE 原始 JSON
pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: OrderTradeUpdateEvent = serde_json::from_str(json).ok()?;
    event.order.to_ws_feed_event()
}
