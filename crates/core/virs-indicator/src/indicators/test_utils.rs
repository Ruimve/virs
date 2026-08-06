//! 测试公共辅助：构造 K 线数据。

use virs_type::Kline;

/// 构造一条 K 线。
pub fn kline(open: f64, high: f64, low: f64, close: f64, volume: f64) -> Kline {
    Kline {
        open_time: 0,
        open,
        high,
        low,
        close,
        volume,
        close_time: 0,
        quote_volume: volume * close,
        trades: 0,
        symbol: "TEST/USDT".to_string(),
        exchange: "test".to_string(),
        interval: "1h".to_string(),
    }
}

/// 构造 N 条上涨趋势 K 线（close 逐步递增）。
pub fn uptrend_klines(n: usize, start: f64, step: f64) -> Vec<Kline> {
    (0..n)
        .map(|i| {
            let close = start + step * i as f64;
            kline(close - step, close + step * 0.5, close - step * 0.5, close, 1000.0 + i as f64 * 10.0)
        })
        .collect()
}

/// 构造 N 条下跌趋势 K 线。
pub fn downtrend_klines(n: usize, start: f64, step: f64) -> Vec<Kline> {
    (0..n)
        .map(|i| {
            let close = start - step * i as f64;
            kline(close + step, close + step * 0.5, close - step * 0.5, close, 1000.0 + i as f64 * 10.0)
        })
        .collect()
}

/// 构造 N 条震荡 K 线（close 在 center 上下波动）。
pub fn sideways_klines(n: usize, center: f64, amplitude: f64) -> Vec<Kline> {
    (0..n)
        .map(|i| {
            let offset = if i % 2 == 0 { amplitude } else { -amplitude };
            let close = center + offset;
            kline(center, close + amplitude * 0.5, close - amplitude * 0.5, close, 1000.0)
        })
        .collect()
}
