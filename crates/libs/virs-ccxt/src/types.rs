use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use virs_error::ExchangeError;


pub use virs_types::enums::{MarketType, OrderStatus, OrderType, PositionMode, PositionSide, Side};
pub use virs_types::market::{
    Balance, ExchangePosition, FundingHistoryEntry, FundingRate, Kline, OrderBook, Ticker,
};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketInfo {
    pub id: String,
    pub symbol: String,
    pub base: String,
    pub quote: String,
    pub active: bool,
    pub market_type: MarketType,
    pub min_amount: Option<f64>,
    pub max_amount: Option<f64>,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub min_cost: Option<f64>,
    pub price_precision: Option<u32>,
    pub amount_precision: Option<u32>,
    pub info: serde_json::Value,
}


#[derive(Debug, Clone)]
pub struct PlaceOrderParams {
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub amount: f64,
    pub price: Option<f64>,
    pub market_type: MarketType,
    pub client_order_id: Option<String>,
    pub stop_price: Option<f64>,
    pub time_in_force: Option<TimeInForce>,
    pub reduce_only: Option<bool>,
    pub leverage: Option<u32>,
    pub margin_mode: Option<MarginMode>,
    pub position_side: Option<PositionSide>,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum MarginMode {
    Cross,
    Isolated,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFee {
    pub cost: f64,
    pub currency: String,
    pub rate: Option<f64>,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeInForce {
    Gtc,
    Ioc,
    Fok,
    Poc,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub side: PositionSide,
    pub size: f64,
    pub entry_price: f64,
    pub leverage: u32,
    pub unrealized_pnl: f64,
    pub margin_mode: MarginMode,
    pub liquidation_price: Option<f64>,
    pub info: serde_json::Value,
}


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


// WS ORDER_TRADE_UPDATE 订单结构体
// 完整映射币安官方文档中 o 对象的 37 个字段
// 字段类型与币安原生返回保持一致: 字符串字段为 String，整数字段为 i64，布尔字段为 bool
// WS 反序列化由 OrderTradeUpdateData 负责，再通过 to_ccxt_order() 转换
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub reduce_only: bool,                   // 是否仅减仓
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
            _ => Self::New,
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
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcxtTicker {
    pub symbol: String,
    pub exchange: String,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub last: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub volume: Option<f64>,
    pub quote_volume: Option<f64>,
    pub open: Option<f64>,
    pub close: Option<f64>,
    pub price_change: Option<f64>,
    pub price_change_pct: Option<f64>,
    pub timestamp: Option<DateTime<Utc>>,
    pub info: serde_json::Value,
}

impl TryFrom<CcxtTicker> for Ticker {
    type Error = ExchangeError;

    fn try_from(t: CcxtTicker) -> Result<Self, Self::Error> {
        let symbol = t.symbol.clone();


        let bid = t.bid;
        let ask = t.ask;


        let last = t.last.ok_or_else(|| {
            tracing::error!(symbol = %symbol, "Ticker last missing");
            ExchangeError::no_data(format!("Ticker last missing for {}", symbol))
        })?;
        let high_24h = t.high.ok_or_else(|| {
            tracing::warn!(symbol = %symbol, "Ticker high_24h missing");
            ExchangeError::no_data(format!("Ticker high_24h missing for {}", symbol))
        })?;
        let low_24h = t.low.ok_or_else(|| {
            tracing::warn!(symbol = %symbol, "Ticker low_24h missing");
            ExchangeError::no_data(format!("Ticker low_24h missing for {}", symbol))
        })?;
        let volume_24h = t.volume.ok_or_else(|| {
            tracing::warn!(symbol = %symbol, "Ticker volume_24h missing");
            ExchangeError::no_data(format!("Ticker volume_24h missing for {}", symbol))
        })?;
        let price_change_24h = t.price_change.ok_or_else(|| {
            tracing::warn!(symbol = %symbol, "Ticker price_change_24h missing");
            ExchangeError::no_data(format!("Ticker price_change_24h missing for {}", symbol))
        })?;
        let price_change_pct_24h = t.price_change_pct.ok_or_else(|| {
            tracing::warn!(symbol = %symbol, "Ticker price_change_pct_24h missing");
            ExchangeError::no_data(format!("Ticker price_change_pct_24h missing for {}", symbol))
        })?;

        Ok(Ticker {
            symbol: t.symbol,
            exchange: t.exchange,
            bid,
            ask,
            last,
            high_24h,
            low_24h,
            volume_24h,
            price_change_24h,
            price_change_pct_24h,
            timestamp: t.timestamp.unwrap_or_else(chrono::Utc::now),
        })
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcxtKline {
    pub timestamp: i64,


    pub close_time: Option<i64>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub quote_volume: Option<f64>,
    pub trades: Option<i64>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcxtOrderBook {
    pub symbol: String,
    pub bids: Vec<(f64, f64)>,
    pub asks: Vec<(f64, f64)>,
    pub timestamp: Option<DateTime<Utc>>,
    pub nonce: Option<u64>,
}

impl From<CcxtOrderBook> for OrderBook {
    fn from(ob: CcxtOrderBook) -> Self {
        OrderBook {
            symbol: ob.symbol,
            bids: ob.bids,
            asks: ob.asks,
            timestamp: ob.timestamp.unwrap_or_else(chrono::Utc::now),
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcxtFundingRate {
    pub symbol: String,
    pub rate: f64,
    pub next_funding_time: Option<DateTime<Utc>>,
    pub info: serde_json::Value,
}

impl From<CcxtFundingRate> for FundingRate {
    fn from(fr: CcxtFundingRate) -> Self {
        FundingRate {
            symbol: fr.symbol,
            rate: fr.rate,
            next_funding_time: fr.next_funding_time,
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcxtFundingHistoryEntry {
    pub funding_time: DateTime<Utc>,
    pub rate: f64,
}

impl From<CcxtFundingHistoryEntry> for FundingHistoryEntry {
    fn from(e: CcxtFundingHistoryEntry) -> Self {
        FundingHistoryEntry {
            funding_time: e.funding_time,
            rate: e.rate,
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRestrictions {
    pub ip_restrict: bool,
    pub ip_whitelist: Vec<String>,
    pub ip_not_restricted: bool,
    pub create_sub_account: bool,
    pub read_info: bool,
    pub enable_withdrawals: bool,
    pub enable_internal_transfer: bool,
    pub enable_futures: bool,
    pub enable_vanilla_options: bool,
    pub enable_portfolio_margin_trading: bool,
    pub enable_fix_api_trade: bool,
    pub enable_fix_api_read: bool,
    pub info: serde_json::Value,
}
