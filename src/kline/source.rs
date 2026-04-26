use async_trait::async_trait;
use crate::kline::{KlineSource, types::Candle};
use crate::ccxt::{self, MarketType as CcxtMarketType};
use crate::models::MarketType;

pub struct CcxtKlineSource {
    proxy_url: Option<String>,
}

impl CcxtKlineSource {
    pub fn new(proxy_url: Option<String>) -> Self {
        Self { proxy_url }
    }
}

#[async_trait]
impl KlineSource for CcxtKlineSource {
    async fn fetch_klines(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: &str,
        limit: u32,
        since: Option<i64>,
        market_type: Option<MarketType>,
    ) -> anyhow::Result<Vec<Candle>> {
        let ccxt_market_type = match market_type.unwrap_or(MarketType::Spot) {
            MarketType::Spot => CcxtMarketType::Spot,
            MarketType::Perpetual => CcxtMarketType::Perpetual,
        };
        let ccxt_ex = ccxt::create_exchange(
            exchange,
            "",
            "",
            None,
            self.proxy_url.as_deref(),
            &ccxt_market_type,
        )
        .map_err(|e| anyhow::anyhow!("Failed to create exchange '{}': {}", exchange, e))?;

        let klines = ccxt_ex.fetch_ohlcv(symbol, timeframe, limit, since)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch klines: {}", e))?;

        Ok(klines.into_iter().map(|k| Candle {
            open_time: k.timestamp,
            close_time: k.timestamp + match timeframe {
                "1m" => 60_000,
                "5m" => 300_000,
                "15m" => 900_000,
                "1h" => 3_600_000,
                "4h" => 14_400_000,
                "1d" => 86_400_000,
                _ => 60_000,
            } - 1,
            open: k.open,
            high: k.high,
            low: k.low,
            close: k.close,
            volume: k.volume,
            quote_volume: k.quote_volume.unwrap_or(0.0),
            trades: k.trades.unwrap_or(0),
            closed: true,
        }).collect())
    }
}
