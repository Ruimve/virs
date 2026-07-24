use serde::{Deserialize, Serialize};

use crate::enums::{OrderStatus, OrderType, PositionSide, Side};


// 下单/撤单返回结果 (仅提取 orderId + clientOrderId)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResult {
    pub order_id: String,        // 系统订单号
    pub client_order_id: String, // 客户端自定义订单ID
}


// WS ORDER_TRADE_UPDATE 事件执行类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutionType {
    #[serde(rename = "NEW")]
    New,         // 新订单
    #[serde(rename = "TRADE")]
    Trade,       // 成交
    #[serde(rename = "CANCELED")]
    Canceled,    // 撤销
    #[serde(rename = "CALCULATED")]
    Calculated,  // 订单 ADL 或爆仓
    #[serde(rename = "EXPIRED")]
    Expired,     // 过期
    #[serde(rename = "AMENDMENT")]
    Amendment,   // 修改
    #[serde(untagged)]
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
}


// 币安订单状态 (与 WS ORDER_TRADE_UPDATE X 字段对齐)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CcxtOrderStatus {
    #[serde(rename = "NEW")]
    New,
    #[serde(rename = "PARTIALLY_FILLED")]
    PartiallyFilled,
    #[serde(rename = "FILLED")]
    Filled,
    #[serde(rename = "CANCELED")]
    Canceled,
    #[serde(rename = "EXPIRED")]
    Expired,
    #[serde(rename = "EXPIRED_IN_MATCH")]
    ExpiredInMatch,
    #[serde(untagged)]
    Unknown(String),
}

impl CcxtOrderStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "NEW" => Self::New,
            "PARTIALLY_FILLED" => Self::PartiallyFilled,
            "FILLED" => Self::Filled,
            "CANCELED" | "CANCELLED" => Self::Canceled,
            "EXPIRED" => Self::Expired,
            "EXPIRED_IN_MATCH" => Self::ExpiredInMatch,
            other => Self::Unknown(other.to_string()),
        }
    }
}

impl From<CcxtOrderStatus> for OrderStatus {
    fn from(s: CcxtOrderStatus) -> Self {
        match s {
            CcxtOrderStatus::New => OrderStatus::Open,
            CcxtOrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
            CcxtOrderStatus::Filled => OrderStatus::Filled,
            CcxtOrderStatus::Canceled => OrderStatus::Canceled,
            CcxtOrderStatus::Expired => OrderStatus::Canceled,
            CcxtOrderStatus::ExpiredInMatch => OrderStatus::Canceled,
            CcxtOrderStatus::Unknown(_) => {
                unreachable!("validate_order_fields ensures status is known before CcxtOrderStatus is created")
            }
        }
    }
}


// WS ORDER_TRADE_UPDATE 订单结构体
// 完整映射币安官方文档中 o 对象的 37 个字段
// 字段类型与币安原生返回保持一致: 字符串字段为 String，整数字段为 i64，布尔字段为 bool
// WS 反序列化由 OrderTradeUpdateData 负责，再通过 to_ccxt_order() 转换
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CcxtOrder {
    // --- 订单标识 ---
    pub order_id: i64,                       // 订单ID
    pub client_order_id: String,             // 客户端自定义订单ID

    // --- 订单基本信息 ---
    pub symbol: String,                      // 交易对
    pub side: Side,                         // 买卖方向 (BUY/SELL)
    pub order_type: OrderType,               // 订单类型
    pub position_side: PositionSide,         // 持仓方向 (LONG/SHORT)
    pub original_order_type: String,         // 原始订单类型
    pub status: CcxtOrderStatus,             // 订单当前状态
    pub execution_type: ExecutionType,       // 本次事件执行类型

    // --- 价格与数量 (币安返回字符串，保持原样) ---
    pub orig_qty: String,                    // 原始数量
    pub original_price: String,              // 原始价格
    pub avg_fill_price: String,              // 平均成交价
    pub filled_qty: String,                  // 累计已成交量
    pub last_fill_qty: String,               // 末次成交量
    pub last_fill_price: String,             // 末次成交价
    pub stop_price: Option<String>,          // 条件订单触发价格

    // --- 手续费与盈亏 (币安返回字符串，保持原样) ---
    pub commission: String,                  // 手续费数量
    pub commission_asset: String,            // 手续费资产类型
    pub realized_pnl: String,                // 该交易实现盈亏

    // --- 订单属性 ---
    pub reduce_only: bool,                   // 是否仅减仓 (exchange-native 字段, 业务层不使用)
    pub is_maker: bool,                      // 是否为挂单成交
    pub close_position: Option<bool>,        // 是否为触发平仓单 (仅在条件订单情况下推送)
    pub time_in_force: String,               // 有效方式 (GTC/IOC/FOK/GTX)
    pub working_type: String,                // 触发价类型

    // --- 名义价值 (币安返回字符串，保持原样) ---
    pub bids_notional: Option<String>,       // 买单净值
    pub ask_notional: Option<String>,        // 卖单净值

    // --- 追踪止损 (仅 TRAILING_STOP_MARKET 返回) ---
    pub activation_price: Option<String>,    // 追踪止损激活价格
    pub callback_rate: Option<String>,      // 追踪止损回调比例

    // --- 价格保护与模式 ---
    pub price_protection: bool,              // 是否开启条件单触发保护
    pub stp_mode: Option<String>,            // 自成交防止模式
    pub price_match_mode: Option<String>,    // 价格匹配模式
    pub gtd_auto_cancel_time: Option<i64>,   // TIF为GTD的订单自动取消时间
    pub expiry_reason: Option<String>,       // 过期原因 (0-9)

    // --- 忽略字段 (官方标注忽略，保留以完整映射) ---
    pub si: i64,                             // 忽略
    pub ss: i64,                             // 忽略

    // --- 时间与成交ID ---
    pub trade_time: i64,                     // 成交时间(ms)
    pub trade_id: i64,                       // 成交ID
}


/// 订单字段校验错误。由 `validate_order_fields` 返回，调用方据此记录日志并跳过该订单。
#[derive(Debug)]
pub enum OrderFieldError {
    /// side (S) 非 BUY/SELL
    InvalidSide(String),
    /// position_side (ps) 非 LONG/SHORT，或为 None（OneWay 模式）
    InvalidPositionSide(Option<String>),
    /// status (X) 不在已知枚举范围内
    UnknownStatus(String),
}

impl std::fmt::Display for OrderFieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSide(v) => write!(f, "side 非法 (非 BUY/SELL): {}", v),
            Self::InvalidPositionSide(Some(v)) => {
                write!(f, "position_side 非法 (非 LONG/SHORT): {}", v)
            }
            Self::InvalidPositionSide(None) => {
                write!(f, "position_side 为 None (OneWay 模式?)")
            }
            Self::UnknownStatus(v) => write!(f, "status 未知: {}", v),
        }
    }
}

impl std::error::Error for OrderFieldError {}


/// 校验订单必需字段的合法性。WS 事件入口和 DB 读取路径共用此函数。
///
/// 校验范围（影响业务逻辑的必需字段）：
/// - side: 必需 BUY/SELL — 决定开平仓方向
/// - position_side: Hedge 模式下必需 LONG/SHORT — 决定仓位定位
/// - status: 必需已知状态 — 决定成交处理和终态清理
///
/// 不校验的字段（纯信息，透传入库不影响逻辑）：
/// - order_type, execution_type: 未知类型保留原始字符串入库
///
/// # 参数
/// - `side`: 原始 side 字符串 (S 字段)
/// - `position_side`: 原始 position_side 字符串 (ps 字段)，DB 路径传 `Some(&str)`，WS 路径可能为 None
/// - `status`: 原始 status 字符串 (X 字段)
///
/// # 返回
/// - `Ok(())`: 字段合法，可以继续转换
/// - `Err(OrderFieldError)`: 字段非法，调用方应记录日志并跳过该订单
pub fn validate_order_fields(
    side: &str,
    position_side: Option<&str>,
    status: &str,
) -> Result<(), OrderFieldError> {
    validate_side(side)?;
    validate_position_side(position_side)?;
    validate_status(status)?;
    Ok(())
}

/// 校验 side (S) 字段：必需 BUY/SELL。
pub fn validate_side(side: &str) -> Result<(), OrderFieldError> {
    match side {
        "BUY" | "SELL" => Ok(()),
        other => Err(OrderFieldError::InvalidSide(other.to_string())),
    }
}

/// 校验 position_side (ps) 字段：Hedge 模式下必需 LONG/SHORT。
/// 可独立调用（如 DB 聚合仓位恢复时只需校验 position_side）。
pub fn validate_position_side(position_side: Option<&str>) -> Result<(), OrderFieldError> {
    match position_side {
        Some("LONG") | Some("SHORT") => Ok(()),
        Some(other) => Err(OrderFieldError::InvalidPositionSide(Some(other.to_string()))),
        None => Err(OrderFieldError::InvalidPositionSide(None)),
    }
}

/// 校验 status (X) 字段：必需已知状态。
pub fn validate_status(status: &str) -> Result<(), OrderFieldError> {
    match status {
        "NEW" | "PARTIALLY_FILLED" | "FILLED" | "CANCELED" | "CANCELLED" | "EXPIRED"
        | "EXPIRED_IN_MATCH" => Ok(()),
        other => Err(OrderFieldError::UnknownStatus(other.to_string())),
    }
}
