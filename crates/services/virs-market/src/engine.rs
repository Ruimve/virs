use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::sync::{broadcast, Mutex};
use virs_error::VirsResult;
use virs_task::{spawn, spawn_periodic, Stop, TaskHandle};

use crate::aggregator::Aggregator;
use crate::cache::SymbolCache;
use crate::gap::GapDetector;
use crate::types::{
    subscription_key, KlineEngineConfig, KlinePersistence, KlineSource, KlineWsClient, WsEvent,
};
use virs_type::{Candle, KlineEngineHandle, KlineEvent, KlineEventSource, KlineEventType, MarketType, Timeframe};

/* 默认空实现：当未配置持久化时，直接丢弃K线数据不报错 */
struct NoOpPersistence;

#[async_trait]
impl KlinePersistence for NoOpPersistence {
    async fn save_candles(
        &self,
        _exchange: &str,
        _symbol: &str,
        _timeframe: &str,
        _candles: &[Candle],
    ) -> VirsResult<()> {
        Ok(())
    }
}

struct SubscriptionEntry {
    exchange: String,
    symbol: String,
    market_type: MarketType,
    cache: Arc<Mutex<SymbolCache>>,
}

struct MarketWsHandler {
    ws: Arc<Mutex<dyn KlineWsClient>>,
}

impl MarketWsHandler {
    fn new(ws: Arc<Mutex<dyn KlineWsClient>>) -> Self {
        Self { ws }
    }

    async fn start(&self, update_tx: broadcast::Sender<WsEvent>) {
        let mut ws = self.ws.lock().await;
        ws.start(update_tx).await;
    }

    async fn stop(&self) {
        let mut ws = self.ws.lock().await;
        ws.stop().await;
    }

    async fn subscribe(&self, symbol: &str) {
        let ws = self.ws.lock().await;
        ws.subscribe(symbol).await;
    }

    async fn _is_running(&self) -> bool {
        let ws = self.ws.lock().await;
        ws.is_running()
    }
}

pub(crate) struct KlineEngine {
    config: KlineEngineConfig,
    source: Arc<dyn KlineSource>,
    persistence: Arc<dyn KlinePersistence>,
    subscriptions: Arc<DashMap<String, SubscriptionEntry>>,
    symbol_index: Arc<DashMap<String, String>>,
    event_tx: broadcast::Sender<KlineEvent>,
    perpetual_handler: MarketWsHandler,
    started: Arc<std::sync::atomic::AtomicBool>,
    ws_loop_task: std::sync::Mutex<Option<TaskHandle>>,
    gap_detection_task: std::sync::Mutex<Option<TaskHandle>>,
}

impl KlineEngine {
    pub(crate) fn new(
        config: KlineEngineConfig,
        source: Arc<dyn KlineSource>,
        perpetual_ws: Arc<Mutex<dyn KlineWsClient>>,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(config.event_channel_capacity);

        Self {
            config,
            source,
            persistence: Arc::new(NoOpPersistence),
            subscriptions: Arc::new(DashMap::new()),
            symbol_index: Arc::new(DashMap::new()),
            event_tx,
            perpetual_handler: MarketWsHandler::new(perpetual_ws),
            started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            ws_loop_task: std::sync::Mutex::new(None),
            gap_detection_task: std::sync::Mutex::new(None),
        }
    }

    pub(crate) fn subscribe_events(&self) -> broadcast::Receiver<KlineEvent> {
        self.event_tx.subscribe()
    }

    pub(crate) async fn start(&self) {
        /* CAS保证start幂等：已启动则直接返回，避免重复创建WS循环和gap检测任务 */
        if self
            .started
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }

        let event_tx = self.event_tx.clone();
        let subscriptions = self.subscriptions.clone();
        let symbol_index = self.symbol_index.clone();
        let source = self.source.clone();
        let persistence = self.persistence.clone();

        let gap_check_subscriptions = subscriptions.clone();
        let gap_check_source = source.clone();
        let gap_check_event_tx = self.event_tx.clone();

        let (ws_update_tx, mut ws_update_rx) = broadcast::channel::<WsEvent>(512);

        self.perpetual_handler.start(ws_update_tx).await;

        let handle = spawn("kline_ws_loop", move |stop: Stop| async move {
            loop {
                tokio::select! {
                    _ = stop.cancelled() => break,
                    result = ws_update_rx.recv() => {
                        match result {
                            Ok(WsEvent::Reconnected) => {
                                /* WS重连后，对所有已订阅symbol触发回填以补齐断连期间缺失的K线 */
                                let entries: Vec<_> = subscriptions
                                    .iter()
                                    .map(|e| {
                                        let sub = e.value();
                                        (
                                            sub.exchange.clone(),
                                            sub.symbol.clone(),
                                            sub.cache.clone(),
                                            sub.market_type,
                                        )
                                    })
                                    .collect();
                                for (exchange, symbol, cache, market_type) in entries {
                                    match GapDetector::detect_and_backfill(
                                        &exchange,
                                        &symbol,
                                        &cache,
                                        &source,
                                        &event_tx,
                                        market_type,
                                    )
                                    .await
                                    {
                                        Ok(_) => {}
                                        Err(e) => {
                                            tracing::error!(exchange = %exchange, symbol = %symbol, error = %e, "Post-reconnect backfill failed");
                                        }
                                    }
                                }
                            }
                            Ok(WsEvent::Candle(update)) => {
                                let symbol = update.symbol;
                                let sub_key = match symbol_index.get(&symbol).map(|r| r.value().clone()) {
                                    Some(key) => key,
                                    None => continue,
                                };

                                let cache = match subscriptions.get(&sub_key) {
                                    Some(entry) => entry.cache.clone(),
                                    None => continue,
                                };

                                let candle_1m = update.candle;
                                let is_closed = candle_1m.closed;

                                /* 更新1m缓存，若K线已关闭则标记关闭状态，并聚合生成更高周期K线 */
                                let (exchange, persist_data, higher_updates) = {
                                    let mut guard = cache.lock().await;
                                    guard.update_candle(Timeframe::M1, candle_1m.clone());
                                    if is_closed {
                                        guard.close_candle(Timeframe::M1, candle_1m.open_time);
                                    }
                                    let higher_updates =
                                        Aggregator::update_higher_timeframes(&candle_1m, &mut guard);
                                    let exchange = match subscriptions.get(&sub_key) {
                                        Some(e) => e.exchange.clone(),
                                        None => continue,
                                    };
                                    let persist_data = if is_closed {
                                        Some(guard.get_klines(Timeframe::M1))
                                    } else {
                                        None
                                    };
                                    (exchange, persist_data, higher_updates)
                                };

                                let event_type = if is_closed {
                                    KlineEventType::Closed
                                } else {
                                    KlineEventType::Update
                                };

                                /* 有接收者时广播K线事件：先发1m事件，再发更高周期聚合事件 */
                                if event_tx.receiver_count() > 0 {
                                    if event_tx
                                        .send(KlineEvent {
                                            exchange: exchange.clone(),
                                            symbol: symbol.clone(),
                                            timeframe: Timeframe::M1,
                                            candle: candle_1m.clone(),
                                            event_type,
                                        })
                                        .is_err()
                                    {
                                        tracing::debug!(
                                            exchange = %exchange,
                                            symbol = %symbol,
                                            "KlineEvent (M1) broadcast — receiver dropped between check and send"
                                        );
                                    }

                                    for (tf, candle) in higher_updates {
                                        let ht_event_type = if candle.closed {
                                            KlineEventType::Closed
                                        } else {
                                            KlineEventType::Update
                                        };
                                        if event_tx
                                            .send(KlineEvent {
                                                exchange: exchange.clone(),
                                                symbol: symbol.clone(),
                                                timeframe: tf,
                                                candle,
                                                event_type: ht_event_type,
                                            })
                                            .is_err()
                                        {
                                            tracing::debug!(
                                                exchange = %exchange,
                                                symbol = %symbol,
                                                "KlineEvent (higher tf) broadcast — receiver dropped between check and send"
                                            );
                                        }
                                    }
                                }

                                if let Some(data) = persist_data {
                                    if let Err(e) = persistence
                                        .save_candles(&exchange, &symbol, "1m", data.as_slice())
                                        .await
                                    {
                                        tracing::warn!(
                                            exchange = %exchange,
                                            symbol = %symbol,
                                            error = %e,
                                            "Failed to save candles"
                                        );
                                    }
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                tracing::warn!(lagged = n, "WS update lagged");
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                break;
                            }
                        }
                    }
                }
            }
        });
        *self.ws_loop_task.lock().unwrap() = Some(handle);

        /* 周期性gap检测：每60秒检查所有订阅symbol的K线连续性，发现缺口自动回填 */
        let gap_handle = spawn_periodic(
            "kline_gap_detection",
            Duration::from_secs(60),
            false,
            move || {
                let gap_check_subscriptions = gap_check_subscriptions.clone();
                let gap_check_source = gap_check_source.clone();
                let gap_check_event_tx = gap_check_event_tx.clone();
                async move {
                    let entries: Vec<_> = gap_check_subscriptions
                        .iter()
                        .map(|e| {
                            let sub = e.value();
                            (
                                sub.exchange.clone(),
                                sub.symbol.clone(),
                                sub.cache.clone(),
                                sub.market_type,
                            )
                        })
                        .collect();
                    for (exchange, symbol, cache, market_type) in entries {
                        let report = GapDetector::check_continuity(&exchange, &symbol, &cache).await;

                        if !report.is_continuous {
                            tracing::debug!(
                                exchange = %exchange,
                                symbol = %symbol,
                                gap_start = ?report.gap_start,
                                gap_end = ?report.gap_end,
                                missing_minutes = report.missing_minutes,
                                "Continuity check detected gap, triggering backfill"
                            );
                            match GapDetector::detect_and_backfill(
                                &exchange,
                                &symbol,
                                &cache,
                                &gap_check_source,
                                &gap_check_event_tx,
                                market_type,
                            )
                            .await
                            {
                                Ok(_) => {}
                                Err(e) => {
                                    tracing::error!(
                                        exchange = %exchange,
                                        symbol = %symbol,
                                        error = %e,
                                        "Backfill failed"
                                    );
                                }
                            }
                        }
                    }
                }
            },
        );
        *self.gap_detection_task.lock().unwrap() = Some(gap_handle);
    }

    pub(crate) async fn stop(&self) {
        /* CAS保证stop幂等：已停止则直接返回 */
        if !self
            .started
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }

        let ws_h = self.ws_loop_task.lock().unwrap().take();
        let gap_h = self.gap_detection_task.lock().unwrap().take();

        /* 先发送cancel信号，再join等待任务退出，确保资源完全释放 */
        if let Some(h) = &ws_h {
            h.cancel();
        }
        if let Some(h) = &gap_h {
            h.cancel();
        }

        let mut join_set = tokio::task::JoinSet::new();
        if let Some(h) = ws_h {
            join_set.spawn(h.join());
        }
        if let Some(h) = gap_h {
            join_set.spawn(h.join());
        }
        while join_set.join_next().await.is_some() {}

        self.perpetual_handler.stop().await;
    }

    pub(crate) async fn subscribe(
        &self,
        exchange: &str,
        symbol: &str,
        market_type: MarketType,
    ) -> VirsResult<()> {
        /* 引擎未启动时自动启动，延迟初始化 */
        if !self.started.load(std::sync::atomic::Ordering::Relaxed) {
            self.start().await;
        }

        let key = subscription_key(exchange, symbol);

        /* 已订阅则去重返回，避免重复订阅和重复回填 */
        if self.subscriptions.contains_key(&key) {
            return Ok(());
        }

        let cache = Arc::new(Mutex::new(SymbolCache::new()));
        let entry = SubscriptionEntry {
            exchange: exchange.to_string(),
            symbol: symbol.to_string(),
            market_type,
            cache: cache.clone(),
        };

        self.subscriptions.insert(key.clone(), entry);
        self.symbol_index.insert(symbol.to_string(), key);

        self.perpetual_handler.subscribe(symbol).await;

        /* 配置开启时，订阅后立即检测并回填历史缺口，确保K线连续性 */
        if self.config.backfill_on_start {
            GapDetector::detect_and_backfill(
                exchange,
                symbol,
                &cache,
                &self.source,
                &self.event_tx,
                market_type,
            )
            .await?;
        }

        Ok(())
    }

    pub(crate) async fn get_klines_async(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: Timeframe,
    ) -> Option<Vec<Candle>> {
        let key = subscription_key(exchange, symbol);
        match self.subscriptions.get(&key) {
            Some(entry) => {
                let guard = entry.cache.lock().await;
                Some(guard.get_klines(timeframe))
            }
            None => None,
        }
    }
}

/* KlineEngine实现KlineEventSource trait，供上层以trait object方式订阅K线事件 */
impl KlineEventSource for KlineEngine {
    fn subscribe_kline_events(&self) -> broadcast::Receiver<KlineEvent> {
        self.subscribe_events()
    }
}

#[async_trait]
/* KlineEngine实现KlineEngineHandle trait，提供订阅行情、停止引擎、查询K线等对外接口 */
impl KlineEngineHandle for KlineEngine {
    async fn subscribe_market(
        &self,
        exchange: &str,
        symbol: &str,
        market_type: MarketType,
    ) -> VirsResult<()> {
        self.subscribe(exchange, symbol, market_type).await
    }

    async fn stop(&self) {
        KlineEngine::stop(self).await
    }

    async fn get_klines(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: Timeframe,
    ) -> Option<Vec<Candle>> {
        self.get_klines_async(exchange, symbol, timeframe).await
    }
}


/* 工厂函数：创建KlineEngine并返回trait object，隐藏内部实现细节 */
pub fn create_kline_engine(
    config: KlineEngineConfig,
    source: Arc<dyn KlineSource>,
    perpetual_ws: Arc<Mutex<dyn KlineWsClient>>,
) -> Arc<dyn KlineEngineHandle> {
    Arc::new(KlineEngine::new(config, source, perpetual_ws))
}
