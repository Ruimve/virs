use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::exchange::MarginMode;
use crate::position::PositionSide;
use super::enums::{KlineEventType, Timeframe};


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ticker {
    pub symbol: String,
    pub exchange: String,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub last: f64,
    pub high_24h: f64,
    pub low_24h: f64,
    pub volume_24h: f64,
    pub price_change_24h: f64,
    pub price_change_pct_24h: f64,
    pub timestamp: DateTime<Utc>,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Kline {
    pub open_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub close_time: i64,
    pub quote_volume: f64,
    pub trades: i64,
    pub symbol: String,
    pub exchange: String,
    pub interval: String,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: Vec<(f64, f64)>,
    pub asks: Vec<(f64, f64)>,
    pub timestamp: DateTime<Utc>,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub free: f64,
    pub used: f64,
    pub total: f64,
}

impl Balance {

    pub fn compute_total(&self) -> f64 {
        self.free + self.used
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangePosition {
    pub symbol: String,
    pub side: PositionSide,
    pub quantity: f64,
    pub entry_price: f64,
    pub margin_mode: MarginMode,
    pub info: serde_json::Value,
}


/// WS K 线数据（OHLCV + 成交量）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candle {
    pub open_time: i64,
    pub close_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub quote_volume: f64,
    pub trades: i64,
    pub closed: bool,
}


/// K 线行情事件（WS 推送 + 回填）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineEvent {
    pub exchange: String,
    pub symbol: String,
    pub timeframe: Timeframe,
    pub candle: Candle,
    pub event_type: KlineEventType,
}


/// 市场指标快照。作为 `indicators_json` 的 JSON 传输格式。
///
/// 注意：本 struct 仅用于过渡期的 JSON 序列化/反序列化。
/// 构造逻辑（`from_indicator_set`）留在 virs-strategy 作为自由函数，
/// 因为其依赖 `IndicatorSet` / `IndicatorSpec` 等策略内部类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketIndicators {
    pub current_price: f64,
    pub rsi: f64,
    pub atr: f64,
    pub atr_pct: f64,
    pub bb_width: f64,
    pub bb_upper: f64,
    pub bb_middle: f64,
    pub bb_lower: f64,
    pub ema12: f64,
    pub ema20: f64,
    pub ema26: f64,
    pub ema50: f64,
    pub macd: f64,
    pub macd_signal: f64,
    pub macd_histogram: f64,
    pub adx: f64,
    pub change_1h: f64,
    pub h1_atr_sma20: f64,
    pub h1_candle_body: f64,
    pub h1_bars_outside_band: i32,
    pub h1_bandwidth_5bars_ago: f64,
    pub h1_high_20: f64,
    pub h1_low_20: f64,
    pub nearest_round_up: f64,
    pub nearest_round_down: f64,
    pub h1_volume: f64,
    pub h1_volume_sma20: f64,
    pub h1_ema_cross_bars_ago: i32,
    pub h1_ema_gap_pct: f64,
    pub h1_ema_gap_trend: String,
    pub h1_high_50: f64,
    pub h1_low_50: f64,

    pub m15_current_price: f64,
    pub m15_rsi: f64,
    pub m15_macd: f64,
    pub m15_macd_signal: f64,
    pub m15_macd_histogram: f64,
    pub m15_bb_width_pct: f64,
    pub m15_atr: f64,
    pub m15_atr_sma20: f64,
    pub m15_adx: f64,
    pub m15_bars_outside_band: i32,
    pub m15_ema20: f64,
    pub m15_ema50: f64,
    pub m15_volume: f64,
    pub m15_volume_sma20: f64,
    pub m15_ema_cross_bars_ago: i32,
    pub m15_high_50: f64,
    pub m15_low_50: f64,

    pub h4_ema20: f64,
    pub h4_ema50: f64,
    pub h4_adx: f64,
    pub h4_bb_width_pct: f64,
    pub h4_rsi: f64,
    pub h4_macd: f64,
    pub h4_macd_signal: f64,
    pub h4_macd_histogram: f64,

    pub funding_rate: f64,
    pub funding_next_time: String,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FundingRate {
    pub symbol: String,
    pub rate: f64,
    pub next_funding_time: Option<DateTime<Utc>>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRestrictions {
    pub ip_restrict: Option<bool>,
    pub ip_whitelist: Vec<String>,
    pub ip_not_restricted: Option<bool>,
    pub create_sub_account: Option<bool>,
    pub read_info: Option<bool>,
    pub enable_withdrawals: Option<bool>,
    pub enable_internal_transfer: Option<bool>,
    pub enable_futures: Option<bool>,
    pub enable_vanilla_options: Option<bool>,
    pub enable_portfolio_margin_trading: Option<bool>,
    pub enable_fix_api_trade: Option<bool>,
    pub enable_fix_api_read: Option<bool>,
    pub info: serde_json::Value,
}
