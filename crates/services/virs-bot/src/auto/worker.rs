use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::auto::ai::{AutoAction, AutoAiService, AutoDecision};
use crate::auto::ports::*;
use crate::auto::strategy;
use crate::auto::types::AutoBotConfig;
use virs_config::TimeConfig;
use virs_types::client_order_id;
use virs_types::enums::PositionSide;
use virs_types::position::{EngineEvent, Position};


#[derive(Debug)]
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
pub(crate) struct PendingClose {
    pub side: String,


    pub close_reason: String,
    pub entry_price: f64,
    pub position_size: f64,
    pub client_order_id: String,
    pub sent_at: tokio::time::Instant,
}


pub struct AutoWorker {
    pub(crate) bot: AutoBotConfig,
    price_provider: Arc<dyn PriceProvider>,
    order_executor: Arc<dyn OrderExecutor>,
    ai_service: Arc<AutoAiService>,
    store: Arc<dyn AutoStore>,
    market_data_provider: Arc<dyn MarketDataProvider>,
    event_rx: broadcast::Receiver<OrderEvent>,
    pe_event_rx: broadcast::Receiver<EngineEvent>,
    pub(crate) current_price: f64,
    pub(crate) consecutive_losses: i32,
    pub(crate) paused: bool,
    pub(crate) pending_open: Option<PendingOpen>,
    pub(crate) pending_close: Option<PendingClose>,
    pub(crate) position_opened_at: Option<tokio::time::Instant>,
    pub(crate) trailing_stop_dirty: bool,

    pub(crate) current_position: Option<Position>,

    pub(crate) current_open_client_order_id: Option<String>,


    pub(crate) current_log_id: Option<Uuid>,

    pub(crate) current_open_fee: f64,


    pub(crate) stop_loss: f64,
    pub(crate) take_profit: f64,


    pub(crate) last_close_event: Option<(String, String, chrono::DateTime<chrono::Utc>)>,

    pub(crate) time_config: TimeConfig,
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
        time_config: TimeConfig,
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
            current_price: 0.0,
            consecutive_losses: 0,
            paused: false,
            pending_open: None,
            pending_close: None,
            position_opened_at: None,
            trailing_stop_dirty: false,
            current_position: None,
            current_open_client_order_id: None,
            current_log_id: None,
            current_open_fee: 0.0,
            stop_loss: 0.0,
            take_profit: 0.0,
            last_close_event: None,
            time_config,
        }
    }


    pub(crate) fn current_side_str(&self) -> String {
        match &self.current_position {
            Some(p) if p.is_open() => match p.side {
                PositionSide::Long => "long".to_string(),
                PositionSide::Short => "short".to_string(),
            },
            _ => "none".to_string(),
        }
    }

    pub(crate) fn has_position(&self) -> bool {
        match &self.current_position {
            Some(p) if p.is_open() => p.size.abs() > 1e-8,
            _ => false,
        }
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.pending_open.is_some() || self.pending_close.is_some()
    }


    pub(crate) fn cooldown_remaining_secs(&self, new_side: &str) -> Option<i64> {
        let (closed_side, reason, closed_at) = self.last_close_event.as_ref()?;
        let elapsed = chrono::Utc::now().signed_duration_since(*closed_at);
        let elapsed_secs = elapsed.num_seconds().max(0);

        let cooldown_secs = strategy::compute_cooldown_secs(closed_side, reason, new_side);

        if cooldown_secs > 0 && elapsed_secs < cooldown_secs {
            Some(cooldown_secs - elapsed_secs)
        } else {
            None
        }
    }


    pub(crate) async fn refresh_position_from_pe(&mut self) -> bool {
        match self
            .order_executor
            .query_open_position(&self.bot.symbol)
            .await
        {
            Ok(Some(pe_pos))
                if pe_pos.is_open() && pe_pos.size.abs() > 1e-8 =>
            {

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

                if self.bot.position_id.is_none() || self.bot.position_id == Some(Uuid::nil()) {
                    self.bot.position_id = Some(pe_pos.id);
                    if let Err(e) = self
                        .store
                        .update_position(self.bot.id, self.bot.position_id)
                        .await
                    {
                        warn!(bot_id = %self.bot.id, error = %e, "Failed to update position");
                    }
                }
                self.current_position = Some(pe_pos);
                true
            }
            Ok(Some(_)) => {
                true
            }
            Ok(None) => {

                if self.has_position() {
                    warn!(
                        bot_id = %self.bot.id,
                        "Position cache has open position but PE confirms none — clearing stale cache"
                    );
                    self.current_position = None;
                }
                true
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to query PE for position, relying on cached state");
                false
            }
        }
    }

    pub(crate) async fn fetch_current_price(&self) -> f64 {
        match self
            .price_provider
            .get_price(&self.bot.exchange, &self.bot.symbol)
            .await
        {
            Some(price) if price > 0.0 => price,
            _ => self.current_price,
        }
    }

    pub(crate) async fn save_position(&self) {
        if let Err(e) = self
            .store
            .update_position(self.bot.id, self.bot.position_id)
            .await
        {
            warn!(bot_id = %self.bot.id, error = %e, "Failed to update position");
        }
    }

    pub(crate) async fn save_stats(&self) {
        if let Err(e) = self
            .store
            .update_stats(
                self.bot.id,
                self.bot.total_pnl,
                self.bot.total_trades,
                self.bot.win_trades,
                self.bot.loss_trades,
            )
            .await
        {
            warn!(bot_id = %self.bot.id, error = %e, "Failed to update stats");
        }
    }

    pub(crate) async fn check_pending_timeout(&mut self) {
        let now = tokio::time::Instant::now();
        let pending_timeout = Duration::from_secs(self.time_config.pending_order_timeout_secs);
        let mut timed_out_open = false;
        let mut timed_out_close = false;

        if let Some(ref pending) = self.pending_open {
            if now.duration_since(pending.sent_at) > pending_timeout {
                warn!(bot_id = %self.bot.id, "Pending open order timed out, clearing");
                self.pending_open = None;
                timed_out_open = true;
            }
        }
        if let Some(ref pending) = self.pending_close {
            if now.duration_since(pending.sent_at) > pending_timeout {
                warn!(bot_id = %self.bot.id, "Pending close order timed out, clearing");
                self.pending_close = None;
                timed_out_close = true;
            }
        }


        if timed_out_open || timed_out_close {
            if let Some(log_id) = self.current_log_id.take() {
                let exec_status = if timed_out_open {
                    "open_failed"
                } else {
                    "close_failed"
                };
                if let Err(e) = self
                    .store
                    .update_analysis_log_execution(log_id, exec_status, Some("订单超时未成交"))
                    .await
                {
                    error!(bot_id = %self.bot.id, error = %e, "Failed to update log on pending timeout");
                }
            }
        }
    }

    pub(crate) fn matches_pending_order(&self, client_order_id: Option<&str>) -> bool {
        match client_order_id {
            Some(cid) => {


                let open_match = self
                    .pending_open
                    .as_ref()
                    .map(|p| p.client_order_id == cid)
                    .unwrap_or(false);
                let close_match = self
                    .pending_close
                    .as_ref()
                    .map(|p| p.client_order_id == cid)
                    .unwrap_or(false);
                open_match || close_match
            }
            None => false,
        }
    }


    pub async fn run(&mut self, mut shutdown_rx: tokio::sync::mpsc::Receiver<()>) {

        let max_retries = self.time_config.retry.initial_price_max_retries;
        for attempt in 1..=max_retries {
            self.current_price = self.fetch_current_price().await;
            if self.current_price > 0.0 {
                break;
            }
            warn!(bot_id = %self.bot.id, attempt, "Failed to fetch initial price, retrying...");
            tokio::time::sleep(Duration::from_secs(self.time_config.price_poll_interval_secs)).await;
        }
        if self.current_price <= 0.0 {
            error!(bot_id = %self.bot.id, "Failed to fetch initial price after {} attempts, setting error status", max_retries);
            if let Err(e) = self.store.update_bot_status(self.bot.id, "error").await {
                error!(error = %e, "Failed to update bot status to error");
            }
            return;
        }


        match self.store.load_consecutive_losses(self.bot.id).await {
            Ok(losses) => {
                self.consecutive_losses = losses;
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to load consecutive losses, starting from 0");
            }
        }


        match self.store.find_last_closed_trade(self.bot.id).await {
            Ok(Some((side, close_reason, closed_at))) => {
                self.last_close_event = Some((side.clone(), close_reason.clone(), closed_at));
            }
            Ok(None) => {}
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to load last closed trade");
            }
        }


        // 先从 PE 查询当前仓位，可能恢复 position_id（PE 已从 pe_orders 聚合恢复）
        // 避免在 PE 有仓位时误判为孤儿 trade
        let pe_ok = self.refresh_position_from_pe().await;

        // 仅在 PE 查询成功且确认无仓位时才检测孤儿 trade
        // PE 报错时跳过，避免误标
        if pe_ok
            && self
                .bot
                .position_id
                .filter(|id| *id != Uuid::nil())
                .is_none()
        {
            match self.store.find_open_trade(self.bot.id).await {
                Ok(Some((client_order_id, _sl, _tp, _opened_at))) => {
                    warn!(
                        bot_id = %self.bot.id,
                        client_order_id = %client_order_id,
                        "Orphaned trade detected: open trade exists but bot.position_id is empty and PE confirms no position, marking as orphaned"
                    );
                    if let Err(e) = self.store.mark_trade_orphaned(&client_order_id).await {
                        warn!(bot_id = %self.bot.id, client_order_id = %client_order_id, error = %e, "Failed to mark trade as orphaned");
                    }
                }
                Ok(None) => {

                }
                Err(e) => {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to check orphaned trade");
                }
            }
        }


        if self
            .bot
            .position_id
            .filter(|id| *id != Uuid::nil())
            .is_some()
        {


            match self.store.find_open_trade(self.bot.id).await {
                Ok(Some((client_order_id, sl, tp, opened_at))) => {
                    self.current_open_client_order_id = Some(client_order_id);
                    self.stop_loss = sl;
                    self.take_profit = tp;


                    let elapsed = chrono::Utc::now().signed_duration_since(opened_at);
                    let elapsed_secs = elapsed.num_seconds().max(0) as u64;
                    let elapsed_dur = std::time::Duration::from_secs(elapsed_secs);
                    self.position_opened_at =
                        tokio::time::Instant::now().checked_sub(elapsed_dur);
                    if self.position_opened_at.is_none() {

                        warn!(
                            bot_id = %self.bot.id,
                            elapsed_secs,
                            "Failed to compute position_opened_at from DB opened_at, using now as fallback"
                        );
                        self.position_opened_at = Some(tokio::time::Instant::now());
                    }
                }
                Ok(None) => {
                    error!(
                        bot_id = %self.bot.id,
                        "No open trade record found for active position — \
                         stop_loss and take_profit remain 0.0, risk checks will be skipped"
                    );
                }
                Err(e) => {
                    error!(
                        bot_id = %self.bot.id,
                        error = %e,
                        "Failed to load open trade record — \
                         stop_loss and take_profit remain 0.0, risk checks will be skipped"
                    );
                }
            }

            if self.current_position.is_none() {

                let deadline = tokio::time::Instant::now()
                    + Duration::from_secs(self.time_config.close_order_timeout_secs);
                loop {
                    if self.current_position.is_some() {
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
                            return;
                        }
                        _ = tokio::time::sleep(Duration::from_millis(200)) => {}
                    }
                }
            }
        }


        let skip_llm = if self.has_position() {

            if self.position_opened_at.is_none() {
                self.position_opened_at = Some(tokio::time::Instant::now());
            }
            if self.check_stop_take_profit().await {
                self.save_position().await;
                true
            } else {
                false
            }
        } else {
            false
        };

        if !skip_llm {
            self.on_llm_decision().await;
        }

        let mut price_tick = tokio::time::interval(
            Duration::from_secs(self.time_config.price_poll_interval_secs)
        );


        let (llm_signal_tx, mut llm_signal_rx) = tokio::sync::mpsc::channel::<()>(1);
        {
            let interval_secs = self.bot.decide_interval_secs.max(60) as u64;
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


        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
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


    pub(crate) async fn on_price_tick(&mut self) {
        if self.current_price <= 0.0 {
            return;
        }

        self.check_pending_timeout().await;


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
            Some(p) if p.is_open() => p.entry_price,
            _ => return false,
        };
        if entry_price <= 0.0 {
            return false;
        }

        let side = self.current_side_str();
        let should_close = match side.as_str() {
            "long" => {
                (self.stop_loss > 0.0 && self.current_price <= self.stop_loss)
                    || (self.take_profit > 0.0 && self.current_price >= self.take_profit)
            }
            "short" => {
                (self.stop_loss > 0.0 && self.current_price >= self.stop_loss)
                    || (self.take_profit > 0.0 && self.current_price <= self.take_profit)
            }
            _ => false,
        };

        if should_close {

            let stop_triggered = self.stop_loss > 0.0
                && ((side == "long" && self.current_price <= self.stop_loss)
                    || (side == "short" && self.current_price >= self.stop_loss));
            let take_triggered = self.take_profit > 0.0
                && ((side == "long" && self.current_price >= self.take_profit)
                    || (side == "short" && self.current_price <= self.take_profit));


            let close_reason = if take_triggered {
                "take_profit"
            } else if stop_triggered {
                "stop_loss"
            } else {

                "stop_loss"
            };
            info!(
                bot_id = %self.bot.id, side = %side,
                close_reason, price = self.current_price,
                stop_loss = self.stop_loss, take_profit = self.take_profit,
                "Stop/take profit triggered"
            );
            self.close_position(close_reason).await;
            return true;
        }
        false
    }

    fn update_trailing_stop(&mut self, atr: f64) {
        let entry_price = match &self.current_position {
            Some(p) if p.is_open() => p.entry_price,
            _ => return,
        };
        if entry_price <= 0.0 || self.stop_loss <= 0.0 {
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
            self.stop_loss,
        );

        if new_stop != self.stop_loss {
            self.stop_loss = new_stop;
            self.trailing_stop_dirty = true;

            if let Some(client_order_id) = self.current_open_client_order_id.clone() {
                let store = self.store.clone();
                tokio::spawn(async move {
                    if let Err(e) = store.update_trade_stop_loss(&client_order_id, new_stop).await {
                        warn!(client_order_id = %client_order_id, error = %e, "Failed to update trade stop_loss");
                    }
                });
            }
        }
    }

    async fn fetch_current_atr(&self) -> f64 {
        let snapshot = AutoMarketSnapshot::from_base(
            self.market_data_provider
                .get_market_snapshot(&self.bot.exchange, &self.bot.symbol)
                .await,
        );
        snapshot.indicators.atr
    }

    async fn check_position_timeout(&mut self) -> bool {
        let max_duration = Duration::from_secs(self.time_config.max_position_duration_secs);
        if let Some(opened_at) = self.position_opened_at {
            if opened_at.elapsed() > max_duration {
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


    pub(crate) async fn on_llm_decision(&mut self) {
        if self.is_pending() {
            warn!(bot_id = %self.bot.id, "Pending order in progress, skipping LLM decision");
            return;
        }


        self.refresh_position_from_pe().await;

        if !self
            .ai_service
            .is_available_for_user(self.bot.user_id)
            .await
        {
            warn!(bot_id = %self.bot.id, "AI service not available, skipping decision");
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

        let (action, log_id) = self
            .handle_llm_result(
                &decision,
                &system_prompt,
                &user_prompt,
                raw_llm_response.as_ref(),
                &llm_model,
            )
            .await;


        self.current_log_id = log_id;


        let intercept_reason = self.execute_decision(&action, decision.as_ref()).await;
        if let Some(reason) = intercept_reason {
            warn!(bot_id = %self.bot.id, action = %action.as_str(), intercept_reason = %reason, "Decision intercepted");
            if let Some(log_id) = self.current_log_id {

                let exec_status = match action {
                    AutoAction::OpenLong | AutoAction::OpenShort => "open_failed",
                    AutoAction::ClosePosition => "close_failed",
                    AutoAction::Hold => "hold",
                };
                if let Err(e) = self
                    .store
                    .update_analysis_log_execution(log_id, exec_status, Some(&reason))
                    .await
                {
                    error!(bot_id = %self.bot.id, error = %e, "Failed to update intercept log");
                }
            }

            self.current_log_id = None;
        } else if matches!(action, AutoAction::Hold) {

            if let Some(log_id) = self.current_log_id {
                if let Err(e) = self
                    .store
                    .update_analysis_log_execution(log_id, "hold", None)
                    .await
                {
                    error!(bot_id = %self.bot.id, error = %e, "Failed to update hold log");
                }
                self.current_log_id = None;
            }
        }

        if !matches!(action, AutoAction::Hold) {
            if let Err(e) = self.store.update_last_decided(self.bot.id).await {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to update last decided");
            }
        }
    }

    async fn build_llm_prompt(&self) -> Option<(String, String)> {
        let snapshot = AutoMarketSnapshot::from_base(
            self.market_data_provider
                .get_market_snapshot(&self.bot.exchange, &self.bot.symbol)
                .await,
        );

        if snapshot.base.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "Market snapshot has zero price, skipping decision");
            return None;
        }

        let account = self
            .market_data_provider
            .get_account_balance(&self.bot.exchange)
            .await;
        let margin_usage_rate = if account.total > 0.0 {
            account.used / account.total
        } else {
            0.0
        };

        let position_info = match &self.current_position {
            Some(p) if p.is_open() => strategy::format_position_info(
                p,
                Some(&self.current_side_str()),
                snapshot.base.current_price,
            ),
            _ => "无仓位".to_string(),
        };

        let stop_take_profit_info =
            strategy::format_stop_take_profit(self.stop_loss, self.take_profit);

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


        let recent_close_info = match &self.last_close_event {
            Some((side, close_reason, closed_at)) => {
                let side_cn = match side.as_str() {
                    "long" => "多",
                    "short" => "空",
                    _ => "未知",
                };
                let reason_cn = match close_reason.as_str() {
                    "stop_loss" => "止损",
                    "take_profit" => "止盈",
                    "position_timeout" => "持仓超时",
                    "llm_decision" => "LLM主动平仓",
                    _ => "其他",
                };
                let elapsed = chrono::Utc::now().signed_duration_since(*closed_at);
                let elapsed_str = {
                    let mins = elapsed.num_minutes();
                    if mins < 60 {
                        format!("{} 分钟前", mins)
                    } else {
                        format!("{} 小时 {} 分钟前", mins / 60, mins % 60)
                    }
                };
                format!(
                    "{}平{}，原因：{}（{}）",
                    elapsed_str,
                    side_cn,
                    reason_cn,
                    closed_at.format("%Y-%m-%d %H:%M:%S UTC")
                )
            }
            None => "无".to_string(),
        };

        let ctx = strategy::PromptContext {
            timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            symbol: self.bot.symbol.clone(),
            exchange: self.bot.exchange.clone(),
            total_balance: account.total,
            available_balance: account.free,
            used_margin: account.used,
            margin_usage_rate,
            leverage: self.bot.leverage,
            position_info,
            position_duration,
            stop_take_profit_info,
            recent_close_info,
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
    ) -> (AutoAction, Option<Uuid>) {
        match decision {
            Some(d) => {
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
                if let Some(raw_llm_response) = raw_llm_response {
                    if let Some(obj) = result.as_object_mut() {
                        obj.insert("raw_llm_response".to_string(), raw_llm_response.clone());
                    } else {
                        tracing::error!("LLM result is not a JSON object — cannot insert raw_llm_response");
                    }
                }
                let log_id = self
                    .store
                    .save_analysis_log(
                        self.bot.id,
                        "periodic",
                        system_prompt,
                        user_prompt,
                        &result,
                        None,
                        llm_model,
                    )
                    .await
                    .ok();

                (d.action.clone(), log_id)
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
                let log_id = self
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
                    .await
                    .ok();

                (AutoAction::Hold, log_id)
            }
        }
    }


    pub(crate) async fn execute_decision(
        &mut self,
        action: &AutoAction,
        decision: Option<&AutoDecision>,
    ) -> Option<String> {
        if matches!(action, AutoAction::Hold) {
            return None;
        }

        if self.is_pending() {
            warn!(bot_id = %self.bot.id, "Pending order in progress, skipping decision execution");
            return Some("有待确认订单，跳过本次决策".to_string());
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
                    return Some(format!(
                        "置信度 {:.2} 低于 0.6 阈值，降级为观望",
                        d.confidence
                    ));
                }
            }
        }

        let snapshot = AutoMarketSnapshot::from_base(
            self.market_data_provider
                .get_market_snapshot(&self.bot.exchange, &self.bot.symbol)
                .await,
        );

        if snapshot.base.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "Market snapshot has zero price, skipping decision execution");
            return Some("市场快照价格为 0，跳过决策".to_string());
        }

        match action {
            AutoAction::OpenLong | AutoAction::OpenShort => {
                if let Some(d) = decision {
                    self.apply_non_structural_params(d).await;
                }

                let side = match action {
                    AutoAction::OpenLong => "long",
                    AutoAction::OpenShort => "short",
                    _ => unreachable!(),
                };

                if self.has_position() {
                    warn!(bot_id = %self.bot.id, side = %side, "Already has position, cannot open");
                    return Some("已有仓位，无法开仓".to_string());
                }

                if let Some(remaining) = self.cooldown_remaining_secs(side) {
                    let (closed_side, close_reason, closed_at) =
                        match self.last_close_event.as_ref() {
                            Some(ev) => ev,
                            None => {
                                error!(
                                    bot_id = %self.bot.id,
                                    side = %side,
                                    remaining_secs = remaining,
                                    "In cooldown but last_close_event is None — data inconsistency, skipping open"
                                );
                                return Some("冷却期中但关闭事件丢失，无法开仓".to_string());
                            }
                        };
                    warn!(
                        bot_id = %self.bot.id,
                        new_side = %side,
                        last_close_side = %closed_side,
                        last_close_reason = %close_reason,
                        last_close_at = %closed_at,
                        remaining_secs = remaining,
                        "In cooldown period, skipping open"
                    );
                    let reason_cn = match close_reason.as_str() {
                        "stop_loss" => "止损",
                        "take_profit" => "止盈",
                        "position_timeout" => "持仓超时",
                        "llm_decision" => "LLM平仓",
                        _ => "其他",
                    };
                    let side_cn = if side == "long" { "多" } else { "空" };
                    let closed_side_cn = if closed_side == "long" { "多" } else { "空" };
                    return Some(format!(
                        "冷却期剩 {}s，不允许开{}（上次：{}{}）",
                        remaining, side_cn, closed_side_cn, reason_cn
                    ));
                }
                self.open_position(side, decision, &snapshot).await;


                if self.pending_open.is_none() {
                    return Some("开仓订单发送失败".to_string());
                }
                None
            }
            AutoAction::ClosePosition => {
                if !self.has_position() {
                    warn!(bot_id = %self.bot.id, "No position to close");
                    return Some("无仓位可平".to_string());
                }

                self.close_position("llm_decision").await;

                if self.pending_close.is_none() {
                    return Some("平仓订单发送失败".to_string());
                }
                None
            }
            AutoAction::Hold => {
                warn!(bot_id = %self.bot.id, "Hold action reached execute_action, skipping");
                None
            }
        }
    }

    async fn apply_non_structural_params(&mut self, d: &AutoDecision) {
        if let Some(ref regime) = d.market_regime {
            self.bot.market_regime = Some(regime.clone());
        }


        let regime = match self.bot.market_regime.as_deref() {
            Some(r) => r,
            None => {
                tracing::warn!(bot_id = %self.bot.id, "market_regime is None — skipping AI analysis update to avoid default value");
                return;
            }
        };


        if let Err(e) = self
            .store
            .update_ai_analysis(
                self.bot.id,
                regime,
                self.bot.leverage,
                d.analysis.as_deref().unwrap_or(""),
            )
            .await
        {
            warn!(bot_id = %self.bot.id, error = %e, "Failed to update AI analysis");
        }
    }

    async fn open_position(
        &mut self,
        side: &str,
        decision: Option<&AutoDecision>,
        snapshot: &AutoMarketSnapshot,
    ) {
        let account = self
            .market_data_provider
            .get_account_balance(&self.bot.exchange)
            .await;

        if account.total <= 0.0 && account.free <= 0.0 {
            warn!(bot_id = %self.bot.id, "Cannot retrieve account balance, skipping open");
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

        let position_size_pct =
            strategy::compute_position_pct(adx, self.consecutive_losses, funding_rate)
                .min(self.bot.max_position_pct);

        let invest_amount = account.free * 0.95 * position_size_pct / 100.0;
        if invest_amount < 1.0 {
            warn!(bot_id = %self.bot.id, invest_amount, "Insufficient funds for opening position");
            return;
        }

        let quantity = invest_amount * self.bot.leverage as f64 / price;


        let min_qty = snapshot.base.min_qty;
        let quantity = if min_qty > 0.0 && quantity < min_qty {

            warn!(
                bot_id = %self.bot.id,
                quantity, min_qty, invest_amount, price,
                "Quantity below min_qty, using min_qty instead"
            );
            min_qty
        } else if min_qty > 0.0 {

            (quantity / min_qty).floor() * min_qty
        } else {
            quantity
        };


        let formula_sl = strategy::compute_stop_loss(price, side, atr);
        let formula_tp = strategy::compute_take_profit(price, side, atr);

        let llm_sl = decision.and_then(|d| d.stop_loss);
        let llm_tp = decision.and_then(|d| d.take_profit);

        let (stop_loss, stop_loss_source) = match (llm_sl, side) {
            (Some(sl), "long") if sl > 0.0 && sl < price => (sl, "llm"),
            (Some(sl), "short") if sl > 0.0 && sl > price => (sl, "llm"),
            (Some(sl), _) => {
                warn!(
                    bot_id = %self.bot.id, side, llm_sl = sl, price,
                    "LLM stop_loss invalid (direction mismatch or non-positive), fallback to formula"
                );
                (formula_sl, "formula")
            }
            (None, _) => (formula_sl, "formula"),
        };

        let (take_profit, take_profit_source) = match (llm_tp, side) {
            (Some(tp), "long") if tp > 0.0 && tp > price => (tp, "llm"),
            (Some(tp), "short") if tp > 0.0 && tp < price => (tp, "llm"),
            (Some(tp), _) => {
                warn!(
                    bot_id = %self.bot.id, side, llm_tp = tp, price,
                    "LLM take_profit invalid (direction mismatch or non-positive), fallback to formula"
                );
                (formula_tp, "formula")
            }
            (None, _) => (formula_tp, "formula"),
        };


        let rr_ratio = match side {
            "long" => (take_profit - price) / (price - stop_loss).max(1e-9),
            "short" => (price - take_profit) / (stop_loss - price).max(1e-9),
            _ => 1.5,
        };
        let (stop_loss, take_profit, sl_source, tp_source) = if rr_ratio < 1.0 {
            warn!(
                bot_id = %self.bot.id, side,
                entry = price, llm_sl = stop_loss, llm_tp = take_profit, rr_ratio,
                "Risk-reward ratio < 1.0, fallback to formula"
            );
            (formula_sl, formula_tp, "formula", "formula")
        } else {
            (stop_loss, take_profit, stop_loss_source, take_profit_source)
        };

        let position_side = match side {
            "long" => BotPositionSide::Long,
            "short" => BotPositionSide::Short,
            _ => {
                error!(side = %side, "Unknown position side — refusing to place order");
                return;
            }
        };

        let order_side = match side {
            "long" => OrderSide::Buy,
            "short" => OrderSide::Sell,
            _ => {
                error!(side = %side, "Unknown position side — refusing to place order");
                return;
            }
        };

        let client_order_id = client_order_id::format_auto_open(self.bot.id, side);

        let result = self
            .order_executor
            .send_command(OrderCommand::OpenPosition {
                symbol: self.bot.symbol.clone(),
                side: position_side,
                order_side,
                amount: quantity,
                leverage: self.bot.leverage.max(1) as u32,
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
                    sl_source, tp_source,
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
            }
        }
    }


    pub(crate) async fn close_position(&mut self, close_reason: &str) {
        if !self.has_position() {
            return;
        }

        let side = self.current_side_str();
        let (entry_price, position_size) = match &self.current_position {
            Some(p) => (p.entry_price, p.size),
            None => (0.0, 0.0),
        };


        if let Some(position_id) = self.bot.position_id.filter(|id| *id != Uuid::nil()) {
            let client_order_id = client_order_id::format_auto_close(self.bot.id, &side);

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
                        close_reason = %close_reason,
                        "Position closing order sent via ClosePosition, awaiting confirmation"
                    );

                    self.pending_close = Some(PendingClose {
                        side: side.clone(),
                        close_reason: close_reason.to_string(),
                        entry_price,
                        position_size,
                        client_order_id,
                        sent_at: tokio::time::Instant::now(),
                    });
                }
                Err(e) => {
                    error!(bot_id = %self.bot.id, error = %e, "Failed to send close position order");
                }
            }
        } else {

            let (order_side, position_side) = match side.as_str() {
                "long" => (OrderSide::Sell, Some(BotPositionSide::Long)),
                "short" => (OrderSide::Buy, Some(BotPositionSide::Short)),
                _ => {
                    error!(bot_id = %self.bot.id, side = %side, "Unknown position side, cannot close");
                    return;
                }
            };

            let client_order_id = client_order_id::format_auto_close(self.bot.id, &side);

            let result = self
                .order_executor
                .send_command(OrderCommand::PlaceOrder {
                    symbol: self.bot.symbol.clone(),
                    side: order_side,
                    amount: position_size,
                    price: None,
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
                        close_reason = %close_reason,
                        "Position closing order sent via PlaceOrder, awaiting confirmation"
                    );

                    self.pending_close = Some(PendingClose {
                        side: side.clone(),
                        close_reason: close_reason.to_string(),
                        entry_price,
                        position_size,
                        client_order_id,
                        sent_at: tokio::time::Instant::now(),
                    });
                }
                Err(e) => {
                    error!(bot_id = %self.bot.id, error = %e, "Failed to send close position order (fallback path)");
                }
            }
        }
    }


    pub(crate) async fn on_pe_event(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::PositionUpdated { position } => {

                if position.symbol != self.bot.symbol {
                    return;
                }
                let is_ours = match self.bot.position_id {
                    Some(pid) if pid != Uuid::nil() => pid == position.id,

                    _ => position.is_open(),
                };
                if !is_ours {
                    return;
                }


                if self.bot.position_id.is_none() || self.bot.position_id == Some(Uuid::nil()) {
                    self.bot.position_id = Some(position.id);
                    if let Err(e) = self
                        .store
                        .update_position(self.bot.id, self.bot.position_id)
                        .await
                    {
                        warn!(bot_id = %self.bot.id, error = %e, "Failed to update position");
                    }
                }
                self.current_position = Some(position);
            }
            EngineEvent::PositionClosed { position } => {
                if position.symbol != self.bot.symbol {
                    return;
                }
                let is_ours = match self.bot.position_id.filter(|id| *id != Uuid::nil()) {
                    Some(pid) => pid == position.id,
                    None => self.current_position.is_some(),
                };
                if !is_ours {
                    return;
                }
                self.current_position = None;

                self.bot.position_id = None;
                if let Err(e) = self
                    .store
                    .update_position(self.bot.id, self.bot.position_id)
                    .await
                {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to update position");
                }
            }
            EngineEvent::PositionOpened { position } => {
                if position.symbol != self.bot.symbol {
                    return;
                }

                if self.bot.position_id.is_none() || self.bot.position_id == Some(Uuid::nil()) {
                    self.bot.position_id = Some(position.id);
                    if let Err(e) = self
                        .store
                        .update_position(self.bot.id, self.bot.position_id)
                        .await
                    {
                        warn!(bot_id = %self.bot.id, error = %e, "Failed to update position");
                    }
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


                if order.position_id.filter(|id| *id != Uuid::nil()).is_some()
                    && self.bot.position_id.filter(|id| *id != Uuid::nil()).is_none()
                {
                    self.bot.position_id = order.position_id;
                }

                let fill_price = order
                    .fill_price
                    .or(order.request_price)
                    .unwrap_or_else(|| {


                        warn!(
                            order_id = %order.id,
                            "Order has no fill_price and no request_price — \
                             using current_price as fallback (PnL may be inaccurate)"
                        );
                        self.current_price
                    });
                let filled_qty = if order.filled > 0.0 {
                    order.filled
                } else {
                    0.0
                };

                if self.pending_open.is_some() {
                    self.apply_pending_open(fill_price, filled_qty, order.fee)
                        .await;
                } else if self.pending_close.is_some() {
                    self.apply_pending_close(fill_price, filled_qty, order.fee)
                        .await;
                }
            }
            OrderEvent::OrderFailed {
                order_id: _,
                reason,
            } if self.pending_open.is_some() || self.pending_close.is_some() => {

                    let was_open = self.pending_open.is_some();
                    warn!(
                        bot_id = %self.bot.id,
                        reason = %reason,
                        was_open,
                        "Order failed, rolling back pending state"
                    );
                    self.rollback_pending_open();
                    self.rollback_pending_close();


                    if let Some(log_id) = self.current_log_id.take() {
                        let exec_status = if was_open {
                            "open_failed"
                        } else {
                            "close_failed"
                        };
                        if let Err(e) = self
                            .store
                            .update_analysis_log_execution(log_id, exec_status, Some(&reason))
                            .await
                        {
                            error!(bot_id = %self.bot.id, error = %e, "Failed to update log on order failed");
                        }
                    }
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
                    .get_market_snapshot(&self.bot.exchange, &self.bot.symbol)
                    .await,
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


        self.stop_loss = stop_loss;
        self.take_profit = take_profit;
        self.position_opened_at = Some(tokio::time::Instant::now());

        self.save_position().await;

        let trade_type = match pending.side.as_str() {
            "long" => "open_long",
            "short" => "open_short",
            _ => {
                error!(side = %pending.side, "Unknown pending side — skipping trade record");
                return;
            }
        };

        self.current_open_fee = fee;
        let client_order_id = pending.client_order_id.clone();
        match self
            .store
            .record_open_trade(
                self.bot.id,
                self.bot.user_id,
                &self.bot.symbol,
                &self.bot.exchange,
                &client_order_id,
                stop_loss,
                take_profit,
            )
            .await
        {
            Ok(()) => {
                info!(bot_id = %self.bot.id, client_order_id = %client_order_id, trade_type, stop_loss, take_profit, "Open trade recorded");
                self.current_open_client_order_id = Some(client_order_id);
            }
            Err(e) => {
                error!(bot_id = %self.bot.id, error = %e, "Failed to record open trade");
            }
        }

        if actual_qty < pending.position_size {
            warn!(
                bot_id = %self.bot.id,
                requested = pending.position_size,
                filled = actual_qty,
                "Partial open: position opened with less than requested quantity"
            );
        }


        if let Some(log_id) = self.current_log_id.take() {
            if let Err(e) = self
                .store
                .update_analysis_log_execution(log_id, "open", None)
                .await
            {
                error!(bot_id = %self.bot.id, error = %e, "Failed to update open execution status");
            }
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


        let gross_pnl = match pending.side.as_str() {
            "long" => (fill_price - pending.entry_price) * actual_qty,
            "short" => (pending.entry_price - fill_price) * actual_qty,
            _ => {
                error!(side = %pending.side, "Unknown pending side — skipping trade record");
                return;
            }
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
            quantity = actual_qty, realized_pnl, pnl_pct,
            close_reason = %pending.close_reason,
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
        self.stop_loss = 0.0;
        self.take_profit = 0.0;
        self.current_position = None;
        self.position_opened_at = None;


        self.last_close_event = Some((
            pending.side.clone(),
            pending.close_reason.clone(),
            chrono::Utc::now(),
        ));

        self.save_position().await;
        self.save_stats().await;

        let close_reason = &pending.close_reason;
        let close_client_order_id = pending.client_order_id.clone();

        let open_client_order_id = self.current_open_client_order_id.take();
        match open_client_order_id {
            Some(open_oid) => {
                if let Err(e) = self
                    .store
                    .close_trade(&open_oid, &close_client_order_id, close_reason)
                    .await
                {
                    error!(bot_id = %self.bot.id, open_client_order_id = %open_oid, error = %e, "Failed to close trade record");
                } else {
                    info!(bot_id = %self.bot.id, open_client_order_id = %open_oid, realized_pnl, "Close trade recorded");
                }
            }
            None => {

                match self.store.find_open_trade(self.bot.id).await {
                    Ok(Some((open_oid, _sl, _tp, _opened_at))) => {
                        if let Err(e) = self
                            .store
                            .close_trade(&open_oid, &close_client_order_id, close_reason)
                            .await
                        {
                            error!(bot_id = %self.bot.id, open_client_order_id = %open_oid, error = %e, "Failed to close trade record (recovered)");
                        } else {
                            info!(bot_id = %self.bot.id, open_client_order_id = %open_oid, "Close trade recorded (recovered from DB)");
                        }
                    }
                    Ok(None) => {
                        warn!(bot_id = %self.bot.id, "No open trade found for close, recording as orphaned");
                        if let Err(e) = self
                            .store
                            .record_orphaned_close_trade(
                                self.bot.id,
                                self.bot.user_id,
                                &self.bot.symbol,
                                &self.bot.exchange,
                                &close_client_order_id,
                                close_reason,
                            )
                            .await
                        {
                            warn!(bot_id = %self.bot.id, error = %e, "Failed to record orphaned close trade");
                        }
                    }
                    Err(e) => {
                        error!(bot_id = %self.bot.id, error = %e, "Failed to find open trade for close");
                    }
                }
            }
        }


        self.current_open_fee = 0.0;


        if let Some(log_id) = self.current_log_id.take() {
            if let Err(e) = self
                .store
                .update_analysis_log_execution(log_id, "close", None)
                .await
            {
                error!(bot_id = %self.bot.id, error = %e, "Failed to update close execution status");
            }
        }
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
