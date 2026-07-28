use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::auto::ai::{AutoAction, AutoAiService, AutoDecision};
use crate::auto::ports::AutoMarketSnapshot;
use crate::auto::strategy;
use virs_types::auto_port::{AutoBotConfig, AutoStore};
use virs_types::bot::{MarketDataProvider, OrderCommand, OrderEvent, OrderExecutor, PriceProvider};
use virs_strategy::prompt::{render, PromptLoader, RenderContext, StrategyType};
use virs_config::TimeConfig;
use virs_types::client_order_id;
use virs_types::enums::{PositionSide, Side};
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

    // ===== 全局状态（不 per-side） =====
    pub(crate) current_price: f64,
    pub(crate) consecutive_losses: i32,
    pub(crate) paused: bool,
    pub(crate) trailing_stop_dirty: bool,
    pub(crate) time_config: TimeConfig,
    pub(crate) prompt_loader: PromptLoader,

    // ===== Per-side 持仓缓存 =====
    pub(crate) current_long: Option<Position>,
    pub(crate) current_short: Option<Position>,

    // ===== Per-side pending =====
    pub(crate) pending_open_long: Option<PendingOpen>,
    pub(crate) pending_open_short: Option<PendingOpen>,
    pub(crate) pending_close_long: Option<PendingClose>,
    pub(crate) pending_close_short: Option<PendingClose>,

    // ===== Per-side 止损止盈 =====
    pub(crate) stop_loss_long: f64,
    pub(crate) take_profit_long: f64,
    pub(crate) stop_loss_short: f64,
    pub(crate) take_profit_short: f64,

    // ===== Per-side 持仓开始时间 =====
    pub(crate) position_opened_at_long: Option<tokio::time::Instant>,
    pub(crate) position_opened_at_short: Option<tokio::time::Instant>,

    // ===== Per-side 开仓 client_order_id =====
    pub(crate) current_open_client_order_id_long: Option<String>,
    pub(crate) current_open_client_order_id_short: Option<String>,

    // ===== Per-side 分析日志 ID =====
    pub(crate) current_log_id_long: Option<Uuid>,
    pub(crate) current_log_id_short: Option<Uuid>,

    // ===== Per-side 开仓手续费 =====
    pub(crate) current_open_fee_long: f64,
    pub(crate) current_open_fee_short: f64,

    // ===== Per-side 上次平仓事件 =====
    pub(crate) last_close_event_long: Option<(String, String, chrono::DateTime<chrono::Utc>)>,
    pub(crate) last_close_event_short: Option<(String, String, chrono::DateTime<chrono::Utc>)>,
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
        prompt_loader: PromptLoader,
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
            trailing_stop_dirty: false,
            time_config,
            prompt_loader,
            current_long: None,
            current_short: None,
            pending_open_long: None,
            pending_open_short: None,
            pending_close_long: None,
            pending_close_short: None,
            stop_loss_long: 0.0,
            take_profit_long: 0.0,
            stop_loss_short: 0.0,
            take_profit_short: 0.0,
            position_opened_at_long: None,
            position_opened_at_short: None,
            current_open_client_order_id_long: None,
            current_open_client_order_id_short: None,
            current_log_id_long: None,
            current_log_id_short: None,
            current_open_fee_long: 0.0,
            current_open_fee_short: 0.0,
            last_close_event_long: None,
            last_close_event_short: None,
        }
    }

    // ===== Per-side 辅助方法 =====

    pub(crate) fn get_position(&self, side: &PositionSide) -> Option<&Position> {
        match side {
            PositionSide::Long => self.current_long.as_ref(),
            PositionSide::Short => self.current_short.as_ref(),
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        }
    }

    pub(crate) fn has_position_side(&self, side: PositionSide) -> bool {
        self.get_position(&side)
            .map(|p| p.is_open() && p.quantity.abs() > 1e-8)
            .unwrap_or(false)
    }

    pub(crate) fn has_any_position(&self) -> bool {
        self.has_position_side(PositionSide::Long) || self.has_position_side(PositionSide::Short)
    }

    pub(crate) fn is_pending_side(&self, side: PositionSide) -> bool {
        match side {
            PositionSide::Long => self.pending_open_long.is_some() || self.pending_close_long.is_some(),
            PositionSide::Short => self.pending_open_short.is_some() || self.pending_close_short.is_some(),
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        }
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.is_pending_side(PositionSide::Long) || self.is_pending_side(PositionSide::Short)
    }

    /// 返回当前持仓方向字符串："long" / "short" / "long+short" / "none"
    #[allow(dead_code)]
    pub(crate) fn current_sides_str(&self) -> String {
        let has_long = self.has_position_side(PositionSide::Long);
        let has_short = self.has_position_side(PositionSide::Short);
        match (has_long, has_short) {
            (true, true) => "long+short".to_string(),
            (true, false) => "long".to_string(),
            (false, true) => "short".to_string(),
            (false, false) => "none".to_string(),
        }
    }

    pub(crate) fn cooldown_remaining_secs(&self, new_side: &str) -> Option<i64> {
        let last_event = match new_side {
            "long" => self.last_close_event_long.as_ref(),
            "short" => self.last_close_event_short.as_ref(),
            _ => return None,
        };
        let (closed_side, reason, closed_at) = last_event?;
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
            .query_open_positions(&self.bot.symbol)
            .await
        {
            Ok(positions) => {
                let mut found_long: Option<Position> = None;
                let mut found_short: Option<Position> = None;
                for pe_pos in positions.into_iter() {
                    if !pe_pos.is_open() || pe_pos.quantity.abs() <= 1e-8 {
                        continue;
                    }
                    match pe_pos.side {
                        PositionSide::Long if found_long.is_none() => found_long = Some(pe_pos),
                        PositionSide::Short if found_short.is_none() => found_short = Some(pe_pos),
                        _ => {}
                    }
                }

                // 恢复日志：缓存为空但 PE 有仓位
                if !self.has_position_side(PositionSide::Long) && found_long.is_some() {
                    let p = found_long.as_ref().unwrap();
                    warn!(
                        bot_id = %self.bot.id,
                        position_id = %p.id,
                        side = "long",
                        quantity = p.quantity,
                        "Long position cache was empty but PE has open position — recovered to prevent duplicate open"
                    );
                }
                if !self.has_position_side(PositionSide::Short) && found_short.is_some() {
                    let p = found_short.as_ref().unwrap();
                    warn!(
                        bot_id = %self.bot.id,
                        position_id = %p.id,
                        side = "short",
                        quantity = p.quantity,
                        "Short position cache was empty but PE has open position — recovered to prevent duplicate open"
                    );
                }

                // per-side 回填 position_id（PE 已从 pe_trades 聚合恢复）
                let mut position_updated = false;
                if self.bot.position_id_long.is_none() || self.bot.position_id_long == Some(Uuid::nil()) {
                    if let Some(ref p) = found_long {
                        self.bot.position_id_long = Some(p.id);
                        position_updated = true;
                    }
                }
                if self.bot.position_id_short.is_none() || self.bot.position_id_short == Some(Uuid::nil()) {
                    if let Some(ref p) = found_short {
                        self.bot.position_id_short = Some(p.id);
                        position_updated = true;
                    }
                }
                if position_updated {
                    if let Err(e) = self
                        .store
                        .update_position(
                            self.bot.id,
                            self.bot.position_id_long,
                            self.bot.position_id_short,
                        )
                        .await
                    {
                        warn!(bot_id = %self.bot.id, error = %e, "Failed to update position");
                    }
                }

                self.current_long = found_long;
                self.current_short = found_short;
                true
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to query PE for positions, relying on cached state");
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
            .update_position(
                self.bot.id,
                self.bot.position_id_long,
                self.bot.position_id_short,
            )
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
        let mut timed_out_open_long = false;
        let mut timed_out_open_short = false;
        let mut timed_out_close_long = false;
        let mut timed_out_close_short = false;

        if let Some(ref pending) = self.pending_open_long {
            if now.duration_since(pending.sent_at) > pending_timeout {
                warn!(bot_id = %self.bot.id, side = "long", "Pending open order timed out, clearing");
                self.pending_open_long = None;
                timed_out_open_long = true;
            }
        }
        if let Some(ref pending) = self.pending_open_short {
            if now.duration_since(pending.sent_at) > pending_timeout {
                warn!(bot_id = %self.bot.id, side = "short", "Pending open order timed out, clearing");
                self.pending_open_short = None;
                timed_out_open_short = true;
            }
        }
        if let Some(ref pending) = self.pending_close_long {
            if now.duration_since(pending.sent_at) > pending_timeout {
                warn!(bot_id = %self.bot.id, side = "long", "Pending close order timed out, clearing");
                self.pending_close_long = None;
                timed_out_close_long = true;
            }
        }
        if let Some(ref pending) = self.pending_close_short {
            if now.duration_since(pending.sent_at) > pending_timeout {
                warn!(bot_id = %self.bot.id, side = "short", "Pending close order timed out, clearing");
                self.pending_close_short = None;
                timed_out_close_short = true;
            }
        }

        let timed_out_open = timed_out_open_long || timed_out_open_short;
        let timed_out_close = timed_out_close_long || timed_out_close_short;

        if timed_out_open || timed_out_close {
            let exec_status = if timed_out_open {
                "open_failed"
            } else {
                "close_failed"
            };
            // 取出对应 side 的 log_id 进行更新
            let log_ids: Vec<Uuid> = [
                (timed_out_open_long || timed_out_close_long)
                    .then(|| self.current_log_id_long.take())
                    .flatten(),
                (timed_out_open_short || timed_out_close_short)
                    .then(|| self.current_log_id_short.take())
                    .flatten(),
            ]
            .into_iter()
            .flatten()
            .collect();
            for log_id in log_ids {
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
                let open_long = self
                    .pending_open_long
                    .as_ref()
                    .map(|p| p.client_order_id == cid)
                    .unwrap_or(false);
                let open_short = self
                    .pending_open_short
                    .as_ref()
                    .map(|p| p.client_order_id == cid)
                    .unwrap_or(false);
                let close_long = self
                    .pending_close_long
                    .as_ref()
                    .map(|p| p.client_order_id == cid)
                    .unwrap_or(false);
                let close_short = self
                    .pending_close_short
                    .as_ref()
                    .map(|p| p.client_order_id == cid)
                    .unwrap_or(false);
                open_long || open_short || close_long || close_short
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
            tokio::time::sleep(Duration::from_secs(
                self.time_config.price_poll_interval_secs,
            ))
            .await;
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
                let event = (side.clone(), close_reason.clone(), closed_at);
                match side.as_str() {
                    "long" => self.last_close_event_long = Some(event),
                    "short" => self.last_close_event_short = Some(event),
                    _ => {
                        // 未知 side，回退到两边都设置
                        self.last_close_event_long = Some(event.clone());
                        self.last_close_event_short = Some(event);
                    }
                }
            }
            Ok(None) => {}
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to load last closed trade");
            }
        }

        // 先从 PE 查询当前仓位，可能恢复 position_id（PE 已从 pe_trades 聚合恢复）
        // 避免在 PE 有仓位时误判为孤儿 trade
        let pe_ok = self.refresh_position_from_pe().await;

        // 仅在 PE 查询成功且确认无仓位时才检测孤儿 trade
        // PE 报错时跳过，避免误标
        if pe_ok
            && self
                .bot
                .position_id_long
                .filter(|id| *id != Uuid::nil())
                .is_none()
            && self
                .bot
                .position_id_short
                .filter(|id| *id != Uuid::nil())
                .is_none()
        {
            match self.store.find_open_trade(self.bot.id).await {
                Ok(Some((client_order_id, _sl, _tp, _opened_at))) => {
                    warn!(
                        bot_id = %self.bot.id,
                        client_order_id = %client_order_id,
                        "Orphaned trade detected: open trade exists but bot.position_id_long/short are empty and PE confirms no position, marking as orphaned"
                    );
                    if let Err(e) = self.store.mark_trade_orphaned(&client_order_id).await {
                        warn!(bot_id = %self.bot.id, client_order_id = %client_order_id, error = %e, "Failed to mark trade as orphaned");
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to check orphaned trade");
                }
            }
        }

        if self
            .bot
            .position_id_long
            .filter(|id| *id != Uuid::nil())
            .is_some()
            || self
                .bot
                .position_id_short
                .filter(|id| *id != Uuid::nil())
                .is_some()
        {
            // DB 单行 find_open_trade：根据 client_order_id 前缀判断 side，
            // 仅能恢复一个方向的 SL/TP（DB 多 side 恢复留待阶段 3）
            match self.store.find_open_trade(self.bot.id).await {
                Ok(Some((client_order_id, sl, tp, opened_at))) => {
                    let side = parse_side_from_client_order_id(&client_order_id);
                    let elapsed = chrono::Utc::now().signed_duration_since(opened_at);
                    let elapsed_secs = elapsed.num_seconds().max(0) as u64;
                    let elapsed_dur = std::time::Duration::from_secs(elapsed_secs);
                    let position_opened_at =
                        tokio::time::Instant::now().checked_sub(elapsed_dur);

                    let position_opened_at = match position_opened_at {
                        Some(at) => at,
                        None => {
                            warn!(
                                bot_id = %self.bot.id,
                                elapsed_secs,
                                "Failed to compute position_opened_at from DB opened_at, using now as fallback"
                            );
                            tokio::time::Instant::now()
                        }
                    };

                    match side {
                        Some(PositionSide::Long) => {
                            self.current_open_client_order_id_long = Some(client_order_id);
                            self.stop_loss_long = sl;
                            self.take_profit_long = tp;
                            self.position_opened_at_long = Some(position_opened_at);
                        }
                        Some(PositionSide::Short) => {
                            self.current_open_client_order_id_short = Some(client_order_id);
                            self.stop_loss_short = sl;
                            self.take_profit_short = tp;
                            self.position_opened_at_short = Some(position_opened_at);
                        }
                        None => {
                            warn!(
                                bot_id = %self.bot.id,
                                client_order_id = %client_order_id,
                                "Cannot parse side from client_order_id — stop_loss/take_profit not restored"
                            );
                        }
                        Some(PositionSide::Unknown(_)) => unreachable!("validate ensures position_side is Long/Short"),
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

            if !self.has_any_position() {
                let deadline = tokio::time::Instant::now()
                    + Duration::from_secs(self.time_config.close_order_timeout_secs);
                loop {
                    if self.has_any_position() {
                        break;
                    }
                    if tokio::time::Instant::now() >= deadline {
                        warn!(
                            bot_id = %self.bot.id,
                            "Timeout waiting for PE position event, clearing stale position_id"
                        );
                        self.bot.position_id_long = None;
                        self.bot.position_id_short = None;
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

        let skip_llm = if self.has_any_position() {
            // 为每个有持仓的方向确保 opened_at 已设置
            if self.has_position_side(PositionSide::Long)
                && self.position_opened_at_long.is_none()
            {
                self.position_opened_at_long = Some(tokio::time::Instant::now());
            }
            if self.has_position_side(PositionSide::Short)
                && self.position_opened_at_short.is_none()
            {
                self.position_opened_at_short = Some(tokio::time::Instant::now());
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

        let mut price_tick = tokio::time::interval(Duration::from_secs(
            self.time_config.price_poll_interval_secs,
        ));

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

        if self.is_pending() {
            return;
        }

        if self.has_any_position() {
            if let Some(atr) = self.fetch_current_atr().await {
                self.update_trailing_stop(atr);
            }

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

    /// 对 Long 和 Short 分别检查止损止盈；任意方向触发平仓即返回 true
    async fn check_stop_take_profit(&mut self) -> bool {
        let mut triggered = false;
        if self.has_position_side(PositionSide::Long) {
            if self.check_stop_take_profit_side(PositionSide::Long).await {
                triggered = true;
            }
        }
        if self.has_position_side(PositionSide::Short) {
            if self.check_stop_take_profit_side(PositionSide::Short).await {
                triggered = true;
            }
        }
        triggered
    }

    async fn check_stop_take_profit_side(&mut self, side: PositionSide) -> bool {
        let entry_price = match self.get_position(&side) {
            Some(p) if p.is_open() => p.entry_price,
            _ => return false,
        };
        if entry_price <= 0.0 {
            return false;
        }

        let (stop_loss, take_profit) = match side {
            PositionSide::Long => (self.stop_loss_long, self.take_profit_long),
            PositionSide::Short => (self.stop_loss_short, self.take_profit_short),
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        };
        let side_str = side_str(&side);

        let should_close = match side {
            PositionSide::Long => {
                (stop_loss > 0.0 && self.current_price <= stop_loss)
                    || (take_profit > 0.0 && self.current_price >= take_profit)
            }
            PositionSide::Short => {
                (stop_loss > 0.0 && self.current_price >= stop_loss)
                    || (take_profit > 0.0 && self.current_price <= take_profit)
            }
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        };

        if should_close {
            let stop_triggered = stop_loss > 0.0
                && ((side == PositionSide::Long && self.current_price <= stop_loss)
                    || (side == PositionSide::Short && self.current_price >= stop_loss));
            let take_triggered = take_profit > 0.0
                && ((side == PositionSide::Long && self.current_price >= take_profit)
                    || (side == PositionSide::Short && self.current_price <= take_profit));

            let close_reason = if take_triggered {
                "take_profit"
            } else if stop_triggered {
                "stop_loss"
            } else {
                "stop_loss"
            };
            info!(
                bot_id = %self.bot.id, side = %side_str,
                close_reason, price = self.current_price,
                stop_loss, take_profit,
                "Stop/take profit triggered"
            );
            self.close_position(side, close_reason).await;
            return true;
        }
        false
    }

    fn update_trailing_stop(&mut self, atr: f64) {
        if atr <= 0.0 {
            return;
        }
        self.update_trailing_stop_side(PositionSide::Long, atr);
        self.update_trailing_stop_side(PositionSide::Short, atr);
    }

    fn update_trailing_stop_side(&mut self, side: PositionSide, atr: f64) {
        let entry_price = match self.get_position(&side) {
            Some(p) if p.is_open() => p.entry_price,
            _ => return,
        };

        let (stop_loss, client_order_id) = match side {
            PositionSide::Long => (
                self.stop_loss_long,
                self.current_open_client_order_id_long.clone(),
            ),
            PositionSide::Short => (
                self.stop_loss_short,
                self.current_open_client_order_id_short.clone(),
            ),
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        };

        if entry_price <= 0.0 || stop_loss <= 0.0 {
            return;
        }

        let side_str = side_str(&side);

        let new_stop = strategy::compute_trailing_stop(
            entry_price,
            self.current_price,
            side_str,
            atr,
            stop_loss,
        );

        if new_stop != stop_loss {
            match side {
                PositionSide::Long => self.stop_loss_long = new_stop,
                PositionSide::Short => self.stop_loss_short = new_stop,
                PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
            }
            self.trailing_stop_dirty = true;

            if let Some(client_order_id) = client_order_id {
                let store = self.store.clone();
                tokio::spawn(async move {
                    if let Err(e) = store
                        .update_trade_stop_loss(&client_order_id, new_stop)
                        .await
                    {
                        warn!(client_order_id = %client_order_id, error = %e, "Failed to update trade stop_loss");
                    }
                });
            }
        }
    }

    async fn fetch_current_atr(&self) -> Option<f64> {
        let snapshot = match self
            .market_data_provider
            .get_market_snapshot(&self.bot.exchange, &self.bot.symbol)
            .await
        {
            Ok(s) => match AutoMarketSnapshot::from_base(s) {
                Ok(snap) => snap,
                Err(e) => {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to parse indicators for ATR");
                    return None;
                }
            },
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to fetch market snapshot for ATR");
                return None;
            }
        };
        Some(snapshot.indicators.atr)
    }

    async fn check_position_timeout(&mut self) -> bool {
        let mut triggered = false;
        if self.check_position_timeout_side(PositionSide::Long).await {
            triggered = true;
        }
        if self.check_position_timeout_side(PositionSide::Short).await {
            triggered = true;
        }
        triggered
    }

    async fn check_position_timeout_side(&mut self, side: PositionSide) -> bool {
        let max_duration = Duration::from_secs(self.time_config.max_position_duration_secs);
        let opened_at = match side {
            PositionSide::Long => self.position_opened_at_long,
            PositionSide::Short => self.position_opened_at_short,
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        };
        if let Some(opened_at) = opened_at {
            if opened_at.elapsed() > max_duration {
                warn!(
                    bot_id = %self.bot.id,
                    side = %side_str(&side),
                    duration_secs = opened_at.elapsed().as_secs(),
                    "Position held too long, force closing"
                );
                self.close_position(side, "position_timeout").await;
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

        // 按 action 分配 log_id 到对应 side
        match action {
            AutoAction::OpenLong => self.current_log_id_long = log_id,
            AutoAction::OpenShort => self.current_log_id_short = log_id,
            AutoAction::ClosePosition => {
                // ClosePosition 会平掉所有持仓方向；将 log_id 分配给有持仓的一侧
                // （若两侧都有，优先 Long；若都无，回退到 Long 供 intercept 路径更新）
                if self.has_position_side(PositionSide::Long)
                    || !self.has_position_side(PositionSide::Short)
                {
                    self.current_log_id_long = log_id;
                } else {
                    self.current_log_id_short = log_id;
                }
            }
            AutoAction::Hold => {
                // Hold 不执行，暂存到 Long 侧供后续更新
                self.current_log_id_long = log_id;
            }
        }

        let intercept_reason = self.execute_decision(&action, decision.as_ref()).await;
        if let Some(reason) = intercept_reason {
            warn!(bot_id = %self.bot.id, action = %action.as_str(), intercept_reason = %reason, "Decision intercepted");
            let exec_status = match action {
                AutoAction::OpenLong | AutoAction::OpenShort => "open_failed",
                AutoAction::ClosePosition => "close_failed",
                AutoAction::Hold => "hold",
            };
            // 取出两侧的 log_id 进行更新
            let log_ids: Vec<Uuid> = [self.current_log_id_long.take(), self.current_log_id_short.take()]
                .into_iter()
                .flatten()
                .collect();
            for log_id in log_ids {
                if let Err(e) = self
                    .store
                    .update_analysis_log_execution(log_id, exec_status, Some(&reason))
                    .await
                {
                    error!(bot_id = %self.bot.id, error = %e, "Failed to update intercept log");
                }
            }
        } else if matches!(action, AutoAction::Hold) {
            if let Some(log_id) = self.current_log_id_long.take() {
                if let Err(e) = self
                    .store
                    .update_analysis_log_execution(log_id, "hold", None)
                    .await
                {
                    error!(bot_id = %self.bot.id, error = %e, "Failed to update hold log");
                }
            }
            self.current_log_id_short = None;
        }

        if !matches!(action, AutoAction::Hold) {
            if let Err(e) = self.store.update_last_decided(self.bot.id).await {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to update last decided");
            }
        }
    }

    async fn build_llm_prompt(&self) -> Option<(String, String)> {
        let snapshot = match self
            .market_data_provider
            .get_market_snapshot(&self.bot.exchange, &self.bot.symbol)
            .await
        {
            Ok(s) => match AutoMarketSnapshot::from_base(s) {
                Ok(snap) => snap,
                Err(e) => {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to parse indicators for LLM prompt");
                    return None;
                }
            },
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to fetch market snapshot for LLM prompt");
                return None;
            }
        };

        if snapshot.base.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "Market snapshot has zero price, skipping decision");
            return None;
        }

        let account = match self
            .market_data_provider
            .get_account_balance(&self.bot.exchange)
            .await
        {
            Ok(b) => b,
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to fetch account balance for LLM prompt");
                return None;
            }
        };
        let margin_usage_rate = if account.total > 0.0 {
            account.used / account.total
        } else {
            0.0
        };

        // 拼接 Long / Short 双向持仓信息（固定双向格式，空方向显示"无仓位"）
        let position_info = {
            let long_info = match &self.current_long {
                Some(p) if p.is_open() => strategy::format_position_info(
                    p,
                    Some("long"),
                    snapshot.base.current_price,
                ),
                _ => "无仓位".to_string(),
            };
            let short_info = match &self.current_short {
                Some(p) if p.is_open() => strategy::format_position_info(
                    p,
                    Some("short"),
                    snapshot.base.current_price,
                ),
                _ => "无仓位".to_string(),
            };
            format!("多：\n{}\n空：\n{}", long_info, short_info)
        };

        // 拼接双向止损止盈信息
        let stop_take_profit_info = {
            let has_long = self.has_position_side(PositionSide::Long);
            let has_short = self.has_position_side(PositionSide::Short);
            let long_info =
                strategy::format_stop_take_profit(self.stop_loss_long, self.take_profit_long);
            let short_info =
                strategy::format_stop_take_profit(self.stop_loss_short, self.take_profit_short);
            if has_long && has_short {
                if long_info.is_empty() || long_info == "未设置" {
                    if short_info.is_empty() || short_info == "未设置" {
                        "未设置".to_string()
                    } else {
                        format!("空：\n{}", short_info)
                    }
                } else if short_info.is_empty() || short_info == "未设置" {
                    format!("多：\n{}", long_info)
                } else {
                    format!("多：\n{}\n空：\n{}", long_info, short_info)
                }
            } else if has_long {
                long_info
            } else if has_short {
                short_info
            } else {
                "未设置".to_string()
            }
        };

        let position_duration = if self.has_any_position() {
            let mut parts: Vec<String> = Vec::new();
            if self.has_position_side(PositionSide::Long) {
                if let Some(opened_at) = self.position_opened_at_long {
                    let elapsed = opened_at.elapsed();
                    let hours = elapsed.as_secs() / 3600;
                    let mins = (elapsed.as_secs() % 3600) / 60;
                    parts.push(format!("多：{}小时{}分钟", hours, mins));
                } else {
                    parts.push("多：未知".to_string());
                }
            }
            if self.has_position_side(PositionSide::Short) {
                if let Some(opened_at) = self.position_opened_at_short {
                    let elapsed = opened_at.elapsed();
                    let hours = elapsed.as_secs() / 3600;
                    let mins = (elapsed.as_secs() % 3600) / 60;
                    parts.push(format!("空：{}小时{}分钟", hours, mins));
                } else {
                    parts.push("空：未知".to_string());
                }
            }
            parts.join("，")
        } else {
            "无持仓".to_string()
        };

        let recent_close_info = {
            let mut parts: Vec<String> = Vec::new();
            if let Some((side, reason, closed_at)) = &self.last_close_event_long {
                parts.push(format_close_event(side, reason, closed_at));
            }
            if let Some((side, reason, closed_at)) = &self.last_close_event_short {
                parts.push(format_close_event(side, reason, closed_at));
            }
            if parts.is_empty() {
                "无".to_string()
            } else {
                parts.join("\n")
            }
        };

        let ctx = RenderContext {
            timestamp: chrono::Utc::now()
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
            symbol: self.bot.symbol.clone(),
            exchange: self.bot.exchange.clone(),
            total_balance: account.total,
            available_balance: account.free,
            used_margin: account.used,
            margin_usage_rate: margin_usage_rate * 100.0,
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
            min_qty: snapshot.base.min_qty,
            ind: snapshot.indicators,
            // Auto bot 不使用以下字段，填默认值
            grid_status: String::new(),
            last_adjust_time: String::new(),
            current_grid_config: String::new(),
            event_flag: false,
            event_description: String::new(),
        };

        // 使用策略文件（STRATEGIES_DIR/auto/{strategy_file}/）。
        // strategy_file 为必填项：缺失或 loader 未命中时报错并跳过决策（不回退默认值）。
        let (system_prompt, user_prompt) = {
            let file_name = match self.bot.strategy_file.as_deref() {
                Some(f) => f,
                None => {
                    error!(
                        bot_id = %self.bot.id,
                        "Bot has no strategy_file bound — cannot build LLM prompt. \
                         Stopping decision cycle."
                    );
                    return None;
                }
            };

            match self
                .prompt_loader
                .get(StrategyType::Auto, file_name)
                .await
            {
                Some(tpl) => {
                    let user = render(&tpl.user_prompt_template, &ctx);
                    let system = self
                        .bot
                        .system_prompt
                        .as_deref()
                        .unwrap_or(&tpl.system_prompt)
                        .to_string();
                    (system, user)
                }
                None => {
                    error!(
                        bot_id = %self.bot.id,
                        strategy_file = file_name,
                        "Strategy file not found in loader — cannot build LLM prompt. \
                         Stopping decision cycle. Check STRATEGIES_DIR and strategy files."
                    );
                    return None;
                }
            }
        };

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
                        tracing::error!(
                            "LLM result is not a JSON object — cannot insert raw_llm_response"
                        );
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
                        &self.bot.strategy_file,
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
                        &self.bot.strategy_file,
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

        let snapshot = match self
            .market_data_provider
            .get_market_snapshot(&self.bot.exchange, &self.bot.symbol)
            .await
        {
            Ok(s) => match AutoMarketSnapshot::from_base(s) {
                Ok(snap) => snap,
                Err(e) => {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to parse indicators for decision execution");
                    return Some(format!("指标解析失败，跳过决策: {}", e));
                }
            },
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to fetch market snapshot for decision execution");
                return Some(format!("市场快照获取失败，跳过决策: {}", e));
            }
        };

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
                let position_side = match side {
                    "long" => PositionSide::Long,
                    "short" => PositionSide::Short,
                    _ => unreachable!(),
                };

                // per-side 硬卡点：仅检查该方向是否已有仓位
                if self.has_position_side(position_side.clone()) {
                    warn!(bot_id = %self.bot.id, side = %side, "Already has position on this side, cannot open");
                    return Some("该方向已有仓位".to_string());
                }

                if let Some(remaining) = self.cooldown_remaining_secs(side) {
                    let last_event = match position_side {
                        PositionSide::Long => self.last_close_event_long.as_ref(),
                        PositionSide::Short => self.last_close_event_short.as_ref(),
                        PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                    };
                    let (closed_side, close_reason, closed_at) = match last_event {
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
                self.open_position(side, &snapshot).await;

                let pending_set = match position_side {
                    PositionSide::Long => self.pending_open_long.is_some(),
                    PositionSide::Short => self.pending_open_short.is_some(),
                    PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                };
                if !pending_set {
                    return Some("开仓订单发送失败".to_string());
                }
                None
            }
            AutoAction::ClosePosition => {
                if !self.has_any_position() {
                    warn!(bot_id = %self.bot.id, "No position to close");
                    return Some("无仓位可平".to_string());
                }

                // 平掉所有方向的仓位
                let mut any_pending = false;
                if self.has_position_side(PositionSide::Long) {
                    self.close_position(PositionSide::Long, "llm_decision").await;
                    if self.pending_close_long.is_some() {
                        any_pending = true;
                    }
                }
                if self.has_position_side(PositionSide::Short) {
                    self.close_position(PositionSide::Short, "llm_decision").await;
                    if self.pending_close_short.is_some() {
                        any_pending = true;
                    }
                }

                if !any_pending {
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
        snapshot: &AutoMarketSnapshot,
    ) {
        let account = match self
            .market_data_provider
            .get_account_balance(&self.bot.exchange)
            .await
        {
            Ok(b) => b,
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to fetch account balance, skipping open");
                return;
            }
        };

        if account.total <= 0.0 && account.free <= 0.0 {
            warn!(bot_id = %self.bot.id, "Account balance is zero, skipping open");
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

        let stop_loss = strategy::compute_stop_loss(price, side, atr);
        let take_profit = strategy::compute_take_profit(price, side, atr);
        let sl_source = "formula";
        let tp_source = "formula";

        let position_side = match side {
            "long" => PositionSide::Long,
            "short" => PositionSide::Short,
            _ => {
                error!(side = %side, "Unknown position side — refusing to place order");
                return;
            }
        };

        let order_side = match side {
            "long" => Side::Buy,
            "short" => Side::Sell,
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

                let pending = PendingOpen {
                    side: side.to_string(),
                    entry_price: price,
                    position_size: quantity,
                    stop_loss,
                    take_profit,
                    client_order_id,
                    sent_at: tokio::time::Instant::now(),
                };
                match side {
                    "long" => self.pending_open_long = Some(pending),
                    "short" => self.pending_open_short = Some(pending),
                    _ => unreachable!(),
                }
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to send open position order");
            }
        }
    }

    pub(crate) async fn close_position(&mut self, side: PositionSide, close_reason: &str) {
        let position = match self.get_position(&side) {
            Some(p) if p.is_open() => p.clone(),
            _ => return,
        };

        let side_str = side_str(&side);
        let entry_price = position.entry_price;
        let position_size = position.quantity;
        let position_id = position.id;

        // 优先使用 per-side 缓存的 position_id；若为 nil 则回退到 PlaceOrder 路径
        if position_id != Uuid::nil() {
            let client_order_id = client_order_id::format_auto_close(self.bot.id, side_str);

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
                        bot_id = %self.bot.id, side = %side_str,
                        entry_price = entry_price,
                        close_price = self.current_price,
                        close_reason = %close_reason,
                        "Position closing order sent via ClosePosition, awaiting confirmation"
                    );

                    let pending = PendingClose {
                        side: side_str.to_string(),
                        close_reason: close_reason.to_string(),
                        entry_price,
                        position_size,
                        client_order_id,
                        sent_at: tokio::time::Instant::now(),
                    };
                    match side {
                        PositionSide::Long => self.pending_close_long = Some(pending),
                        PositionSide::Short => self.pending_close_short = Some(pending),
                        PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                    }
                }
                Err(e) => {
                    error!(bot_id = %self.bot.id, side = %side_str, error = %e, "Failed to send close position order");
                }
            }
        } else {
            // 回退路径：缓存 position_id 为 nil 时使用 PlaceOrder 反向单
            let (order_side, position_side_field) = match side {
                PositionSide::Long => (Side::Sell, Some(PositionSide::Long)),
                PositionSide::Short => (Side::Buy, Some(PositionSide::Short)),
                PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
            };

            let client_order_id = client_order_id::format_auto_close(self.bot.id, side_str);

            let result = self
                .order_executor
                .send_command(OrderCommand::PlaceOrder {
                    symbol: self.bot.symbol.clone(),
                    side: order_side,
                    amount: position_size,
                    price: None,
                    position_side: position_side_field,
                    position_id: None,
                    client_order_id: Some(client_order_id.clone()),
                })
                .await;

            match result {
                Ok(()) => {
                    info!(
                        bot_id = %self.bot.id, side = %side_str,
                        entry_price = entry_price,
                        close_price = self.current_price,
                        close_reason = %close_reason,
                        "Position closing order sent via PlaceOrder, awaiting confirmation"
                    );

                    let pending = PendingClose {
                        side: side_str.to_string(),
                        close_reason: close_reason.to_string(),
                        entry_price,
                        position_size,
                        client_order_id,
                        sent_at: tokio::time::Instant::now(),
                    };
                    match side {
                        PositionSide::Long => self.pending_close_long = Some(pending),
                        PositionSide::Short => self.pending_close_short = Some(pending),
                        PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                    }
                }
                Err(e) => {
                    error!(bot_id = %self.bot.id, side = %side_str, error = %e, "Failed to send close position order (fallback path)");
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
                let side = position.side.clone();
                let cached_id = self.get_position(&side).map(|p| p.id);
                let is_ours = match cached_id {
                    Some(pid) => pid == position.id,
                    None => {
                        let persisted = match side {
                            PositionSide::Long => self.bot.position_id_long,
                            PositionSide::Short => self.bot.position_id_short,
                            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                        };
                        match persisted.filter(|id| *id != Uuid::nil()) {
                            Some(pid) => pid == position.id,
                            None => position.is_open(),
                        }
                    }
                };
                if !is_ours {
                    return;
                }

                let needs_update = match side {
                    PositionSide::Long => {
                        self.bot.position_id_long.is_none()
                            || self.bot.position_id_long == Some(Uuid::nil())
                    }
                    PositionSide::Short => {
                        self.bot.position_id_short.is_none()
                            || self.bot.position_id_short == Some(Uuid::nil())
                    }
                    PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                };
                if needs_update {
                    match side {
                        PositionSide::Long => self.bot.position_id_long = Some(position.id),
                        PositionSide::Short => self.bot.position_id_short = Some(position.id),
                        PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                    }
                    if let Err(e) = self
                        .store
                        .update_position(
                            self.bot.id,
                            self.bot.position_id_long,
                            self.bot.position_id_short,
                        )
                        .await
                    {
                        warn!(bot_id = %self.bot.id, error = %e, "Failed to update position");
                    }
                }
                match side {
                    PositionSide::Long => self.current_long = Some(position),
                    PositionSide::Short => self.current_short = Some(position),
                    PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                }
            }
            EngineEvent::PositionClosed { position } => {
                if position.symbol != self.bot.symbol {
                    return;
                }
                let side = position.side;
                let cached_id = self.get_position(&side).map(|p| p.id);
                let is_ours = match cached_id {
                    Some(pid) => pid == position.id,
                    None => {
                        let persisted = match side {
                            PositionSide::Long => self.bot.position_id_long,
                            PositionSide::Short => self.bot.position_id_short,
                            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                        };
                        match persisted.filter(|id| *id != Uuid::nil()) {
                            Some(pid) => pid == position.id,
                            None => self.get_position(&side).is_some(),
                        }
                    }
                };
                if !is_ours {
                    return;
                }

                // 外部平仓场景（apply_pending_close 未处理）：完整清理 per-side 字段
                // apply_pending_close 已清空 current_{side} 和 bot.position_id_{side}，
                // is_ours 检查会直接 return，不会进入此分支。
                // 进入此分支说明是 PE 直接推送的 PositionClosed（外部止损单/强平），
                // 需补充与 apply_pending_close 一致的字段清理，否则残留状态影响后续决策。
                let now = chrono::Utc::now();
                match side {
                    PositionSide::Long => {
                        self.current_long = None;
                        self.stop_loss_long = 0.0;
                        self.take_profit_long = 0.0;
                        self.position_opened_at_long = None;
                        self.current_open_client_order_id_long = None;
                        self.current_log_id_long = None;
                        self.current_open_fee_long = 0.0;
                        // 外部平仓设置 last_close_event 触发冷却（compute_cooldown_secs 对未知 reason 返回 15 分钟）
                        self.last_close_event_long =
                            Some(("long".to_string(), "external_close".to_string(), now));
                        self.bot.position_id_long = None;
                    }
                    PositionSide::Short => {
                        self.current_short = None;
                        self.stop_loss_short = 0.0;
                        self.take_profit_short = 0.0;
                        self.position_opened_at_short = None;
                        self.current_open_client_order_id_short = None;
                        self.current_log_id_short = None;
                        self.current_open_fee_short = 0.0;
                        self.last_close_event_short =
                            Some(("short".to_string(), "external_close".to_string(), now));
                        self.bot.position_id_short = None;
                    }
                    PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                }
                warn!(
                    bot_id = %self.bot.id, side = ?side,
                    "Position closed by external event (not initiated by worker) — \
                     per-side state cleared, cooldown armed with reason=external_close"
                );
                if let Err(e) = self
                    .store
                    .update_position(
                        self.bot.id,
                        self.bot.position_id_long,
                        self.bot.position_id_short,
                    )
                    .await
                {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to update position");
                }
            }
            EngineEvent::PositionOpened { position } => {
                if position.symbol != self.bot.symbol {
                    return;
                }

                let needs_update = match position.side {
                    PositionSide::Long => {
                        self.bot.position_id_long.is_none()
                            || self.bot.position_id_long == Some(Uuid::nil())
                    }
                    PositionSide::Short => {
                        self.bot.position_id_short.is_none()
                            || self.bot.position_id_short == Some(Uuid::nil())
                    }
                    PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                };
                if needs_update {
                    match position.side {
                        PositionSide::Long => self.bot.position_id_long = Some(position.id),
                        PositionSide::Short => self.bot.position_id_short = Some(position.id),
                        PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                    }
                    if let Err(e) = self
                        .store
                        .update_position(
                            self.bot.id,
                            self.bot.position_id_long,
                            self.bot.position_id_short,
                        )
                        .await
                    {
                        warn!(bot_id = %self.bot.id, error = %e, "Failed to update position");
                    }
                }
                match position.side {
                    PositionSide::Long => self.current_long = Some(position),
                    PositionSide::Short => self.current_short = Some(position),
                    PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                }
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

                let order_position_id = order.position_id;

                let fill_price = order.fill_price.or(order.request_price).unwrap_or_else(|| {
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

                let cid = match order.client_order_id.as_deref() {
                    Some(cid) => cid,
                    None => return,
                };

                let is_open_long = self
                    .pending_open_long
                    .as_ref()
                    .map(|p| p.client_order_id == cid)
                    .unwrap_or(false);
                let is_open_short = self
                    .pending_open_short
                    .as_ref()
                    .map(|p| p.client_order_id == cid)
                    .unwrap_or(false);
                let is_close_long = self
                    .pending_close_long
                    .as_ref()
                    .map(|p| p.client_order_id == cid)
                    .unwrap_or(false);
                let is_close_short = self
                    .pending_close_short
                    .as_ref()
                    .map(|p| p.client_order_id == cid)
                    .unwrap_or(false);

                if is_open_long {
                    self.apply_pending_open(PositionSide::Long, fill_price, filled_qty, order.fee, order_position_id)
                        .await;
                } else if is_open_short {
                    self.apply_pending_open(PositionSide::Short, fill_price, filled_qty, order.fee, order_position_id)
                        .await;
                } else if is_close_long {
                    self.apply_pending_close(PositionSide::Long, fill_price, filled_qty, order.fee)
                        .await;
                } else if is_close_short {
                    self.apply_pending_close(PositionSide::Short, fill_price, filled_qty, order.fee)
                        .await;
                }
            }
            OrderEvent::OrderFailed {
                order_id: _,
                reason,
            } if self.is_pending() => {
                let was_open_long = self.pending_open_long.is_some();
                let was_open_short = self.pending_open_short.is_some();
                let was_open = was_open_long || was_open_short;
                warn!(
                    bot_id = %self.bot.id,
                    reason = %reason,
                    was_open,
                    "Order failed, rolling back pending state"
                );
                self.rollback_pending_open(PositionSide::Long);
                self.rollback_pending_open(PositionSide::Short);
                self.rollback_pending_close(PositionSide::Long);
                self.rollback_pending_close(PositionSide::Short);

                let exec_status = if was_open {
                    "open_failed"
                } else {
                    "close_failed"
                };
                let log_ids: Vec<Uuid> = [
                    self.current_log_id_long.take(),
                    self.current_log_id_short.take(),
                ]
                .into_iter()
                .flatten()
                .collect();
                for log_id in log_ids {
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

    pub(crate) async fn apply_pending_open(
        &mut self,
        side: PositionSide,
        fill_price: f64,
        filled_qty: f64,
        fee: f64,
        order_position_id: Option<Uuid>,
    ) {
        let pending = match side {
            PositionSide::Long => self.pending_open_long.take(),
            PositionSide::Short => self.pending_open_short.take(),
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        };
        let Some(pending) = pending else { return };

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
            let atr = match self
                .market_data_provider
                .get_market_snapshot(&self.bot.exchange, &self.bot.symbol)
                .await
            {
                Ok(s) => match AutoMarketSnapshot::from_base(s) {
                    Ok(snap) if snap.indicators.atr > 0.0 => snap.indicators.atr,
                    Ok(_) => {
                        warn!(bot_id = %self.bot.id, "ATR is zero in snapshot, using fill_price * 2% as fallback for SL/TP");
                        fill_price * 0.02
                    }
                    Err(e) => {
                        warn!(bot_id = %self.bot.id, error = %e, "Failed to parse indicators for SL/TP recalculation, using fill_price * 2%");
                        fill_price * 0.02
                    }
                },
                Err(e) => {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to fetch market snapshot for SL/TP recalculation, using fill_price * 2%");
                    fill_price * 0.02
                }
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

        // 写入 per-side 字段
        match side {
            PositionSide::Long => {
                self.stop_loss_long = stop_loss;
                self.take_profit_long = take_profit;
                self.position_opened_at_long = Some(tokio::time::Instant::now());
                self.current_open_fee_long = fee;
            }
            PositionSide::Short => {
                self.stop_loss_short = stop_loss;
                self.take_profit_short = take_profit;
                self.position_opened_at_short = Some(tokio::time::Instant::now());
                self.current_open_fee_short = fee;
            }
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        }

        // 从订单事件回填 per-side position_id（PE PositionOpened 事件可能尚未到达）
        if let Some(pid) = order_position_id.filter(|id| *id != Uuid::nil()) {
            match side {
                PositionSide::Long => {
                    if self.bot.position_id_long.filter(|id| *id != Uuid::nil()).is_none() {
                        self.bot.position_id_long = Some(pid);
                    }
                }
                PositionSide::Short => {
                    if self.bot.position_id_short.filter(|id| *id != Uuid::nil()).is_none() {
                        self.bot.position_id_short = Some(pid);
                    }
                }
                PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
            }
        }

        self.save_position().await;

        let trade_type = match pending.side.as_str() {
            "long" => "open_long",
            "short" => "open_short",
            _ => {
                error!(side = %pending.side, "Unknown pending side — skipping trade record");
                return;
            }
        };

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
                &self.bot.strategy_file,
            )
            .await
        {
            Ok(()) => {
                info!(bot_id = %self.bot.id, client_order_id = %client_order_id, trade_type, stop_loss, take_profit, "Open trade recorded");
                match side {
                    PositionSide::Long => {
                        self.current_open_client_order_id_long = Some(client_order_id)
                    }
                    PositionSide::Short => {
                        self.current_open_client_order_id_short = Some(client_order_id)
                    }
                    PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                }
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

        let log_id = match side {
            PositionSide::Long => self.current_log_id_long.take(),
            PositionSide::Short => self.current_log_id_short.take(),
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        };
        if let Some(log_id) = log_id {
            if let Err(e) = self
                .store
                .update_analysis_log_execution(log_id, "open", None)
                .await
            {
                error!(bot_id = %self.bot.id, error = %e, "Failed to update open execution status");
            }
        }
    }

    pub(crate) async fn apply_pending_close(
        &mut self,
        side: PositionSide,
        fill_price: f64,
        filled_qty: f64,
        fee: f64,
    ) {
        let pending = match side {
            PositionSide::Long => self.pending_close_long.take(),
            PositionSide::Short => self.pending_close_short.take(),
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        };
        let Some(pending) = pending else { return };

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
        let open_fee = match side {
            PositionSide::Long => self.current_open_fee_long,
            PositionSide::Short => self.current_open_fee_short,
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        };
        let total_fee = open_fee + fee;
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
            open_fee = open_fee, close_fee = fee, total_fee,
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

        // 清除 per-side 字段
        match side {
            PositionSide::Long => {
                self.stop_loss_long = 0.0;
                self.take_profit_long = 0.0;
                self.current_long = None;
                self.position_opened_at_long = None;
                self.current_open_fee_long = 0.0;
            }
            PositionSide::Short => {
                self.stop_loss_short = 0.0;
                self.take_profit_short = 0.0;
                self.current_short = None;
                self.position_opened_at_short = None;
                self.current_open_fee_short = 0.0;
            }
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        }

        // 写入 per-side last_close_event
        let close_event = (
            pending.side.clone(),
            pending.close_reason.clone(),
            chrono::Utc::now(),
        );
        match side {
            PositionSide::Long => self.last_close_event_long = Some(close_event),
            PositionSide::Short => self.last_close_event_short = Some(close_event),
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        }

        // 平仓后清空对应 side 的 position_id
        match side {
            PositionSide::Long => self.bot.position_id_long = None,
            PositionSide::Short => self.bot.position_id_short = None,
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        }

        self.save_position().await;
        self.save_stats().await;

        let close_reason = &pending.close_reason;
        let close_client_order_id = pending.client_order_id.clone();

        let open_client_order_id = match side {
            PositionSide::Long => self.current_open_client_order_id_long.take(),
            PositionSide::Short => self.current_open_client_order_id_short.take(),
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        };
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
            None => match self.store.find_open_trade(self.bot.id).await {
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
                            &self.bot.strategy_file,
                        )
                        .await
                    {
                        warn!(bot_id = %self.bot.id, error = %e, "Failed to record orphaned close trade");
                    }
                }
                Err(e) => {
                    error!(bot_id = %self.bot.id, error = %e, "Failed to find open trade for close");
                }
            },
        }

        let log_id = match side {
            PositionSide::Long => self.current_log_id_long.take(),
            PositionSide::Short => self.current_log_id_short.take(),
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        };
        if let Some(log_id) = log_id {
            if let Err(e) = self
                .store
                .update_analysis_log_execution(log_id, "close", None)
                .await
            {
                error!(bot_id = %self.bot.id, error = %e, "Failed to update close execution status");
            }
        }
    }

    fn rollback_pending_open(&mut self, side: PositionSide) {
        let pending = match side {
            PositionSide::Long => self.pending_open_long.take(),
            PositionSide::Short => self.pending_open_short.take(),
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        };
        if pending.is_some() {
            warn!(bot_id = %self.bot.id, side = %side_str(&side), "Rolling back pending open order");
        }
    }

    fn rollback_pending_close(&mut self, side: PositionSide) {
        let pending = match side {
            PositionSide::Long => self.pending_close_long.take(),
            PositionSide::Short => self.pending_close_short.take(),
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        };
        if pending.is_some() {
            warn!(bot_id = %self.bot.id, side = %side_str(&side), "Rolling back pending close order");
        }
    }
}

// ===== 模块级辅助函数 =====

fn side_str(side: &PositionSide) -> &'static str {
    match side {
        PositionSide::Long => "long",
        PositionSide::Short => "short",
        PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
    }
}

/// 从 client_order_id 前缀解析 side。
/// `AOL__` / `ACL__` → Long，`AOS__` / `ACS__` → Short
fn parse_side_from_client_order_id(cid: &str) -> Option<PositionSide> {
    if cid.starts_with("AOL") || cid.starts_with("ACL") {
        Some(PositionSide::Long)
    } else if cid.starts_with("AOS") || cid.starts_with("ACS") {
        Some(PositionSide::Short)
    } else {
        None
    }
}

fn format_close_event(
    side: &str,
    close_reason: &str,
    closed_at: &chrono::DateTime<chrono::Utc>,
) -> String {
    let side_cn = match side {
        "long" => "多",
        "short" => "空",
        _ => "未知",
    };
    let reason_cn = match close_reason {
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
