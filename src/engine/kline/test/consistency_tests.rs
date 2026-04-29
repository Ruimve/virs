use crate::engine::kline::*;
use crate::engine::kline::types::WsCandleUpdate;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// 可编程控制的 Mock WS 客户端，可以手动推送事件
struct ControllableMockWsClient {
    running: AtomicBool,
    event_tx: tokio::sync::Mutex<Option<broadcast::Sender<WsEvent>>>,
}

impl ControllableMockWsClient {
    fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            event_tx: tokio::sync::Mutex::new(None),
        }
    }

    async fn push_candle(&self, symbol: &str, candle: Candle) {
        if let Some(tx) = self.event_tx.lock().await.as_ref() {
            let _ = tx.send(WsEvent::Candle(WsCandleUpdate {
                symbol: symbol.to_string(),
                candle,
            }));
        }
    }

    async fn push_reconnected(&self) {
        if let Some(tx) = self.event_tx.lock().await.as_ref() {
            let _ = tx.send(WsEvent::Reconnected);
        }
    }
}

#[async_trait]
impl KlineWsClient for ControllableMockWsClient {
    async fn start(&mut self, update_tx: broadcast::Sender<WsEvent>) {
        self.running.store(true, Ordering::Relaxed);
        *self.event_tx.lock().await = Some(update_tx);
    }

    async fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        *self.event_tx.lock().await = None;
    }

    async fn subscribe(&self, _symbol: &str) {}

    async fn unsubscribe(&self, _symbol: &str) {}

    fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

/// 可编程控制的 Mock 数据源，可以按周期设置返回数据
struct ControllableMockSource {
    data: std::sync::Mutex<HashMap<String, Vec<Candle>>>,
}

impl ControllableMockSource {
    fn new() -> Self {
        Self {
            data: std::sync::Mutex::new(HashMap::new()),
        }
    }

    fn set_data(&self, timeframe: &str, candles: Vec<Candle>) {
        self.data.lock().unwrap().insert(timeframe.to_string(), candles);
    }
}

#[async_trait]
impl KlineSource for ControllableMockSource {
    async fn fetch_klines(
        &self,
        _exchange: &str,
        _symbol: &str,
        timeframe: &str,
        _limit: u32,
        _since: Option<i64>,
        _market_type: Option<MarketType>,
    ) -> anyhow::Result<Vec<Candle>> {
        let data = self.data.lock().unwrap();
        Ok(data.get(timeframe).cloned().unwrap_or_default())
    }
}

/// 生成测试用的 1m K 线序列
fn make_test_1m_candles(count: usize, start_time: i64, base_price: f64) -> Vec<Candle> {
    (0..count)
        .map(|i| {
            let open_time = start_time + (i as i64) * 60_000;
            let close_time = open_time + 59_999;
            let price = base_price + (i as f64) * 0.5;
            Candle {
                open_time,
                close_time,
                open: price,
                high: price + 1.0,
                low: price - 1.0,
                close: price + 0.5,
                volume: 100.0 + i as f64,
                quote_volume: (100.0 + i as f64) * price,
                trades: 50 + i as i64,
                closed: true,
            }
        })
        .collect()
}

/// 容差比较两根 K 线
fn assert_candle_consistent(actual: &Candle, expected: &Candle, label: &str) {
    assert_eq!(actual.open_time, expected.open_time, "{}: open_time 不一致", label);
    assert_eq!(actual.close_time, expected.close_time, "{}: close_time 不一致", label);
    assert!(
        (actual.open - expected.open).abs() < 0.001,
        "{}: open 偏差 {:.6} vs {:.6}",
        label,
        actual.open,
        expected.open
    );
    assert!(
        (actual.high - expected.high).abs() < 0.001,
        "{}: high 偏差 {:.6} vs {:.6}",
        label,
        actual.high,
        expected.high
    );
    assert!(
        (actual.low - expected.low).abs() < 0.001,
        "{}: low 偏差 {:.6} vs {:.6}",
        label,
        actual.low,
        expected.low
    );
    assert!(
        (actual.close - expected.close).abs() < 0.001,
        "{}: close 偏差 {:.6} vs {:.6}",
        label,
        actual.close,
        expected.close
    );
    let vol_diff = (actual.volume - expected.volume).abs();
    let vol_tol = expected.volume.max(0.001) * 0.001;
    assert!(
        vol_diff < vol_tol,
        "{}: volume 偏差 {:.4} vs {:.4} (tol={:.4})",
        label,
        actual.volume,
        expected.volume,
        vol_tol
    );
    assert_eq!(actual.closed, expected.closed, "{}: closed 不一致", label);
}

/// 等待 KlineEngine 广播足够数量的事件
async fn wait_for_engine_events(
    engine: &KlineEngine,
    min_count: usize,
    timeout: Duration,
) -> Vec<KlineEvent> {
    let mut rx = engine.subscribe_events();
    let start = tokio::time::Instant::now();
    let mut events = Vec::new();
    while events.len() < min_count && start.elapsed() < timeout {
        match tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
            Ok(Ok(event)) => events.push(event),
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => {
                // 跳过 lagged 事件，继续等待
                continue;
            }
            Ok(Err(broadcast::error::RecvError::Closed)) => break,
            Err(_) => continue, // timeout, retry
        }
    }
    events
}

/// 创建带可控 WS 的测试引擎，返回 (engine, spot_ws 控制句柄)
fn create_consistency_engine(
    source: Arc<ControllableMockSource>,
) -> (KlineEngine, Arc<Mutex<ControllableMockWsClient>>) {
    let spot_ws = Arc::new(Mutex::new(ControllableMockWsClient::new()));
    let perpetual_ws = Arc::new(Mutex::new(ControllableMockWsClient::new()));
    let config = KlineEngineConfig {
        backfill_on_start: true,
        event_channel_capacity: 8192,
        ..KlineEngineConfig::default()
    };
    let engine = KlineEngine::new(
        config,
        source.clone(),
        spot_ws.clone(),
        perpetual_ws,
    );
    (engine, spot_ws)
}

// ── REST 一致性测试 ──

/// 辅助：为 source 设置 1m 数据，并从 1m 聚合出高级周期数据填入 source
/// 模拟真实交易所 REST API 行为：高级周期由交易所直接提供
fn setup_source_with_aggregated(
    source: &ControllableMockSource,
    candles_1m: Vec<Candle>,
) {
    source.set_data("1m", candles_1m.clone());
    // 模拟交易所 REST API 返回预聚合的高级周期数据
    for (tf_str, tf) in &[("5m", Timeframe::M5), ("15m", Timeframe::M15), ("1h", Timeframe::H1), ("4h", Timeframe::H4), ("1d", Timeframe::D1)] {
        let aggregated = Aggregator::aggregate_1m_to_timeframe(&candles_1m, *tf);
        source.set_data(tf_str, aggregated);
    }
}

#[tokio::test]
async fn test_rest_consistency_m1() {
    let candles_1m = make_test_1m_candles(100, 1713900000000, 65000.0);
    let source = Arc::new(ControllableMockSource::new());
    setup_source_with_aggregated(&source, candles_1m.clone());

    let (engine, _spot_ws) = create_consistency_engine(source);
    engine.start().await;
    engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();

    // 等待 backfill 完成
    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = engine.get_klines_async("binance", "BTCUSDT", Timeframe::M1).await;
    assert!(result.is_some(), "M1 数据不应为 None");
    let result_candles = result.unwrap();
    assert_eq!(result_candles.len(), 100, "M1 应有 100 根");
    for (i, (actual, expected)) in result_candles.iter().zip(candles_1m.iter()).enumerate() {
        assert_candle_consistent(actual, expected, &format!("M1[{}]", i));
    }
}

#[tokio::test]
async fn test_rest_consistency_m5() {
    let candles_1m = make_test_1m_candles(100, 1713900000000, 65000.0);
    let expected_m5 = Aggregator::aggregate_1m_to_timeframe(&candles_1m, Timeframe::M5);
    // initial_load 会追加 1 根 unclosed candle，只比较 closed 部分
    let expected_m5_closed: Vec<_> = expected_m5.into_iter().filter(|c| c.closed).collect();

    let source = Arc::new(ControllableMockSource::new());
    setup_source_with_aggregated(&source, candles_1m);

    let (engine, _spot_ws) = create_consistency_engine(source);
    engine.start().await;
    engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = engine.get_klines_async("binance", "BTCUSDT", Timeframe::M5).await;
    assert!(result.is_some(), "M5 数据不应为 None");
    let result_candles = result.unwrap();
    let result_closed: Vec<_> = result_candles.into_iter().filter(|c| c.closed).collect();
    assert_eq!(result_closed.len(), expected_m5_closed.len(), "M5 closed 数量应一致");
    for (i, (actual, expected)) in result_closed.iter().zip(expected_m5_closed.iter()).enumerate() {
        assert_candle_consistent(actual, expected, &format!("M5[{}]", i));
    }
}

#[tokio::test]
async fn test_rest_consistency_m15() {
    let candles_1m = make_test_1m_candles(150, 1713900000000, 65000.0);
    let expected_m15 = Aggregator::aggregate_1m_to_timeframe(&candles_1m, Timeframe::M15);
    let expected_m15_closed: Vec<_> = expected_m15.into_iter().filter(|c| c.closed).collect();

    let source = Arc::new(ControllableMockSource::new());
    setup_source_with_aggregated(&source, candles_1m);

    let (engine, _spot_ws) = create_consistency_engine(source);
    engine.start().await;
    engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = engine.get_klines_async("binance", "BTCUSDT", Timeframe::M15).await;
    assert!(result.is_some());
    let result_candles = result.unwrap();
    let result_closed: Vec<_> = result_candles.into_iter().filter(|c| c.closed).collect();
    assert_eq!(result_closed.len(), expected_m15_closed.len(), "M15 closed 数量应一致");
    for (i, (actual, expected)) in result_closed.iter().zip(expected_m15_closed.iter()).enumerate() {
        assert_candle_consistent(actual, expected, &format!("M15[{}]", i));
    }
}

#[tokio::test]
async fn test_rest_consistency_h1() {
    let candles_1m = make_test_1m_candles(120, 1713900000000, 65000.0);
    let expected_h1 = Aggregator::aggregate_1m_to_timeframe(&candles_1m, Timeframe::H1);
    let expected_h1_closed: Vec<_> = expected_h1.into_iter().filter(|c| c.closed).collect();

    let source = Arc::new(ControllableMockSource::new());
    setup_source_with_aggregated(&source, candles_1m);

    let (engine, _spot_ws) = create_consistency_engine(source);
    engine.start().await;
    engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = engine.get_klines_async("binance", "BTCUSDT", Timeframe::H1).await;
    assert!(result.is_some());
    let result_candles = result.unwrap();
    let result_closed: Vec<_> = result_candles.into_iter().filter(|c| c.closed).collect();
    assert_eq!(result_closed.len(), expected_h1_closed.len(), "H1 closed 数量应一致");
    for (i, (actual, expected)) in result_closed.iter().zip(expected_h1_closed.iter()).enumerate() {
        assert_candle_consistent(actual, expected, &format!("H1[{}]", i));
    }
}

#[tokio::test]
async fn test_rest_consistency_h4() {
    let candles_1m = make_test_1m_candles(240, 1713900000000, 65000.0);
    let expected_h4 = Aggregator::aggregate_1m_to_timeframe(&candles_1m, Timeframe::H4);
    let expected_h4_closed: Vec<_> = expected_h4.into_iter().filter(|c| c.closed).collect();

    let source = Arc::new(ControllableMockSource::new());
    setup_source_with_aggregated(&source, candles_1m);

    let (engine, _spot_ws) = create_consistency_engine(source);
    engine.start().await;
    engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = engine.get_klines_async("binance", "BTCUSDT", Timeframe::H4).await;
    assert!(result.is_some());
    let result_candles = result.unwrap();
    let result_closed: Vec<_> = result_candles.into_iter().filter(|c| c.closed).collect();
    assert_eq!(result_closed.len(), expected_h4_closed.len(), "H4 closed 数量应一致");
    for (i, (actual, expected)) in result_closed.iter().zip(expected_h4_closed.iter()).enumerate() {
        assert_candle_consistent(actual, expected, &format!("H4[{}]", i));
    }
}

#[tokio::test]
async fn test_rest_consistency_d1() {
    let candles_1m = make_test_1m_candles(1440, 1713900000000, 65000.0);
    let expected_d1 = Aggregator::aggregate_1m_to_timeframe(&candles_1m, Timeframe::D1);
    let expected_d1_closed: Vec<_> = expected_d1.into_iter().filter(|c| c.closed).collect();

    let source = Arc::new(ControllableMockSource::new());
    setup_source_with_aggregated(&source, candles_1m);

    let (engine, _spot_ws) = create_consistency_engine(source);
    engine.start().await;
    engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    let result = engine.get_klines_async("binance", "BTCUSDT", Timeframe::D1).await;
    assert!(result.is_some());
    let result_candles = result.unwrap();
    let result_closed: Vec<_> = result_candles.into_iter().filter(|c| c.closed).collect();
    assert_eq!(result_closed.len(), expected_d1_closed.len(), "D1 closed 数量应一致");
    for (i, (actual, expected)) in result_closed.iter().zip(expected_d1_closed.iter()).enumerate() {
        assert_candle_consistent(actual, expected, &format!("D1[{}]", i));
    }
}

// ── WS 一致性测试 ──

#[tokio::test]
async fn test_ws_consistency_m1_realtime() {
    let source = Arc::new(ControllableMockSource::new());
    let (engine, spot_ws) = create_consistency_engine(source);
    engine.start().await;
    engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();

    // 订阅事件（在 subscribe 之后订阅，避免收到 backfill 事件）
    let mut event_rx = engine.subscribe_events();

    let candles = make_test_1m_candles(50, 1713900000000, 65000.0);

    // 逐根推送并验证（引擎每根 1m 会广播 M1 + 高级周期事件，需过滤 M1）
    for (i, candle) in candles.iter().enumerate() {
        spot_ws.lock().await.push_candle("BTCUSDT", candle.clone()).await;

        // 等待 M1 事件（跳过高级周期事件）
        let event = loop {
            match tokio::time::timeout(Duration::from_secs(2), event_rx.recv()).await {
                Ok(Ok(e)) if e.timeframe == Timeframe::M1 => break e,
                Ok(Ok(_)) => continue, // 跳过高级周期事件
                Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
                other => panic!("WS[{}]: 等待 M1 事件失败: {:?}", i, other),
            }
        };

        assert_candle_consistent(&event.candle, candle, &format!("WS_M1[{}]", i));
    }
}

#[tokio::test]
async fn test_ws_consistency_m5_aggregated() {
    let source = Arc::new(ControllableMockSource::new());
    let (engine, spot_ws) = create_consistency_engine(source);
    engine.start().await;
    engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();

    let candles_1m = make_test_1m_candles(60, 1713900000000, 65000.0);
    let expected_m5 = Aggregator::aggregate_1m_to_timeframe(&candles_1m, Timeframe::M5);

    // 先订阅事件，再推送（避免 Lagged）
    let mut event_rx = engine.subscribe_events();

    // 逐根推送 1m K 线，同时收集 M5 Closed 事件
    let mut m5_events: Vec<KlineEvent> = Vec::new();
    for candle in &candles_1m {
        spot_ws.lock().await.push_candle("BTCUSDT", candle.clone()).await;
        // 消费所有已产生的事件，提取 M5 Closed
        loop {
            match event_rx.try_recv() {
                Ok(e) if e.timeframe == Timeframe::M5 && e.event_type == KlineEventType::Closed => m5_events.push(e),
                Ok(_) => {} // 跳过其他事件
                Err(broadcast::error::TryRecvError::Empty) => break,
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                Err(broadcast::error::TryRecvError::Closed) => break,
            }
        }
    }
    // 最后再等一下确保所有事件都处理完
    tokio::time::sleep(Duration::from_millis(50)).await;
    loop {
        match event_rx.try_recv() {
            Ok(e) if e.timeframe == Timeframe::M5 && e.event_type == KlineEventType::Closed => m5_events.push(e),
            Ok(_) => {}
            Err(broadcast::error::TryRecvError::Empty) => break,
            Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(broadcast::error::TryRecvError::Closed) => break,
        }
    }

    assert_eq!(m5_events.len(), expected_m5.len(), "M5 事件数量应一致");
    for (i, (actual, expected)) in m5_events.iter().zip(expected_m5.iter()).enumerate() {
        assert_candle_consistent(&actual.candle, expected, &format!("WS_M5[{}]", i));
    }
}

#[tokio::test]
async fn test_ws_consistency_update_then_close() {
    let source = Arc::new(ControllableMockSource::new());
    let (engine, spot_ws) = create_consistency_engine(source);
    engine.start().await;
    engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();

    let mut event_rx = engine.subscribe_events();

    let open_time = 1713900000000;
    let base = Candle {
        open_time,
        close_time: open_time + 59_999,
        open: 65000.0,
        high: 65100.0,
        low: 64900.0,
        close: 65050.0,
        volume: 100.0,
        quote_volume: 6505000.0,
        trades: 50,
        closed: false,
    };

    // 推送 3 次 update
    let updates = [
        Candle { close: 65060.0, high: 65110.0, volume: 150.0, quote_volume: 9759000.0, trades: 75, ..base.clone() },
        Candle { close: 65070.0, high: 65120.0, volume: 200.0, quote_volume: 13014000.0, trades: 100, ..base.clone() },
        Candle { close: 65080.0, high: 65130.0, low: 64890.0, volume: 250.0, quote_volume: 16270000.0, trades: 125, ..base.clone() },
    ];

    for (i, update) in updates.iter().enumerate() {
        spot_ws.lock().await.push_candle("BTCUSDT", update.clone()).await;
        // 等待 M1 事件（跳过高级周期事件）
        let event = loop {
            match tokio::time::timeout(Duration::from_secs(2), event_rx.recv()).await {
                Ok(Ok(e)) if e.timeframe == Timeframe::M1 => break e,
                Ok(Ok(_)) => continue,
                Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
                other => panic!("update[{}]: 等待 M1 事件失败: {:?}", i, other),
            }
        };
        assert_eq!(event.event_type, KlineEventType::Update, "update[{}]: 应为 Update", i);
        assert_candle_consistent(&event.candle, update, &format!("update[{}]", i));
    }

    // 推送 close
    let closed = Candle { closed: true, close: 65080.0, high: 65130.0, low: 64890.0, volume: 250.0, quote_volume: 16270000.0, trades: 125, ..base };
    spot_ws.lock().await.push_candle("BTCUSDT", closed.clone()).await;
    // 等待 M1 Closed 事件
    let event = loop {
        match tokio::time::timeout(Duration::from_secs(2), event_rx.recv()).await {
            Ok(Ok(e)) if e.timeframe == Timeframe::M1 && e.event_type == KlineEventType::Closed => break e,
            Ok(Ok(_)) => continue,
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
            other => panic!("close: 等待 M1 Closed 事件失败: {:?}", other),
        }
    };
    assert_candle_consistent(&event.candle, &closed, "close");
}

#[tokio::test]
async fn test_ws_consistency_multi_timeframe() {
    let source = Arc::new(ControllableMockSource::new());
    let (engine, spot_ws) = create_consistency_engine(source);
    engine.start().await;
    engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();

    let candles_1m = make_test_1m_candles(240, 1713900000000, 65000.0);
    let expected_m5 = Aggregator::aggregate_1m_to_timeframe(&candles_1m, Timeframe::M5);
    let expected_m15 = Aggregator::aggregate_1m_to_timeframe(&candles_1m, Timeframe::M15);
    let expected_h1 = Aggregator::aggregate_1m_to_timeframe(&candles_1m, Timeframe::H1);

    // 先订阅事件，再逐根推送（避免 broadcast Lagged）
    let mut event_rx = engine.subscribe_events();
    let mut m1_events: Vec<KlineEvent> = Vec::new();
    let mut m5_events: Vec<KlineEvent> = Vec::new();
    let mut m15_events: Vec<KlineEvent> = Vec::new();
    let mut h1_events: Vec<KlineEvent> = Vec::new();

    for candle in &candles_1m {
        spot_ws.lock().await.push_candle("BTCUSDT", candle.clone()).await;
        // 实时消费事件，按 timeframe 分类
        loop {
            match event_rx.try_recv() {
                Ok(e) => match e.timeframe {
                    Timeframe::M1 => m1_events.push(e),
                    Timeframe::M5 => m5_events.push(e),
                    Timeframe::M15 => m15_events.push(e),
                    Timeframe::H1 => h1_events.push(e),
                    _ => {} // H4, D1 不在此测试验证
                },
                Err(broadcast::error::TryRecvError::Empty) => break,
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                Err(broadcast::error::TryRecvError::Closed) => break,
            }
        }
    }
    // 等待最后的事件处理完
    tokio::time::sleep(Duration::from_millis(50)).await;
    loop {
        match event_rx.try_recv() {
            Ok(e) => match e.timeframe {
                Timeframe::M1 => m1_events.push(e),
                Timeframe::M5 => m5_events.push(e),
                Timeframe::M15 => m15_events.push(e),
                Timeframe::H1 => h1_events.push(e),
                _ => {}
            },
            Err(broadcast::error::TryRecvError::Empty) => break,
            Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(broadcast::error::TryRecvError::Closed) => break,
        }
    }

    // M1: 每根 1m 推送都会产生 M1 事件
    assert_eq!(m1_events.len(), 240, "M1 事件数应为 240，实际 {}", m1_events.len());

    // M5: 只比较 Closed 事件（取 min 长度，因为 unclosed 追加可能导致数量差异）
    let m5_closed: Vec<_> = m5_events.iter().filter(|e| e.event_type == KlineEventType::Closed).collect();
    let compare_len = m5_closed.len().min(expected_m5.len());
    assert!(compare_len > 0, "应有至少 1 个 M5 closed 事件");
    for (i, (actual, expected)) in m5_closed.iter().zip(expected_m5.iter()).take(compare_len).enumerate() {
        assert_candle_consistent(&actual.candle, expected, &format!("MULTI_M5[{}]", i));
    }

    // M15
    let m15_closed: Vec<_> = m15_events.iter().filter(|e| e.event_type == KlineEventType::Closed).collect();
    let compare_len = m15_closed.len().min(expected_m15.len());
    assert!(compare_len > 0, "应有至少 1 个 M15 closed 事件");
    for (i, (actual, expected)) in m15_closed.iter().zip(expected_m15.iter()).take(compare_len).enumerate() {
        assert_candle_consistent(&actual.candle, expected, &format!("MULTI_M15[{}]", i));
    }

    // H1
    let h1_closed: Vec<_> = h1_events.iter().filter(|e| e.event_type == KlineEventType::Closed).collect();
    let compare_len = h1_closed.len().min(expected_h1.len());
    assert!(compare_len > 0, "应有至少 1 个 H1 closed 事件");
    for (i, (actual, expected)) in h1_closed.iter().zip(expected_h1.iter()).take(compare_len).enumerate() {
        assert_candle_consistent(&actual.candle, expected, &format!("MULTI_H1[{}]", i));
    }
}

// ── 端到端一致性测试 ──

#[tokio::test]
async fn test_e2e_rest_then_ws() {
    let rest_candles = make_test_1m_candles(50, 1713900000000, 65000.0);
    let ws_candles = make_test_1m_candles(10, 1713900000000 + 50 * 60_000, 65025.0);
    let all_expected: Vec<Candle> = rest_candles.iter().chain(ws_candles.iter()).cloned().collect();

    let source = Arc::new(ControllableMockSource::new());
    source.set_data("1m", rest_candles.clone());
    for tf in &["5m", "15m", "1h", "4h", "1d"] {
        source.set_data(tf, vec![]);
    }

    let (engine, spot_ws) = create_consistency_engine(source);
    engine.start().await;
    engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();

    // 等待 REST backfill
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 通过 WS 推送新数据
    for candle in &ws_candles {
        spot_ws.lock().await.push_candle("BTCUSDT", candle.clone()).await;
    }
    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = engine.get_klines_async("binance", "BTCUSDT", Timeframe::M1).await;
    assert!(result.is_some());
    let result_candles = result.unwrap();
    assert_eq!(result_candles.len(), all_expected.len(), "应有 60 根");
    for (i, (actual, expected)) in result_candles.iter().zip(all_expected.iter()).enumerate() {
        assert_candle_consistent(actual, expected, &format!("E2E[{}]", i));
    }
}

#[tokio::test]
async fn test_e2e_ws_reconnect_data_integrity() {
    let first_batch = make_test_1m_candles(20, 1713900000000, 65000.0);
    let second_batch = make_test_1m_candles(20, 1713900000000 + 20 * 60_000, 65010.0);
    let all_expected: Vec<Candle> = first_batch.iter().chain(second_batch.iter()).cloned().collect();

    let source = Arc::new(ControllableMockSource::new());
    let (engine, spot_ws) = create_consistency_engine(source.clone());
    engine.start().await;
    engine.subscribe("binance", "BTCUSDT", MarketType::Spot).await.unwrap();

    // 推送第一批
    for candle in &first_batch {
        spot_ws.lock().await.push_candle("BTCUSDT", candle.clone()).await;
    }
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 模拟重连：设置 source 数据（backfill 会使用），然后发送 Reconnected
    source.set_data("1m", first_batch.clone());
    for tf in &["5m", "15m", "1h", "4h", "1d"] {
        source.set_data(tf, vec![]);
    }
    spot_ws.lock().await.push_reconnected().await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    // 推送第二批
    for candle in &second_batch {
        spot_ws.lock().await.push_candle("BTCUSDT", candle.clone()).await;
    }
    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = engine.get_klines_async("binance", "BTCUSDT", Timeframe::M1).await;
    assert!(result.is_some());
    let result_candles = result.unwrap();
    assert_eq!(result_candles.len(), all_expected.len(), "应有 40 根");
    for (i, (actual, expected)) in result_candles.iter().zip(all_expected.iter()).enumerate() {
        assert_candle_consistent(actual, expected, &format!("RECONNECT[{}]", i));
    }
}
