//! Auto worker — individual auto trading bot execution.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::auto::ai::{AutoAction, AutoAiService, AutoDecision};
use crate::auto::ports::*;
use crate::auto::strategy;
use crate::auto::types::{AutoBotConfig, AutoEvent};
use virs_types::auto_port::AutoMarketType;
use virs_types::enums::{PositionSide, PositionStatus};
use virs_types::position::{EngineEvent, Position};

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
    pe_event_rx: broadcast::Receiver<EngineEvent>,
    auto_event_tx: broadcast::Sender<AutoEvent>,
    pub(crate) current_price: f64,
    pub(crate) consecutive_losses: i32,
    pub(crate) paused: bool,
    pub(crate) pending_open: Option<PendingOpen>,
    pub(crate) pending_close: Option<PendingClose>,
    pub(crate) position_opened_at: Option<tokio::time::Instant>,
    pub(crate) trailing_stop_dirty: bool,
    /// 当前仓位缓存，从 PE 事件更新
    pub(crate) current_position: Option<Position>,
    /// 当前开仓 trade 记录 ID（开仓时 INSERT 返回，平仓时 UPDATE 用）
    pub(crate) current_trade_id: Option<Uuid>,
    /// 当前开仓手续费（平仓时计算总手续费用）
    pub(crate) current_open_fee: f64,
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
        pe_event_rx: broadcast::Receiver<EngineEvent>,
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
            pe_event_rx,
            auto_event_tx,
            current_price: 0.0,
            consecutive_losses: 0,
            paused: false,
            pending_open: None,
            pending_close: None,
            position_opened_at: None,
            trailing_stop_dirty: false,
            current_position: None,
            current_trade_id: None,
            current_open_fee: 0.0,
        }
    }

    /// 当前仓位方向（"long"/"short"/"none"）
    pub(crate) fn current_side_str(&self) -> String {
        match &self.current_position {
            Some(p) if p.status == PositionStatus::Open => match p.side {
                PositionSide::Long => "long".to_string(),
                PositionSide::Short => "short".to_string(),
                PositionSide::Both => "none".to_string(),
            },
            _ => "none".to_string(),
        }
    }

    pub(crate) fn is_spot(&self) -> bool {
        matches!(self.bot.market_type, AutoMarketType::Spot)
    }

    pub(crate) fn has_position(&self) -> bool {
        match &self.current_position {
            Some(p) if p.status == PositionStatus::Open => p.size.abs() > 1e-8,
            _ => false,
        }
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.pending_open.is_some() || self.pending_close.is_some()
    }

    /// 直接查询 PositionEngine 当前 Open 仓位，刷新 current_position 缓存。
    /// 防止 PE broadcast 事件丢失导致缓存失效 → 重复开仓。
    pub(crate) async fn refresh_position_from_pe(&mut self) {
        match self.order_executor.query_open_position(&self.bot.symbol).await {
            Ok(Some(pe_pos)) if pe_pos.status == PositionStatus::Open && pe_pos.size.abs() > 1e-8 => {
                // PE 有开仓但本地缓存为空 → 恢复
                let was_empty = !self.has_position();
                if was_empty {
                    warn!(
                        bot_id = %self.bot.id,
                        position_id = %pe_pos.id,
                        side = ?pe_pos.side,
                        size = pe_pos.size,
                        "Position cache was empty but PE has open position — recovered to prevent duplicate open"
                    );
                }
                // 恢复 position_id（如果丢失）
                if self.bot.position_id.is_none() || self.bot.position_id == Some(Uuid::nil()) {
                    self.bot.position_id = Some(pe_pos.id);
                    let _ = self
                        .store
                        .update_position(self.bot.id, self.bot.position_id, self.bot.stop_loss, self.bot.take_profit)
                        .await;
                    info!(bot_id = %self.bot.id, position_id = %pe_pos.id, "Recovered bot.position_id from PE direct query");
                }
                self.current_position = Some(pe_pos);
            }
            Ok(Some(_)) => {
                // PE 有仓位但非 Open（Opening/Closing）→ 不更新缓存，保持现状
            }
            Ok(None) => {
                // PE 确认无开仓 → 清空缓存（防止幽灵仓位）
                if self.has_position() {
                    warn!(
                        bot_id = %self.bot.id,
                        "Position cache has open position but PE confirms none — clearing stale cache"
                    );
                    self.current_position = None;
                }
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to query PE for position, relying on cached state");
            }
        }
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
        let _ = self
            .store
            .update_position(
                self.bot.id,
                self.bot.position_id,
                self.bot.stop_loss,
                self.bot.take_profit,
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
            Some(cid) => {
                // 必须包含 bot_id，且前缀匹配 pending 状态
                // open: "auto:{long|short}:{bot_id}"
                // close: "auto:close:{reason}:{bot_id}"
                if !cid.contains(&bot_id_str) {
                    return false;
                }
                let is_open_cid = cid.starts_with("auto:long:") || cid.starts_with("auto:short:");
                let is_close_cid = cid.starts_with("auto:close:");
                (self.pending_open.is_some() && is_open_cid)
                    || (self.pending_close.is_some() && is_close_cid)
            }
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

        // 如果 bot 有 position_id，等待 PE 推送仓位事件以恢复 current_position
        // 这确保重启后能立即获取仓位状态，不会错过止损止盈检查
        if self.bot.position_id.filter(|id| *id != Uuid::nil()).is_some() {
            info!(
                bot_id = %self.bot.id,
                position_id = ?self.bot.position_id,
                "Waiting for PE to restore current_position"
            );
            // 同时从 DB 恢复 current_trade_id（用于平仓时 UPDATE 对应的开仓记录）
            match self.store.find_open_trade(self.bot.id).await {
                Ok(Some(trade_id)) => {
                    self.current_trade_id = Some(trade_id);
                    info!(bot_id = %self.bot.id, trade_id = %trade_id, "Restored current_trade_id from DB");
                }
                Ok(None) => {
                    warn!(bot_id = %self.bot.id, "No open trade record found for active position");
                }
                Err(e) => {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to load open trade record");
                }
            }
            let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
            loop {
                if self.current_position.is_some() {
                    info!(bot_id = %self.bot.id, "current_position restored from PE event");
                    break;
                }
                if tokio::time::Instant::now() >= deadline {
                    warn!(
                        bot_id = %self.bot.id,
                        "Timeout waiting for PE position event, clearing stale position_id"
                    );
                    self.bot.position_id = None;
                    self.save_position().await;
                    break;
                }
                tokio::select! {
                    ev = self.pe_event_rx.recv() => {
                        match ev {
                            Ok(ev) => self.on_pe_event(ev).await,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!(bot_id = %self.bot.id, lagged = n, "PE event lagged during startup restore");
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                warn!(bot_id = %self.bot.id, "PE event channel closed during startup restore");
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!(bot_id = %self.bot.id, "Shutdown during PE position restore");
                        return;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(200)) => {}
                }
            }
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
                event = self.pe_event_rx.recv() => {
                    match event {
                        Ok(ev) => self.on_pe_event(ev).await,
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(bot_id = %self.bot.id, lagged = n, "PE event lagged");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            warn!(bot_id = %self.bot.id, "PE event channel closed");
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

        // 有 pending 订单时，跳过止损止盈检查，避免在订单未确认时重复触发
        if self.pending_open.is_some() || self.pending_close.is_some() {
            return;
        }

        if self.has_position() {
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
    }

    async fn check_stop_take_profit(&mut self) -> bool {
        let entry_price = match &self.current_position {
            Some(p) if p.status == PositionStatus::Open => p.entry_price,
            _ => return false,
        };
        if entry_price <= 0.0 {
            return false;
        }

        let side = self.current_side_str();
        let should_close = match side.as_str() {
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
        let entry_price = match &self.current_position {
            Some(p) if p.status == PositionStatus::Open => p.entry_price,
            _ => return,
        };
        if entry_price <= 0.0 || self.bot.stop_loss <= 0.0 {
            return;
        }

        if atr <= 0.0 {
            return;
        }

        let side = self.current_side_str();

        let new_stop = strategy::compute_trailing_stop(
            entry_price,
            self.current_price,
            &side,
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
        let snapshot = AutoMarketSnapshot::from_base(
            self.market_data_provider
                .get_market_snapshot(&self.bot.exchange, &self.bot.symbol, self.bot.market_type.as_str())
                .await
        );
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

        // 决策前直接查询 PE 仓位，刷新 current_position 缓存。
        // 防止 PE 事件丢失（broadcast lag）导致缓存为空 → 误判"无仓位" → 重复开仓。
        self.refresh_position_from_pe().await;

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
        let (decision, raw_llm_response, llm_model) = match decision_result {
            Some((d, raw, m)) => (Some(d), Some(raw), m),
            None => (None, None, String::new()),
        };

        let action = self
            .handle_llm_result(&decision, &system_prompt, &user_prompt, raw_llm_response.as_ref(), &llm_model)
            .await;

        self.execute_decision(&action, decision.as_ref()).await;

        if !matches!(action, AutoAction::Hold) {
            let _ = self.store.update_last_decided(self.bot.id).await;
        }
    }

    async fn build_llm_prompt(&self) -> Option<(String, String)> {
        let snapshot = AutoMarketSnapshot::from_base(
            self.market_data_provider
                .get_market_snapshot(&self.bot.exchange, &self.bot.symbol, self.bot.market_type.as_str())
                .await
        );

        if snapshot.base.current_price <= 0.0 {
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

        let (pos_side, pos_entry, pos_size, pos_liq) = match &self.current_position {
            Some(p) if p.status == PositionStatus::Open => (
                Some(self.current_side_str()),
                p.entry_price,
                p.size,
                p.liquidation_price,
            ),
            _ => (None, 0.0, 0.0, None),
        };
        let position_info = strategy::format_position_info(
            pos_side.as_deref(),
            pos_entry,
            pos_size,
            snapshot.base.current_price,
            pos_liq,
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
            funding_rate: snapshot.base.funding_rate,
            funding_next_time: snapshot.base.funding_next_time,
            total_trades: self.bot.total_trades,
            win_trades: self.bot.win_trades,
            loss_trades: self.bot.loss_trades,
            total_pnl: self.bot.total_pnl,
            consecutive_losses: self.consecutive_losses,
            trigger_reason: "scheduled".to_string(),
            ind: snapshot.indicators,
            min_qty: snapshot.base.min_qty,
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
        llm_model: &str,
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
                    .save_analysis_log(self.bot.id, "periodic", system_prompt, user_prompt, &result, None, llm_model)
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
                        llm_model,
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

        let snapshot = AutoMarketSnapshot::from_base(
            self.market_data_provider
                .get_market_snapshot(&self.bot.exchange, &self.bot.symbol, self.bot.market_type.as_str())
                .await
        );

        if snapshot.base.current_price <= 0.0 {
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

    async fn apply_non_structural_params(&mut self, d: &AutoDecision, snapshot: &AutoMarketSnapshot) {
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
        snapshot: &AutoMarketSnapshot,
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
            snapshot.base.current_price * 0.02
        };
        let adx = snapshot.indicators.adx;
        let funding_rate = snapshot.base.funding_rate;
        let price = snapshot.base.current_price;

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

        // 校验最小下单量，并按 min_qty 精度处理
        let min_qty = snapshot.base.min_qty;
        let quantity = if min_qty > 0.0 && quantity < min_qty {
            // 如果计算出的数量小于最小下单量，直接使用 min_qty 开单
            warn!(
                bot_id = %self.bot.id,
                quantity, min_qty, invest_amount, price,
                "Quantity below min_qty, using min_qty instead"
            );
            min_qty
        } else if min_qty > 0.0 {
            // 向下取整到 min_qty 的倍数，避免交易所拒绝
            (quantity / min_qty).floor() * min_qty
        } else {
            quantity
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
            .send_command(OrderCommand::OpenPosition {
                symbol: self.bot.symbol.clone(),
                side: position_side.unwrap_or(BotPositionSide::Long),
                order_side,
                amount: quantity,
                leverage: Some(self.bot.leverage.max(1) as u32),
                price: None,
                stop_loss: Some(stop_loss),
                take_profit: Some(take_profit),
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

        let side = self.current_side_str();
        let (entry_price, position_size, unrealized_pnl) = match &self.current_position {
            Some(p) => (p.entry_price, p.size, p.unrealized_pnl),
            None => (0.0, 0.0, 0.0),
        };

        // Use ClosePosition if we have a valid position_id, otherwise fall back to PlaceOrder
        // 注意：Uuid::nil() 视为无效（历史 bug 可能导致 nil UUID 被保存）
        if let Some(position_id) = self.bot.position_id.filter(|id| *id != Uuid::nil()) {
            let client_order_id = format!("auto:close:{}:{}", reason, self.bot.id);

            let result = self
                .order_executor
                .send_command(OrderCommand::ClosePosition {
                    position_id,
                    price: None,
                    client_order_id: Some(client_order_id.clone()),
                })
                .await;

            match result {
                Ok(()) => {
                    info!(
                        bot_id = %self.bot.id, side = %side,
                        entry_price = entry_price,
                        close_price = self.current_price,
                        reason = %reason,
                        "Position closing order sent via ClosePosition, awaiting confirmation"
                    );

                    self.pending_close = Some(PendingClose {
                        side: side.clone(),
                        reason: reason.to_string(),
                        entry_price,
                        position_size,
                        unrealized_pnl,
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
        } else {
            // Fallback: use PlaceOrder with reduce_only when position_id is not available
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
                    amount: position_size,
                    price: None,
                    reduce_only: true,
                    position_side,
                    position_id: None,
                    client_order_id: Some(client_order_id.clone()),
                })
                .await;

            match result {
                Ok(()) => {
                    info!(
                        bot_id = %self.bot.id, side = %side,
                        entry_price = entry_price,
                        close_price = self.current_price,
                        reason = %reason,
                        "Position closing order sent via PlaceOrder, awaiting confirmation"
                    );

                    self.pending_close = Some(PendingClose {
                        side: side.clone(),
                        reason: reason.to_string(),
                        entry_price,
                        position_size,
                        unrealized_pnl,
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
    }

    // ── 订单事件处理 ────────────────────────────────────────

    /// 处理 PositionEngine 事件，维护 current_position 缓存
    pub(crate) async fn on_pe_event(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::PositionUpdated { position } => {
                // 只关心本 bot 的 symbol 且 position_id 匹配
                if position.symbol != self.bot.symbol {
                    return;
                }
                let is_ours = match self.bot.position_id {
                    Some(pid) if pid != Uuid::nil() => pid == position.id,
                    // position_id 还没建立时，按 symbol + Open 状态匹配
                    _ => position.status == PositionStatus::Open,
                };
                if !is_ours {
                    return;
                }
                // 如果 position_id 还没建立，从 PositionUpdated 事件中恢复
                // 这可以处理 PositionOpened 事件丢失（broadcast lag）的场景
                if self.bot.position_id.is_none() || self.bot.position_id == Some(Uuid::nil()) {
                    self.bot.position_id = Some(position.id);
                    let _ = self
                        .store
                        .update_position(
                            self.bot.id,
                            self.bot.position_id,
                            self.bot.stop_loss,
                            self.bot.take_profit,
                        )
                        .await;
                    info!(
                        bot_id = %self.bot.id,
                        position_id = %position.id,
                        "Recovered bot.position_id from PositionUpdated event"
                    );
                }
                self.current_position = Some(position);
            }
            EngineEvent::PositionClosed { position } => {
                if position.symbol != self.bot.symbol {
                    return;
                }
                if let Some(pid) = self.bot.position_id {
                    if pid == position.id {
                        self.current_position = None;
                        // 清空 position_id 并持久化，避免重启后尝试恢复已关闭的仓位
                        self.bot.position_id = None;
                        let _ = self
                            .store
                            .update_position(
                                self.bot.id,
                                self.bot.position_id,
                                self.bot.stop_loss,
                                self.bot.take_profit,
                            )
                            .await;
                        info!(
                            bot_id = %self.bot.id,
                            position_id = %pid,
                            "Position closed event received, cleared bot.position_id"
                        );
                    }
                }
            }
            EngineEvent::PositionOpened { position } => {
                if position.symbol != self.bot.symbol {
                    return;
                }
                // 开仓事件到达后，记录 position_id 并缓存
                if self.bot.position_id.is_none() || self.bot.position_id == Some(Uuid::nil()) {
                    self.bot.position_id = Some(position.id);
                    let _ = self
                        .store
                        .update_position(
                            self.bot.id,
                            self.bot.position_id,
                            self.bot.stop_loss,
                            self.bot.take_profit,
                        )
                        .await;
                }
                self.current_position = Some(position);
            }
            _ => {}
        }
    }

    pub(crate) async fn on_order_event(&mut self, event: OrderEvent) {
        match event {
            OrderEvent::OrderFilled { order } => {
                if !self.matches_pending_order(order.client_order_id.as_deref()) {
                    return;
                }

                // Track position_id from order events
                if order.position_id.is_some() && self.bot.position_id.is_none() {
                    self.bot.position_id = order.position_id;
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
                        fee = order.fee,
                        "Open order filled, confirming position"
                    );
                    self.apply_pending_open(fill_price, filled_qty, order.fee).await;
                } else if self.pending_close.is_some() {
                    info!(
                        bot_id = %self.bot.id,
                        symbol = %order.symbol,
                        side = ?order.side,
                        fill_price,
                        filled_qty,
                        fee = order.fee,
                        "Close order filled, confirming close"
                    );
                    self.apply_pending_close(fill_price, filled_qty, order.fee).await;
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

    pub(crate) async fn apply_pending_open(&mut self, fill_price: f64, filled_qty: f64, fee: f64) {
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
            let snapshot = AutoMarketSnapshot::from_base(
                self.market_data_provider
                    .get_market_snapshot(&self.bot.exchange, &self.bot.symbol, self.bot.market_type.as_str())
                    .await
            );
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

        // 仓位实时状态由 PE 通过 PositionUpdated 事件维护，这里只更新 bot 的风控参数
        self.bot.stop_loss = stop_loss;
        self.bot.take_profit = take_profit;
        self.position_opened_at = Some(tokio::time::Instant::now());

        self.save_position().await;

        let trade_type = match pending.side.as_str() {
            "long" => "open_long",
            "short" => "open_short",
            _ => {
                warn!(bot_id = %self.bot.id, side = %pending.side, "Unexpected side, defaulting to open_long");
                "open_long"
            }
        };
        let trade_side = match pending.side.as_str() {
            "long" => "buy",
            "short" => "sell",
            _ => "buy",
        };
        // 开仓时 INSERT 一条 status='open' 的 trade 记录，保存 trade_id 和 open_fee
        self.current_open_fee = fee;
        match self
            .store
            .record_open_trade(
                self.bot.id,
                self.bot.user_id,
                &self.bot.symbol,
                &self.bot.exchange,
                trade_side,
                fill_price,
                actual_qty,
                fee,
                None,
            )
            .await
        {
            Ok(trade_id) => {
                self.current_trade_id = Some(trade_id);
                info!(bot_id = %self.bot.id, trade_id = %trade_id, trade_type, "Open trade recorded");
            }
            Err(e) => {
                error!(bot_id = %self.bot.id, error = %e, "Failed to record open trade");
            }
        }

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

    pub(crate) async fn apply_pending_close(&mut self, fill_price: f64, filled_qty: f64, fee: f64) {
        let pending = match self.pending_close.take() {
            Some(p) => p,
            None => return,
        };

        let actual_qty = if filled_qty > 0.0 {
            filled_qty
        } else {
            pending.position_size
        };

        // 平仓 PnL = 价格差收益 - 开仓手续费 - 平仓手续费
        let gross_pnl = match pending.side.as_str() {
            "long" => (fill_price - pending.entry_price) * actual_qty,
            "short" => (pending.entry_price - fill_price) * actual_qty,
            _ => 0.0,
        };
        let total_fee = self.current_open_fee + fee;
        let realized_pnl = gross_pnl - total_fee;

        let pnl_pct = if pending.entry_price > 0.0 && actual_qty > 0.0 {
            realized_pnl / (pending.entry_price * actual_qty) * 100.0
        } else {
            0.0
        };

        info!(
            bot_id = %self.bot.id, side = %pending.side,
            entry_price = pending.entry_price, close_price = fill_price,
            quantity = actual_qty, realized_pnl, reason = %pending.reason,
            open_fee = self.current_open_fee, close_fee = fee, total_fee,
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

        self.bot.position_id = None;
        self.bot.stop_loss = 0.0;
        self.bot.take_profit = 0.0;
        self.current_position = None;
        self.position_opened_at = None;

        self.save_position().await;
        self.save_stats().await;

        let close_side = match pending.side.as_str() {
            "long" => "sell",
            "short" => "buy",
            _ => "sell",
        };
        // trigger_source 必须满足 DB CHECK 约束 ('llm', 'risk_control')
        // 将 reason 映射为合法的 trigger_source
        let trigger_source = match pending.reason.as_str() {
            "stop_loss" | "take_profit" | "position_timeout" => "risk_control",
            _ => "llm",
        };
        let close_reason = &pending.reason;

        // 平仓时 UPDATE 对应的开仓 trade 记录
        let trade_id = self.current_trade_id.take();
        match trade_id {
            Some(tid) => {
                if let Err(e) = self
                    .store
                    .close_trade(
                        tid,
                        close_side,
                        fill_price,
                        actual_qty,
                        None,
                        fee,
                        realized_pnl,
                        pnl_pct,
                        trigger_source,
                        close_reason,
                    )
                    .await
                {
                    error!(bot_id = %self.bot.id, trade_id = %tid, error = %e, "Failed to close trade record");
                } else {
                    info!(bot_id = %self.bot.id, trade_id = %tid, realized_pnl, "Close trade recorded");
                }
            }
            None => {
                // 内存中无 trade_id，尝试从 DB 查找
                match self.store.find_open_trade(self.bot.id).await {
                    Ok(Some(tid)) => {
                        if let Err(e) = self
                            .store
                            .close_trade(
                                tid,
                                close_side,
                                fill_price,
                                actual_qty,
                                None,
                                fee,
                                realized_pnl,
                                pnl_pct,
                                trigger_source,
                                close_reason,
                            )
                            .await
                        {
                            error!(bot_id = %self.bot.id, trade_id = %tid, error = %e, "Failed to close trade record (recovered)");
                        } else {
                            info!(bot_id = %self.bot.id, trade_id = %tid, "Close trade recorded (recovered from DB)");
                        }
                    }
                    Ok(None) => {
                        warn!(bot_id = %self.bot.id, "No open trade found for close, recording as orphaned");
                        let _ = self
                            .store
                            .record_orphaned_close_trade(
                                self.bot.id,
                                self.bot.user_id,
                                &self.bot.symbol,
                                &self.bot.exchange,
                                close_side,
                                fill_price,
                                actual_qty,
                                None,
                                fee,
                                realized_pnl,
                                pnl_pct,
                                trigger_source,
                                close_reason,
                            )
                            .await;
                    }
                    Err(e) => {
                        error!(bot_id = %self.bot.id, error = %e, "Failed to find open trade for close");
                    }
                }
            }
        }

        // 重置开仓手续费缓存
        self.current_open_fee = 0.0;

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
