use virs_error::VirsResult;

pub(crate) use virs_type::{KlineWsClient, WsEvent};

use virs_type::{Candle, Timeframe};

pub(crate) use virs_type::{
    OrderBookWsClient, WsOrderBookEvent,
};

pub(crate) use virs_type::MarketType;

#[derive(Debug, Clone)]
pub struct KlineEngineConfig {
    pub reconnect_delay_secs: u64,
    pub max_reconnect_delay_secs: u64,
    pub ws_ping_interval_secs: u64,
    pub ws_max_lifetime_secs: u64,
    pub backfill_on_start: bool,
    pub event_channel_capacity: usize,
    pub proxy_url: Option<String>,
}

impl Default for KlineEngineConfig {
    fn default() -> Self {
        Self {
            reconnect_delay_secs: 1,
            max_reconnect_delay_secs: 60,
            ws_ping_interval_secs: 30,
            /* WS连接最大生命周期23小时：定期重建连接，避免交易所长时间连接导致的潜在问题 */
            ws_max_lifetime_secs: 23 * 3600,
            backfill_on_start: true,
            event_channel_capacity: 512,
            proxy_url: None,
        }
    }
}

/* K线数据源trait抽象：解耦K线获取逻辑，支持交易所REST API等不同实现 */
#[async_trait::async_trait]
pub trait KlineSource: Send + Sync {
    async fn fetch_klines(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: &str,
        limit: u32,
        since: Option<i64>,
        market_type: Option<MarketType>,
    ) -> VirsResult<Vec<Candle>>;
}

#[async_trait::async_trait]
pub(crate) trait KlinePersistence: Send + Sync {
    async fn save_candles(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: &str,
        candles: &[Candle],
    ) -> VirsResult<()>;
}

/* 生成订阅唯一键：exchange小写 + symbol大写，确保大小写不敏感匹配 */
pub fn subscription_key(exchange: &str, symbol: &str) -> String {
    format!("{}:{}", exchange.to_lowercase(), symbol.to_uppercase())
}

/* 将任意时间戳对齐到指定周期的开盘时间（向下取整） */
pub fn align_open_time(open_time: i64, timeframe: Timeframe) -> i64 {
    (open_time / timeframe.ms()) * timeframe.ms()
}

#[derive(Debug, Clone)]
pub struct OrderBookEngineConfig {
    pub event_channel_capacity: usize,
}

impl Default for OrderBookEngineConfig {
    fn default() -> Self {
        Self {
            event_channel_capacity: 512,
        }
    }
}
