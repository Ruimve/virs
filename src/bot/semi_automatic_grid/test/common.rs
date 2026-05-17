use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use crate::bot::semi_automatic_grid::ai::GridAiService;
use crate::bot::semi_automatic_grid::ports::*;

pub struct MockPriceProvider {
    pub price: f64,
}

impl MockPriceProvider {
    pub fn new(price: f64) -> Self {
        Self { price }
    }
}

#[async_trait]
impl PriceProvider for MockPriceProvider {
    async fn get_price(&self, _exchange: &str, _symbol: &str) -> Option<f64> {
        if self.price > 0.0 {
            Some(self.price)
        } else {
            None
        }
    }
}

pub struct MockOrderExecutor {
    pub commands: Arc<Mutex<Vec<OrderCommand>>>,
    pub should_fail: bool,
}

impl MockOrderExecutor {
    pub fn new() -> Self {
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
            should_fail: false,
        }
    }

    pub fn failing() -> Self {
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
            should_fail: true,
        }
    }

    pub async fn commands(&self) -> Vec<OrderCommand> {
        self.commands.lock().await.clone()
    }
}

#[async_trait]
impl OrderExecutor for MockOrderExecutor {
    async fn send_command(&self, cmd: OrderCommand) -> anyhow::Result<()> {
        if self.should_fail {
            anyhow::bail!("mock order executor failure");
        }
        self.commands.lock().await.push(cmd);
        Ok(())
    }
}

pub struct MockWorkerStore {
    pub trades: Vec<GridTradeRecord>,
    pub bot: Option<GridBotConfig>,
    pub recorded_trades: Arc<Mutex<Vec<(Uuid, String, i32, f64, f64, f64)>>>,
    pub stats_saved: Arc<Mutex<Vec<(Uuid, f64, f64, i32, i32)>>>,
    pub statuses_updated: Arc<Mutex<Vec<(Uuid, String)>>>,
    pub grid_params_updated: Arc<Mutex<Vec<(Uuid, f64, f64)>>>,
    pub quantities_updated: Arc<Mutex<Vec<(Uuid, f64)>>>,
    pub deleted_bots: Arc<Mutex<Vec<Uuid>>>,
    pub last_adjusted: Arc<Mutex<Vec<Uuid>>>,
    pub should_fail_load: bool,
}

impl MockWorkerStore {
    pub fn new() -> Self {
        Self {
            trades: vec![],
            bot: None,
            recorded_trades: Arc::new(Mutex::new(Vec::new())),
            stats_saved: Arc::new(Mutex::new(Vec::new())),
            statuses_updated: Arc::new(Mutex::new(Vec::new())),
            grid_params_updated: Arc::new(Mutex::new(Vec::new())),
            quantities_updated: Arc::new(Mutex::new(Vec::new())),
            deleted_bots: Arc::new(Mutex::new(Vec::new())),
            last_adjusted: Arc::new(Mutex::new(Vec::new())),
            should_fail_load: false,
        }
    }

    pub fn failing() -> Self {
        Self {
            should_fail_load: true,
            ..Self::new()
        }
    }

    pub fn with_trades(mut self, trades: Vec<GridTradeRecord>) -> Self {
        self.trades = trades;
        self
    }

    pub fn with_bot(mut self, bot: GridBotConfig) -> Self {
        self.bot = Some(bot);
        self
    }
}

#[async_trait]
impl GridStore for MockWorkerStore {
    async fn load_running_bots(&self) -> anyhow::Result<Vec<GridBotConfig>> {
        Ok(vec![])
    }

    async fn load_bot(&self, _bot_id: Uuid) -> anyhow::Result<Option<GridBotConfig>> {
        if self.should_fail_load {
            anyhow::bail!("mock store failure");
        }
        Ok(self.bot.clone())
    }

    async fn load_trades(&self, _bot_id: Uuid) -> anyhow::Result<Vec<GridTradeRecord>> {
        if self.should_fail_load {
            anyhow::bail!("mock store failure");
        }
        Ok(self.trades.clone())
    }

    async fn record_trade(
        &self,
        bot_id: Uuid,
        _user_id: Uuid,
        _symbol: &str,
        _exchange: &str,
        side: &str,
        grid_level: i32,
        price: f64,
        quantity: f64,
        pnl: f64,
        _pnl_pct: f64,
    ) -> anyhow::Result<()> {
        self.recorded_trades.lock().await.push((bot_id, side.to_string(), grid_level, price, quantity, pnl));
        Ok(())
    }

    async fn save_stats(&self, bot_id: Uuid, total_pnl: f64, unrealized_pnl: f64, total_trades: i32, grid_filled_count: i32, _levels_json: Option<&serde_json::Value>) -> anyhow::Result<()> {
        self.stats_saved.lock().await.push((bot_id, total_pnl, unrealized_pnl, total_trades, grid_filled_count));
        Ok(())
    }

    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> anyhow::Result<()> {
        self.statuses_updated.lock().await.push((bot_id, status.to_string()));
        Ok(())
    }

    async fn update_last_adjusted(&self, bot_id: Uuid) -> anyhow::Result<()> {
        self.last_adjusted.lock().await.push(bot_id);
        Ok(())
    }

    async fn update_grid_params(&self, bot_id: Uuid, upper_price: f64, lower_price: f64) -> anyhow::Result<()> {
        self.grid_params_updated.lock().await.push((bot_id, upper_price, lower_price));
        Ok(())
    }

    async fn update_quantity_per_grid(&self, bot_id: Uuid, quantity: f64) -> anyhow::Result<()> {
        self.quantities_updated.lock().await.push((bot_id, quantity));
        Ok(())
    }

    async fn update_ai_analysis(
        &self,
        _bot_id: Uuid,
        _market_regime: &str,
        _upper_price: f64,
        _lower_price: f64,
        _grid_count: i32,
        _grid_profit_pct: f64,
        _quantity_per_grid: f64,
        _leverage: i32,
        _ai_analysis: &str,
        _grid_levels_json: Option<&serde_json::Value>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn save_analysis_log(
        &self,
        _bot_id: Uuid,
        _analysis_type: &str,
        _system_prompt: &str,
        _user_prompt: &str,
        _result: &serde_json::Value,
        _error: Option<&str>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn load_analysis_logs(&self, _bot_id: Uuid) -> anyhow::Result<Vec<AnalysisLogEntry>> {
        Ok(vec![])
    }

    async fn delete_bot(&self, bot_id: Uuid) -> anyhow::Result<()> {
        self.deleted_bots.lock().await.push(bot_id);
        Ok(())
    }
}

pub struct MockEngineStore {
    pub bots: Arc<Mutex<Vec<GridBotConfig>>>,
    pub deleted_bots: Arc<Mutex<Vec<Uuid>>>,
    pub statuses: Arc<Mutex<Vec<(Uuid, String)>>>,
    pub should_fail_load: bool,
}

impl MockEngineStore {
    pub fn new() -> Self {
        Self {
            bots: Arc::new(Mutex::new(Vec::new())),
            deleted_bots: Arc::new(Mutex::new(Vec::new())),
            statuses: Arc::new(Mutex::new(Vec::new())),
            should_fail_load: false,
        }
    }

    pub fn failing() -> Self {
        Self {
            should_fail_load: true,
            ..Self::new()
        }
    }

    pub async fn add_bot(&self, bot: GridBotConfig) {
        self.bots.lock().await.push(bot);
    }

    pub async fn update_bot_config(&self, bot: GridBotConfig) {
        let mut bots = self.bots.lock().await;
        if let Some(existing) = bots.iter_mut().find(|b| b.id == bot.id) {
            *existing = bot;
        }
    }
}

#[async_trait]
impl GridStore for MockEngineStore {
    async fn load_running_bots(&self) -> anyhow::Result<Vec<GridBotConfig>> {
        if self.should_fail_load {
            anyhow::bail!("mock store failure");
        }
        Ok(self.bots.lock().await.clone())
    }

    async fn load_bot(&self, bot_id: Uuid) -> anyhow::Result<Option<GridBotConfig>> {
        if self.should_fail_load {
            anyhow::bail!("mock store failure");
        }
        let bots = self.bots.lock().await;
        Ok(bots.iter().find(|b| b.id == bot_id).cloned())
    }

    async fn load_trades(&self, _bot_id: Uuid) -> anyhow::Result<Vec<GridTradeRecord>> {
        Ok(vec![])
    }

    async fn record_trade(
        &self, _bot_id: Uuid, _user_id: Uuid, _symbol: &str, _exchange: &str,
        _side: &str, _grid_level: i32, _price: f64, _quantity: f64, _pnl: f64, _pnl_pct: f64,
    ) -> anyhow::Result<()> { Ok(()) }

    async fn save_stats(&self, _bot_id: Uuid, _total_pnl: f64, _unrealized_pnl: f64, _total_trades: i32, _grid_filled_count: i32, _levels_json: Option<&serde_json::Value>) -> anyhow::Result<()> { Ok(()) }

    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> anyhow::Result<()> {
        self.statuses.lock().await.push((bot_id, status.to_string()));
        Ok(())
    }

    async fn update_last_adjusted(&self, _bot_id: Uuid) -> anyhow::Result<()> { Ok(()) }

    async fn update_grid_params(&self, _bot_id: Uuid, _upper_price: f64, _lower_price: f64) -> anyhow::Result<()> { Ok(()) }

    async fn update_quantity_per_grid(&self, _bot_id: Uuid, _quantity: f64) -> anyhow::Result<()> { Ok(()) }

    async fn update_ai_analysis(
        &self,
        _bot_id: Uuid,
        _market_regime: &str,
        _upper_price: f64,
        _lower_price: f64,
        _grid_count: i32,
        _grid_profit_pct: f64,
        _quantity_per_grid: f64,
        _leverage: i32,
        _ai_analysis: &str,
        _grid_levels_json: Option<&serde_json::Value>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn save_analysis_log(
        &self,
        _bot_id: Uuid,
        _analysis_type: &str,
        _system_prompt: &str,
        _user_prompt: &str,
        _result: &serde_json::Value,
        _error: Option<&str>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn load_analysis_logs(&self, _bot_id: Uuid) -> anyhow::Result<Vec<AnalysisLogEntry>> {
        Ok(vec![])
    }

    async fn delete_bot(&self, bot_id: Uuid) -> anyhow::Result<()> {
        self.deleted_bots.lock().await.push(bot_id);
        Ok(())
    }
}

pub struct MockLlmResolver {
    pub available: bool,
}

impl MockLlmResolver {
    pub fn new(available: bool) -> Self {
        Self { available }
    }
}

impl LlmProviderResolver for MockLlmResolver {
    fn is_available(&self) -> bool {
        self.available
    }

    fn resolve(&self, _user_credentials: &[(String, String)]) -> anyhow::Result<(String, String, String, String)> {
        if self.available {
            Ok((
                "test-key".to_string(),
                "https://api.test.com".to_string(),
                "test-model".to_string(),
                "test-provider".to_string(),
            ))
        } else {
            anyhow::bail!("not available")
        }
    }
}

pub struct MockCredentialStore {
    pub credentials: Vec<(String, String)>,
}

impl MockCredentialStore {
    pub fn new() -> Self {
        Self { credentials: vec![] }
    }

    pub fn with_creds(mut self, creds: Vec<(String, String)>) -> Self {
        self.credentials = creds;
        self
    }
}

#[async_trait]
impl CredentialStore for MockCredentialStore {
    async fn load_credentials(&self, _user_id: Uuid) -> anyhow::Result<Vec<(String, String)>> {
        Ok(self.credentials.clone())
    }
}

pub fn make_bot_config() -> GridBotConfig {
    GridBotConfig {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        name: "Test Bot".to_string(),
        symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(),
        grid_count: 10,
        upper_price: 60000.0,
        lower_price: 50000.0,
        grid_profit_pct: 0.5,
        quantity_per_grid: 100.0,
        leverage: 10,
        dynamic_adjust: false,
        adjust_interval_secs: 300,
        market_regime: None,
        grid_levels_json: None,
        system_prompt: None,
    }
}

pub fn make_mock_ai_service() -> Arc<GridAiService> {
    Arc::new(GridAiService::new(
        Box::new(MockLlmResolver::new(false)),
        Box::new(MockCredentialStore::new()),
    ))
}

pub struct MockMarketDataProvider;

#[async_trait]
impl crate::bot::semi_automatic_grid::ports::MarketDataProvider for MockMarketDataProvider {
    async fn get_market_snapshot(&self, _exchange: &str, _symbol: &str) -> crate::bot::semi_automatic_grid::ports::MarketSnapshot {
        use crate::bot::semi_automatic_grid::utils::indicators::MarketIndicators;
        crate::bot::semi_automatic_grid::ports::MarketSnapshot {
            current_price: 55000.0,
            funding_rate: 0.01,
            indicators: MarketIndicators {
                current_price: 55000.0,
                rsi: 50.0,
                atr: 500.0,
                atr_pct: 0.9,
                bb_width: 0.04,
                bb_upper: 56000.0,
                bb_middle: 55000.0,
                bb_lower: 54000.0,
                ema12: 55200.0,
                ema20: 55100.0,
                ema26: 54800.0,
                ema50: 54700.0,
                ema12_trend: "横盘".to_string(),
                ema20_trend: "横盘".to_string(),
                ema26_trend: "横盘".to_string(),
                ema50_trend: "横盘".to_string(),
                price_high: 56000.0,
                price_low: 54000.0,
                volatility: 3.5,
                change_1h: 0.1,
                change_4h: -0.3,
                change_24h: 1.2,
                macd: 50.0,
                macd_signal: 45.0,
                macd_histogram: 5.0,
                adx: 20.0,
                funding_rate: 0.01,
                funding_next_time: "N/A".to_string(),
                h1_atr_sma20: 480.0,
                h1_candle_body: 150.0,
                h1_bars_outside_band: 0,
                h1_bandwidth_5bars_ago: 0.03,
                h1_high_20: 56200.0,
                h1_low_20: 53800.0,
                nearest_round_up: 56000.0,
                nearest_round_down: 55000.0,
                m15_current_price: 55050.0,
                m15_bb_width_pct: 0.03,
                m15_atr: 120.0,
                m15_atr_sma20: 115.0,
                m15_adx: 18.0,
                m15_bars_outside_band: 0,
                m15_ema20: 55020.0,
                m15_ema50: 54900.0,
                h4_ema20: 54500.0,
                h4_ema50: 54200.0,
                h4_adx: 22.0,
                h4_bb_width_pct: 0.05,
            },
        }
    }

    async fn get_account_balance(&self, _exchange: &str) -> super::super::ports::AccountBalance {
        super::super::ports::AccountBalance {
            total: 10000.0,
            free: 8000.0,
            used: 2000.0,
        }
    }
}

pub fn make_worker(bot: GridBotConfig, price: f64) -> crate::bot::semi_automatic_grid::worker::GridWorker {
    let (event_tx, event_rx) = broadcast::channel(16);
    let (grid_event_tx, _) = broadcast::channel(16);
    let price_provider = Arc::new(MockPriceProvider::new(price));
    let order_executor = Arc::new(MockOrderExecutor::new());
    let ai_service = make_mock_ai_service();
    let store = Arc::new(MockWorkerStore::new());
    let market_data_provider = Arc::new(MockMarketDataProvider);

    crate::bot::semi_automatic_grid::worker::GridWorker::new(
        bot,
        price_provider,
        order_executor,
        ai_service,
        store,
        market_data_provider,
        event_rx,
        grid_event_tx,
    )
}

pub fn make_worker_with_store(
    bot: GridBotConfig,
    price: f64,
    store: Arc<MockWorkerStore>,
) -> crate::bot::semi_automatic_grid::worker::GridWorker {
    let (event_tx, event_rx) = broadcast::channel(16);
    let (grid_event_tx, _) = broadcast::channel(16);
    let price_provider = Arc::new(MockPriceProvider::new(price));
    let order_executor = Arc::new(MockOrderExecutor::new());
    let ai_service = make_mock_ai_service();
    let market_data_provider = Arc::new(MockMarketDataProvider);

    crate::bot::semi_automatic_grid::worker::GridWorker::new(
        bot,
        price_provider,
        order_executor,
        ai_service,
        store,
        market_data_provider,
        event_rx,
        grid_event_tx,
    )
}

pub fn make_worker_with_executor(
    bot: GridBotConfig,
    price: f64,
    order_executor: Arc<MockOrderExecutor>,
) -> crate::bot::semi_automatic_grid::worker::GridWorker {
    let (event_tx, event_rx) = broadcast::channel(16);
    let (grid_event_tx, _) = broadcast::channel(16);
    let price_provider = Arc::new(MockPriceProvider::new(price));
    let ai_service = make_mock_ai_service();
    let store = Arc::new(MockWorkerStore::new());
    let market_data_provider = Arc::new(MockMarketDataProvider);

    crate::bot::semi_automatic_grid::worker::GridWorker::new(
        bot,
        price_provider,
        order_executor,
        ai_service,
        store,
        market_data_provider,
        event_rx,
        grid_event_tx,
    )
}
