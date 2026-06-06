//! Auto worker — individual auto trading bot execution.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::auto::ai::{AutoAction, AutoAiService, AutoDecision};
use crate::auto::ports::*;
use crate::auto::strategy;
use crate::auto::types::{AutoBotConfig, AutoEvent, MarketType};

const PENDING_ORDER_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_POSITION_DURATION: Duration = Duration::from_secs(48 * 3600);

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct PendingOpen {
    pub side: String,
    pub entry_price: f64,
    pub position_size: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub client_order_id: String,
    pub sent_at: tokio::time::Instant,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct PendingClose {
    pub side: String,
    pub reason: String,
    pub entry_price: f64,
    pub position_size: f64,
    pub unrealized_pnl: f64,
    pub client_order_id: String,
    pub sent_at: tokio::time::Instant,
}

/// 全自动交易 Worker
pub struct AutoWorker {
    pub(crate) bot: AutoBotConfig,
    price_provider: Arc<dyn PriceProvider>,
    order_executor: Arc<dyn OrderExecutor>,
    ai_service: Arc<AutoAiService>,
    store: Arc<dyn AutoStore>,
    market_data_provider: Arc<dyn MarketDataProvider>,
    event_rx: broadcast::Receiver<OrderEvent>,
    auto_event_tx: broadcast::Sender<AutoEvent>,
    pub(crate) current_price: f64,
    pub(crate) consecutive_losses: i32,
    pub(crate) paused: bool,
    pub(crate) pending_open: Option<PendingOpen>,
    pub(crate) pending_close: Option<PendingClose>,
    pub(crate) position_opened_at: Option<tokio::time::Instant>,
    pub(crate) trailing_stop_dirty: bool,
}

impl AutoWorker {
    pub fn new(
        bot: AutoBotConfig,
        price_provider: Arc<dyn PriceProvider>,
        order_executor: Arc<dyn OrderExecutor>,
        ai_service: Arc<AutoAiService>,
        store: Arc<dyn AutoStore>,
        market_data_provider: Arc<dyn MarketDataProvider>,
        event_rx: broadcast::Receiver<OrderEvent>,
        auto_event_tx: broadcast::Sender<AutoEvent>,
    ) -> Self {
        Self {
            bot,
            price_provider,
            order_executor,
            ai_service,
            store,
            market_data_provider,
            event_rx,
            auto_event_tx,
            current_price: 0.0,
            consecutive_losses: 0,
            paused: false,
            pending_open: None,
            pending_close: None,
            position_opened_at: None,
            trailing_stop_dirty: false,
        }
    }

    pub(crate) fn is_spot(&self) -> bool {
        matches!(self.bot.market_type, MarketType::Spot)
    }

    pub(crate) fn has_position(&self) -> bool {
        self.bot
            .current_side
            .as_ref()
            .map_or(false, |s| !s.is_empty() && s != "none")
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.pending_open.is_some() || self.pending_close.is_some()
    }

    pub(crate) async fn fetch_current_price(&self) -> f64 {
        match self
            .price_provider
            .get_price(&self.bot.exchange, &self.bot.symbol, self.bot.market_type.as_str())
            .await
        {
            Some(price) if price > 0.0 => price,
            _ => self.current_price,
        }
    }

    pub(crate) async fn save_position(&self) {
        let side_str = self.bot.current_side.as_deref().unwrap_or("none");
        let _ = self
            .store
            .update_position(
                self.bot.id,
                Some(side_str),
                self.bot.entry_price,
                self.bot.position_size,
                self.bot.stop_loss,
                self.bot.take_profit,
                self.bot.liquidation_price,
            )
            .await;
    }

    pub(crate) async fn save_stats(&self) {
        let _ = self
            .store
            .update_stats(
                self.bot.id,
                self.bot.total_pnl,
                self.bot.total_trades,
                self.bot.win_trades,
                self.bot.loss_trades,
            )
            .await;
    }

    pub(crate) fn check_pending_timeout(&mut self) {
        let now = tokio::time::Instant::now();
        if let Some(ref pending) = self.pending_open {
            if now.duration_since(pending.sent_at) > PENDING_ORDER_TIMEOUT {
                warn!(bot_id = %self.bot.id, "Pending open order timed out, clearing");
                self.pending_open = None;
            }
        }
        if let Some(ref pending) = self.pending_close {
            if now.duration_since(pending.sent_at) > PENDING_ORDER_TIMEOUT {
                warn!(bot_id = %self.bot.id, "Pending close order timed out, clearing");
                self.pending_close = None;
            }
        }
    }

    pub(crate) fn matches_pending_order(&self, client_order_id: Option<&str>) -> bool {
        let bot_id_str = self.bot.id.to_string();
        match client_order_id {
            Some(cid) => cid.contains(&bot_id_str),
            None => false,
        }
    }

    // ── 主运行循环 ──────────────────────────────────────────

    pub async fn run(&mut self, mut shutdown_rx: tokio::sync::mpsc::Receiver<()>) {
        info!(
            bot_id = %self.bot.id,
            symbol = %self.bot.symbol,
            market_type = %self.bot.market_type,
            "AutoWorker starting"
        );

        // 获取初始价格
        for attempt in 1..=10 {
            self.current_price = self.fetch_current_price().await;
            if self.current_price > 0.0 {
                break;
            }
            warn!(bot_id = %self.bot.id, attempt, "Failed to fetch initial price, retrying in 5s...");
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        if self.current_price <= 0.0 {
            error!(bot_id = %self.bot.id, "Failed to fetch initial price after 10 attempts, setting error status");
            let _ = self.store.update_bot_status(self.bot.id, "error").await;
            let _ = self.auto_event_tx.send(AutoEvent::BotError {
                bot_id: self.bot.id,
                error: "Failed to fetch initial price after 10 attempts".to_string(),
            });
            return;
        }
        info!(bot_id = %self.bot.id, price = self.current_price, "Initial price fetched");

        // 加载连续亏损次数
        match self.store.load_consecutive_losses(self.bot.id).await {
            Ok(losses) => {
                self.consecutive_losses = losses;
                info!(bot_id = %self.bot.id, consecutive_losses = losses, "Loaded consecutive losses from DB");
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to load consecutive losses, starting from 0");
            }
        }

        if self.bot.current_side.is_none() {
            self.bot.current_side = Some("none".to_string());
        }

        // 初始 LLM 分析
        if self.has_position() {
            self.position_opened_at = Some(tokio::time::Instant::now());
            if self.check_stop_take_profit().await {
                info!(bot_id = %self.bot.id, "Stop/take profit triggered on startup, skipping initial LLM analysis");
                self.save_position().await;
            } else {
                info!(bot_id = %self.bot.id, "Performing initial LLM analysis");
                self.on_llm_decision().await;
            }
        } else {
            info!(bot_id = %self.bot.id, "Performing initial LLM analysis");
            self.on_llm_decision().await;
        }

        let mut price_tick = tokio::time::interval(Duration::from_secs(5));

        // LLM 周期性决策定时器
        let (llm_signal_tx, mut llm_signal_rx) = tokio::sync::mpsc::channel::<()>(1);
        {
            let interval_secs = self.bot.decide_interval_secs.max(60) as u64;
            info!(bot_id = %self.bot.id, interval_secs, "LLM periodic decision enabled");
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(interval_secs));
                tick.tick().await;
                loop {
                    tick.tick().await;
                    if llm_signal_tx.send(()).await.is_err() {
                        break;
                    }
                }
            });
        }

        // 主事件循环
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!(bot_id = %self.bot.id, "AutoWorker shutting down");
                    break;
                }
                _ = price_tick.tick() => {
                    self.current_price = self.fetch_current_price().await;
                    if !self.paused {
                        self.on_price_tick().await;
                    }
                }
                Some(()) = llm_signal_rx.recv() => {
                    self.on_llm_decision().await;
                }
                event = self.event_rx.recv() => {
                    match event {
                        Ok(event) => self.on_order_event(event).await,
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(bot_id = %self.bot.id, lagged = n, "Event lagged");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            warn!(bot_id = %self.bot.id, "Event channel closed");
                            break;
                        }
                    }
                }
            }
        }

        self.save_position().await;
        self.save_stats().await;
    }

    // ── 价格 tick 处理 ──────────────────────────────────────

    pub(crate) async fn on_price_tick(&mut self) {
        if self.current_price <= 0.0 {
            return;
        }

        self.check_pending_timeout();

        if self.pending_close.is_some() {
            return;
        }

        if self.has_position() {
            self.update_unrealized_pnl();

            let atr = self.fetch_current_atr().await;
            self.update_trailing_stop(atr);

            if self.check_position_timeout().await {
                return;
            }

            if self.check_stop_take_profit().await {
                return;
            }
        }

        if self.trailing_stop_dirty {
            self.save_position().await;
            self.trailing_stop_dirty = false;
        }

        let _ = self.auto_event_tx.send(AutoEvent::PriceUpdate {
            bot_id: self.bot.id,
            symbol: self.bot.symbol.clone(),
            side: self.bot.current_side.clone().unwrap_or_default(),
            entry_price: self.bot.entry_price,
            position_size: self.bot.position_size,
            current_price: self.current_price,
            unrealized_pnl: self.bot.unrealized_pnl,
            total_pnl: self.bot.total_pnl,
            liquidation_price: self.bot.liquidation_price,
        });
    }

    fn update_unrealized_pnl(&mut self) {
        if !self.has_position() || self.bot.entry_price <= 0.0 {
            return;
        }
        let side = self.bot.current_side.as_deref().unwrap_or("none");
        self.bot.unrealized_pnl = match side {
            "long" => (self.current_price - self.bot.entry_price) * self.bot.position_size,
            "short" => (self.bot.entry_price - self.current_price) * self.bot.position_size,
            _ => 0.0,
        };
    }

    async fn check_stop_take_profit(&mut self) -> bool {
        if !self.has_position() || self.bot.entry_price <= 0.0 {
            return false;
        }

        let side = self.bot.current_side.as_deref().unwrap_or("none");
        let should_close = match side {
            "long" => {
                (self.bot.stop_loss > 0.0 && self.current_price <= self.bot.stop_loss)
                    || (self.bot.take_profit > 0.0 && self.current_price >= self.bot.take_profit)
            }
            "short" => {
                (self.bot.stop_loss > 0.0 && self.current_price >= self.bot.stop_loss)
                    || (self.bot.take_profit > 0.0 && self.current_price <= self.bot.take_profit)
            }
            _ => false,
        };

        if should_close {
            let reason = if self.bot.stop_loss > 0.0
                && ((side == "long" && self.current_price <= self.bot.stop_loss)
                    || (side == "short" && self.current_price >= self.bot.stop_loss))
            {
                "stop_loss"
            } else {
                "take_profit"
            };
            info!(bot_id = %self.bot.id, side = %side, reason = %reason, price = self.current_price, "Stop/take profit triggered");
            self.close_position(reason).await;
            return true;
        }
        false
    }

    fn update_trailing_stop(&mut self, atr: f64) {
        if !self.has_position() || self.bot.entry_price <= 0.0 || self.bot.stop_loss <= 0.0 {
            return;
        }

        if atr <= 0.0 {
            return;
        }

        let side = self.bot.current_side.as_deref().unwrap_or("none");

        let new_stop = strategy::compute_trailing_stop(
            self.bot.entry_price,
            self.current_price,
            side,
            atr,
            self.bot.stop_loss,
        );

        if new_stop != self.bot.stop_loss {
            info!(
                bot_id = %self.bot.id, side = %side,
                old_stop = self.bot.stop_loss, new_stop,
                "Trailing stop updated"
            );
            self.bot.stop_loss = new_stop;
            self.trailing_stop_dirty = true;
        }
    }

    async fn fetch_current_atr(&self) -> f64 {
        let snapshot = self
            .market_data_provider
            .get_market_snapshot(&self.bot.exchange, &self.bot.symbol, self.bot.market_type.as_str())
            .await;
        snapshot.indicators.atr
    }

    async fn check_position_timeout(&mut self) -> bool {
        if let Some(opened_at) = self.position_opened_at {
            if opened_at.elapsed() > MAX_POSITION_DURATION {
                warn!(
                    bot_id = %self.bot.id,
                    duration_secs = opened_at.elapsed().as_secs(),
                    "Position held too long, force closing"
                );
                self.close_position("position_timeout").await;
                return true;
            }
        }
        false
    }

    // ── LLM 决策 ────────────────────────────────────────────

    pub(crate) async fn on_llm_decision(&mut self) {
        info!(bot_id = %self.bot.id, "LLM decision tick");

        if self.is_pending() {
            warn!(bot_id = %self.bot.id, "Pending order in progress, skipping LLM decision");
            return;
        }

        if !self.ai_service.is_available_for_user(self.bot.user_id).await {
            warn!(bot_id = %self.bot.id, "AI service not available, skipping decision");
            let _ = self.auto_event_tx.send(AutoEvent::BotError {
                bot_id: self.bot.id,
                error: "LLM decision skipped: AI service not configured".to_string(),
            });
            return;
        }

        let (system_prompt, user_prompt) = match self.build_llm_prompt().await {
            Some(prompts) => prompts,
            None => return,
        };

        let decision_result = self
            .ai_service
            .auto_decision(self.bot.user_id, &system_prompt, &user_prompt)
            .await;
        let (decision, raw_llm_response) = match decision_result {
            Some((d, raw)) => (Some(d), Some(raw)),
            None => (None, None),
        };

        let action = self
            .handle_llm_result(&decision, &system_prompt, &user_prompt, raw_llm_response.as_ref())
            .await;

        self.execute_decision(&action, decision.as_ref()).await;

        if !matches!(action, AutoAction::Hold) {
            let _ = self.store.update_last_decided(self.bot.id).await;
        }
    }

    async fn build_llm_prompt(&self) -> Option<(String, String)> {
        let snapshot = self
            .market_data_provider
            .get_market_snapshot(&self.bot.exchange, &self.bot.symbol, self.bot.market_type.as_str())
            .await;

        if snapshot.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "Market snapshot has zero price, skipping decision");
            return None;
        }

        let account = self
            .market_data_provider
            .get_account_balance(&self.bot.exchange, self.bot.market_type.as_str())
            .await;
        let margin_usage_rate = if account.total > 0.0 {
            account.used / account.total
        } else {
            0.0
        };

        let position_info = strategy::format_position_info(
            self.bot.current_side.as_deref(),
            self.bot.entry_price,
            self.bot.position_size,
            snapshot.current_price,
            snapshot.liquidation_price,
        );

        let stop_take_profit_info =
            strategy::format_stop_take_profit(self.bot.stop_loss, self.bot.take_profit);

        let position_duration = if self.has_position() {
            if let Some(opened_at) = self.position_opened_at {
                let elapsed = opened_at.elapsed();
                let hours = elapsed.as_secs() / 3600;
                let mins = (elapsed.as_secs() % 3600) / 60;
                format!("{}小时{}分钟", hours, mins)
            } else {
                "未知".to_string()
            }
        } else {
            "无持仓".to_string()
        };

        let ctx = strategy::PromptContext {
            timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            symbol: self.bot.symbol.clone(),
            exchange: self.bot.exchange.clone(),
            market_type: self.bot.market_type.as_str().to_string(),
            total_balance: account.total,
            available_balance: account.free,
            used_margin: account.used,
            margin_usage_rate,
            leverage: self.bot.leverage,
            position_info,
            position_duration,
            stop_take_profit_info,
            funding_rate: snapshot.funding_rate,
            funding_next_time: snapshot.funding_next_time,
            total_trades: self.bot.total_trades,
            win_trades: self.bot.win_trades,
            loss_trades: self.bot.loss_trades,
            total_pnl: self.bot.total_pnl,
            consecutive_losses: self.consecutive_losses,
            trigger_reason: "scheduled".to_string(),
            ind: snapshot.indicators,
            min_qty: snapshot.min_qty,
        };

        let template = crate::auto::types::DEFAULT_USER_PROMPT_TEMPLATE;
        let user_prompt = strategy::render_prompt(template, &ctx);
        let system_prompt = self
            .bot
            .system_prompt
            .as_deref()
            .unwrap_or(crate::auto::types::DEFAULT_SYSTEM_PROMPT)
            .to_string();

        Some((system_prompt, user_prompt))
    }

    async fn handle_llm_result(
        &mut self,
        decision: &Option<AutoDecision>,
        system_prompt: &str,
        user_prompt: &str,
        raw_llm_response: Option<&serde_json::Value>,
    ) -> AutoAction {
        match decision {
            Some(d) => {
                info!(bot_id = %self.bot.id, action = d.action.as_str(), reason = %d.reason, confidence = d.confidence, "LLM decision");

                if let Some(ref w) = d.funding_rate_warning {
                    warn!(bot_id = %self.bot.id, warning = %w, "Funding rate warning");
                }
                if let Some(ref w) = d.risk_warning {
                    warn!(bot_id = %self.bot.id, warning = %w, "Risk warning");
                }

                let mut result = serde_json::json!({
                    "decision": {
                        "action": d.action.as_str(),
                        "reason": d.reason,
                        "confidence": d.confidence,
                    },
                    "market": {
                        "market_regime": d.market_regime,
                        "funding_rate_warning": d.funding_rate_warning,
                        "event_impact": d.event_impact,
                    },
                    "analysis": d.analysis,
                    "risk_warning": d.risk_warning,
                });
                if let Some(raw) = raw_llm_response {
                    result
                        .as_object_mut()
                        .unwrap()
                        .insert("raw_llm_response".to_string(), raw.clone());
                }
                let _ = self
                    .store
                    .save_analysis_log(self.bot.id, "periodic", system_prompt, user_prompt, &result, None)
                    .await;

                d.action.clone()
            }
            None => {
                warn!(bot_id = %self.bot.id, "LLM call failed, holding position");

                let result = serde_json::json!({
                    "decision": {
                        "action": "hold",
                        "reason": "LLM call failed",
                        "confidence": 0.0,
                    },
                    "market": {
                        "market_regime": null,
                        "funding_rate_warning": null,
                        "event_impact": null,
                    },
                    "analysis": null,
                    "risk_warning": null,
                });
                let _ = self
                    .store
                    .save_analysis_log(
                        self.bot.id,
                        "periodic",
                        system_prompt,
                        user_prompt,
                        &result,
                        Some("LLM call failed"),
                    )
                    .await;

                let _ = self.auto_event_tx.send(AutoEvent::BotError {
                    bot_id: self.bot.id,
                    error: "LLM call failed, holding".to_string(),
                });

                AutoAction::Hold
            }
        }
    }

    pub(crate) async fn execute_decision(
        &mut self,
        action: &AutoAction,
        decision: Option<&AutoDecision>,
    ) {
        if matches!(action, AutoAction::Hold) {
            info!(bot_id = %self.bot.id, "Hold: no action taken, no params applied");
            return;
        }

        if self.is_pending() {
            warn!(bot_id = %self.bot.id, "Pending order in progress, skipping decision execution");
            return;
        }

        if matches!(action, AutoAction::OpenLong | AutoAction::OpenShort) {
            if let Some(d) = decision {
                if d.confidence < 0.6 {
                    warn!(
                        bot_id = %self.bot.id,
                        action = action.as_str(),
                        confidence = d.confidence,
                        "Confidence below 0.6 threshold for opening position, downgrading to Hold"
                    );
                    return;
                }
            }
        }

        let snapshot = self
            .market_data_provider
            .get_market_snapshot(&self.bot.exchange, &self.bot.symbol, self.bot.market_type.as_str())
            .await;

        if snapshot.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "Market snapshot has zero price, skipping decision execution");
            return;
        }

        match action {
            AutoAction::OpenLong | AutoAction::OpenShort => {
                if let Some(d) = decision {
                    self.apply_non_structural_params(d, &snapshot).await;
                }

                let side = match action {
                    AutoAction::OpenLong => "long",
                    AutoAction::OpenShort => "short",
                    _ => unreachable!(),
                };

                if side == "short" && self.is_spot() {
                    warn!(bot_id = %self.bot.id, "Cannot open short on spot market");
                    return;
                }
                if self.has_position() {
                    warn!(bot_id = %self.bot.id, side = %side, "Already has position, cannot open");
                    return;
                }
                self.open_position(side, decision, &snapshot).await;
            }
            AutoAction::ClosePosition => {
                if !self.has_position() {
                    warn!(bot_id = %self.bot.id, "No position to close");
                    return;
                }
                let reason = decision
                    .and_then(|d| d.close_reason.as_deref())
                    .unwrap_or("llm_decision");
                self.close_position(reason).await;
            }
            AutoAction::Hold => unreachable!(),
        }
    }

    async fn apply_non_structural_params(&mut self, d: &AutoDecision, snapshot: &MarketSnapshot) {
        if let Some(ref regime) = d.market_regime {
            self.bot.market_regime = Some(regime.clone());
        }

        let adx = snapshot.indicators.adx;
        let new_leverage = strategy::compute_leverage(adx, self.is_spot());
        self.bot.leverage = new_leverage;

        let _ = self
            .store
            .update_ai_analysis(
                self.bot.id,
                self.bot.market_regime.as_deref().unwrap_or("ranging"),
                self.bot.leverage,
                d.analysis.as_deref().unwrap_or(""),
            )
            .await;
    }

    async fn open_position(
        &mut self,
        side: &str,
        _decision: Option<&AutoDecision>,
        snapshot: &MarketSnapshot,
    ) {
        let account = self
            .market_data_provider
            .get_account_balance(&self.bot.exchange, self.bot.market_type.as_str())
            .await;

        if account.total <= 0.0 && account.free <= 0.0 {
            warn!(bot_id = %self.bot.id, "Cannot retrieve account balance, skipping open");
            let _ = self.auto_event_tx.send(AutoEvent::BotError {
                bot_id: self.bot.id,
                error: format!("Cannot retrieve account balance for opening {} position", side),
            });
            return;
        }

        let atr = if snapshot.indicators.atr > 0.0 {
            snapshot.indicators.atr
        } else {
            snapshot.current_price * 0.02
        };
        let adx = snapshot.indicators.adx;
        let funding_rate = snapshot.funding_rate;
        let price = snapshot.current_price;

        let position_size_pct = strategy::compute_position_pct(adx, self.consecutive_losses, funding_rate)
            .min(self.bot.max_position_pct);

        let invest_amount = account.free * 0.95 * position_size_pct / 100.0;
        if invest_amount < 1.0 {
            warn!(bot_id = %self.bot.id, invest_amount, "Insufficient funds for opening position");
            let _ = self.auto_event_tx.send(AutoEvent::BotError {
                bot_id: self.bot.id,
                error: format!(
                    "Insufficient funds: only {:.2} USDT available (need >= 1.0)",
                    invest_amount
                ),
            });
            return;
        }

        let quantity = if self.is_spot() {
            invest_amount / price
        } else {
            invest_amount * self.bot.leverage as f64 / price
        };

        let stop_loss = strategy::compute_stop_loss(price, side, atr);
        let take_profit = strategy::compute_take_profit(price, side, atr);

        let position_side = match side {
            "long" => Some(BotPositionSide::Long),
            "short" => Some(BotPositionSide::Short),
            _ => None,
        };

        let order_side = match side {
            "long" => OrderSide::Buy,
            "short" => OrderSide::Sell,
            _ => OrderSide::Buy,
        };

        let client_order_id = format!("auto:{}:{}", side, self.bot.id);

        let result = self
            .order_executor
            .send_command(OrderCommand::PlaceOrder {
                symbol: self.bot.symbol.clone(),
                side: order_side,
                amount: quantity,
                price: None,
                reduce_only: false,
                position_side,
                client_order_id: Some(client_order_id.clone()),
            })
            .await;

        match result {
            Ok(()) => {
                info!(
                    bot_id = %self.bot.id, side = %side,
                    price, quantity,
                    stop_loss, take_profit,
                    "Position opening order sent, awaiting confirmation"
                );

                self.pending_open = Some(PendingOpen {
                    side: side.to_string(),
                    entry_price: price,
                    position_size: quantity,
                    stop_loss,
                    take_profit,
                    client_order_id,
                    sent_at: tokio::time::Instant::now(),
                });
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to send open position order");
                let _ = self.auto_event_tx.send(AutoEvent::BotError {
                    bot_id: self.bot.id,
                    error: format!("Failed to send open {} order: {}", side, e),
                });
            }
        }
    }

    pub(crate) async fn close_position(&mut self, reason: &str) {
        if !self.has_position() {
            return;
        }

        let side = self.bot.current_side.clone().unwrap_or_default();
        let (order_side, position_side) = match side.as_str() {
            "long" => (OrderSide::Sell, Some(BotPositionSide::Long)),
            "short" => (OrderSide::Buy, Some(BotPositionSide::Short)),
            _ => {
                warn!(bot_id = %self.bot.id, side = %side, "Unknown position side, cannot close");
                return;
            }
        };

        let client_order_id = format!("auto:close:{}:{}", reason, self.bot.id);

        let result = self
            .order_executor
            .send_command(OrderCommand::PlaceOrder {
                symbol: self.bot.symbol.clone(),
                side: order_side,
                amount: self.bot.position_size,
                price: None,
                reduce_only: true,
                position_side,
                client_order_id: Some(client_order_id.clone()),
            })
            .await;

        match result {
            Ok(()) => {
                info!(
                    bot_id = %self.bot.id, side = %side,
                    entry_price = self.bot.entry_price,
                    close_price = self.current_price,
                    reason = %reason,
                    "Position closing order sent, awaiting confirmation"
                );

                self.pending_close = Some(PendingClose {
                    side: side.clone(),
                    reason: reason.to_string(),
                    entry_price: self.bot.entry_price,
                    position_size: self.bot.position_size,
                    unrealized_pnl: self.bot.unrealized_pnl,
                    client_order_id,
                    sent_at: tokio::time::Instant::now(),
                });
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to send close position order");
                let _ = self.auto_event_tx.send(AutoEvent::BotError {
                    bot_id: self.bot.id,
                    error: format!("Failed to send close order ({}): {}", reason, e),
                });
            }
        }
    }

    // ── 订单事件处理 ────────────────────────────────────────

    pub(crate) async fn on_order_event(&mut self, event: OrderEvent) {
        match event {
            OrderEvent::OrderFilled { order } => {
                if !self.matches_pending_order(order.client_order_id.as_deref()) {
                    return;
                }

                let fill_price = order
                    .fill_price
                    .or(order.request_price)
                    .unwrap_or(self.current_price);
                let filled_qty = if order.filled > 0.0 { order.filled } else { 0.0 };

                if self.pending_open.is_some() {
                    info!(
                        bot_id = %self.bot.id,
                        symbol = %order.symbol,
                        side = ?order.side,
                        fill_price,
                        filled_qty,
                        "Open order filled, confirming position"
                    );
                    self.apply_pending_open(fill_price, filled_qty).await;
                } else if self.pending_close.is_some() {
                    info!(
                        bot_id = %self.bot.id,
                        symbol = %order.symbol,
                        side = ?order.side,
                        fill_price,
                        filled_qty,
                        "Close order filled, confirming close"
                    );
                    self.apply_pending_close(fill_price, filled_qty).await;
                }
            }
            OrderEvent::OrderFailed { order_id: _, reason } => {
                if self.pending_open.is_some() || self.pending_close.is_some() {
                    warn!(
                        bot_id = %self.bot.id,
                        reason = %reason,
                        "Order failed, rolling back pending state"
                    );
                    self.rollback_pending_open();
                    self.rollback_pending_close();
                }
            }
            OrderEvent::LiquidationWarning {
                symbol,
                liquidation_price,
                current_price,
            } => {
                warn!(
                    bot_id = %self.bot.id, symbol = %symbol,
                    liquidation_price, current_price,
                    "Liquidation warning in auto trade"
                );
            }
            _ => {}
        }
    }

    pub(crate) async fn apply_pending_open(&mut self, fill_price: f64, filled_qty: f64) {
        let pending = match self.pending_open.take() {
            Some(p) => p,
            None => return,
        };

        let actual_qty = if filled_qty > 0.0 {
            filled_qty
        } else {
            pending.position_size
        };

        let price_deviation = if pending.entry_price > 0.0 {
            (fill_price - pending.entry_price).abs() / pending.entry_price
        } else {
            0.0
        };

        let (stop_loss, take_profit) = if price_deviation > 0.005 {
            let snapshot = self
                .market_data_provider
                .get_market_snapshot(&self.bot.exchange, &self.bot.symbol, self.bot.market_type.as_str())
                .await;
            let atr = if snapshot.indicators.atr > 0.0 {
                snapshot.indicators.atr
            } else {
                fill_price * 0.02
            };
            let sl = strategy::compute_stop_loss(fill_price, &pending.side, atr);
            let tp = strategy::compute_take_profit(fill_price, &pending.side, atr);
            warn!(
                bot_id = %self.bot.id,
                requested_price = pending.entry_price, fill_price,
                deviation_pct = format!("{:.2}%", price_deviation * 100.0),
                old_sl = pending.stop_loss, new_sl = sl,
                old_tp = pending.take_profit, new_tp = tp,
                "Fill price deviated, recalculating stop/take profit"
            );
            (sl, tp)
        } else {
            (pending.stop_loss, pending.take_profit)
        };

        info!(
            bot_id = %self.bot.id, side = %pending.side,
            entry_price = fill_price, requested_qty = pending.position_size,
            actual_qty, stop_loss, take_profit,
            "Open order confirmed, applying position state"
        );

        self.bot.current_side = Some(pending.side.clone());
        self.bot.entry_price = fill_price;
        self.bot.position_size = actual_qty;
        self.bot.stop_loss = stop_loss;
        self.bot.take_profit = take_profit;
        self.bot.unrealized_pnl = 0.0;
        self.position_opened_at = Some(tokio::time::Instant::now());

        self.save_position().await;

        let trade_type = match pending.side.as_str() {
            "long" => "open_long",
            "short" => "open_short",
            _ => "open",
        };
        let trade_side = match pending.side.as_str() {
            "long" => "buy",
            "short" => "sell",
            _ => "buy",
        };
        let _ = self
            .store
            .record_trade(
                self.bot.id,
                self.bot.user_id,
                &self.bot.symbol,
                &self.bot.exchange,
                trade_side,
                trade_type,
                "llm",
                fill_price,
                actual_qty,
                0.0,
                0.0,
                None,
            )
            .await;

        let _ = self.auto_event_tx.send(AutoEvent::PositionOpened {
            bot_id: self.bot.id,
            side: pending.side.clone(),
            price: fill_price,
            quantity: actual_qty,
        });

        if actual_qty < pending.position_size {
            warn!(
                bot_id = %self.bot.id,
                requested = pending.position_size,
                filled = actual_qty,
                "Partial open: position opened with less than requested quantity"
            );
        }
    }

    pub(crate) async fn apply_pending_close(&mut self, fill_price: f64, filled_qty: f64) {
        let pending = match self.pending_close.take() {
            Some(p) => p,
            None => return,
        };

        let actual_qty = if filled_qty > 0.0 {
            filled_qty
        } else {
            pending.position_size
        };

        let realized_pnl = match pending.side.as_str() {
            "long" => (fill_price - pending.entry_price) * actual_qty,
            "short" => (pending.entry_price - fill_price) * actual_qty,
            _ => 0.0,
        };

        let pnl_pct = if pending.entry_price > 0.0 && actual_qty > 0.0 {
            realized_pnl / (pending.entry_price * actual_qty) * 100.0
        } else {
            0.0
        };

        info!(
            bot_id = %self.bot.id, side = %pending.side,
            entry_price = pending.entry_price, close_price = fill_price,
            quantity = actual_qty, realized_pnl, reason = %pending.reason,
            "Position closed"
        );

        self.bot.total_pnl += realized_pnl;
        self.bot.total_trades += 1;
        if realized_pnl >= 0.0 {
            self.bot.win_trades += 1;
            self.consecutive_losses = 0;
        } else {
            self.bot.loss_trades += 1;
            self.consecutive_losses += 1;
        }

        self.bot.current_side = Some("none".to_string());
        self.bot.entry_price = 0.0;
        self.bot.position_size = 0.0;
        self.bot.stop_loss = 0.0;
        self.bot.take_profit = 0.0;
        self.bot.unrealized_pnl = 0.0;
        self.bot.liquidation_price = None;
        self.position_opened_at = None;

        self.save_position().await;
        self.save_stats().await;

        let trade_side = match pending.side.as_str() {
            "long" => "sell",
            "short" => "buy",
            _ => "sell",
        };
        let trade_type = format!("close_{}", pending.side);
        let _ = self
            .store
            .record_trade(
                self.bot.id,
                self.bot.user_id,
                &self.bot.symbol,
                &self.bot.exchange,
                trade_side,
                &trade_type,
                &pending.reason,
                fill_price,
                actual_qty,
                realized_pnl,
                pnl_pct,
                None,
            )
            .await;

        let _ = self.auto_event_tx.send(AutoEvent::PositionClosed {
            bot_id: self.bot.id,
            side: pending.side.clone(),
            price: fill_price,
            pnl: realized_pnl,
        });
    }

    fn rollback_pending_open(&mut self) {
        if self.pending_open.is_some() {
            warn!(bot_id = %self.bot.id, "Rolling back pending open order");
            self.pending_open = None;
        }
    }

    fn rollback_pending_close(&mut self) {
        if self.pending_close.is_some() {
            warn!(bot_id = %self.bot.id, "Rolling back pending close order");
            self.pending_close = None;
        }
    }
}
