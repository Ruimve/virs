mod side_state;
mod risk;
mod decision;
mod executor;
mod events;
mod client_order_id;

#[cfg(test)]
mod side_state_tests;
#[cfg(test)]
mod worker_tests;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tracing::{error, info, warn};
use uuid::Uuid;
use virs_task::{spawn_periodic, Stop};

use crate::auto::ai::AutoAiService;
use crate::auto::strategy;
use virs_type::auto::{AutoBotConfig, AutoStore};
use virs_type::bot::{MarketDataProvider, OrderEvent, OrderExecutor};
use virs_type::KlineEvent;
use virs_strategy::prompt::PromptLoader;
use virs_config::TimeConfig;
use virs_type::PositionSide;
use virs_type::position::{EngineEvent, Position};

pub(crate) use side_state::{PendingClose, PendingOpen, SideState};

pub struct AutoWorker {
    pub(crate) bot: AutoBotConfig,
    kline_rx: broadcast::Receiver<KlineEvent>,
    order_executor: Arc<dyn OrderExecutor>,
    ai_service: Arc<AutoAiService>,
    store: Arc<dyn AutoStore>,
    market_data_provider: Arc<dyn MarketDataProvider>,
    event_rx: broadcast::Receiver<OrderEvent>,
    pe_event_rx: broadcast::Receiver<EngineEvent>,

    pub(crate) current_price: f64,
    pub(crate) consecutive_losses: i32,
    pub(crate) paused: bool,
    pub(crate) trailing_stop_dirty: bool,
    pub(crate) time_config: TimeConfig,
    pub(crate) prompt_loader: PromptLoader,

    pub(crate) long: SideState,
    pub(crate) short: SideState,
}

impl AutoWorker {
    pub(crate) fn side(&self, side: &PositionSide) -> &SideState {
        match side {
            PositionSide::Long => &self.long,
            PositionSide::Short => &self.short,
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        }
    }

    pub(crate) fn side_mut(&mut self, side: &PositionSide) -> &mut SideState {
        match side {
            PositionSide::Long => &mut self.long,
            PositionSide::Short => &mut self.short,
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        }
    }
}

impl AutoWorker {
    pub fn new(
        bot: AutoBotConfig,
        kline_rx: broadcast::Receiver<KlineEvent>,
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
            kline_rx,
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
            long: SideState::default(),
            short: SideState::default(),
        }
    }

    pub(crate) fn get_position(&self, side: &PositionSide) -> Option<&virs_type::position::Position> {
        self.side(side).get_position()
    }

    pub(crate) fn has_position_side(&self, side: PositionSide) -> bool {
        self.side(&side).has_position()
    }

    pub(crate) fn has_any_position(&self) -> bool {
        self.has_position_side(PositionSide::Long) || self.has_position_side(PositionSide::Short)
    }

    pub(crate) fn is_pending_side(&self, side: PositionSide) -> bool {
        self.side(&side).is_pending()
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.is_pending_side(PositionSide::Long) || self.is_pending_side(PositionSide::Short)
    }

    pub(crate) fn cooldown_remaining_secs(&self, new_side: &str) -> Option<i64> {
        let side_state = match new_side {
            "long" => &self.long,
            "short" => &self.short,
            _ => return None,
        };
        let (closed_side, reason, closed_at) = side_state.last_close_event.as_ref()?;
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

                self.long.position = found_long;
                self.short.position = found_short;
                true
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to query PE for positions, relying on cached state");
                false
            }
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
            error!(bot_id = %self.bot.id, error = %e, "Failed to update stats");
        }
    }

    pub(crate) async fn check_pending_timeout(&mut self) {
        let now = tokio::time::Instant::now();
        let pending_timeout = Duration::from_secs(self.time_config.pending_order_timeout_secs);
        let mut timed_out_open = false;
        let mut timed_out_close = false;
        let mut timed_out_sides: Vec<PositionSide> = Vec::new();

        for side in [PositionSide::Long, PositionSide::Short] {
            let side_str = match side {
                PositionSide::Long => "long",
                PositionSide::Short => "short",
                PositionSide::Unknown(_) => unreachable!(),
            };
            let bot_id = self.bot.id;
            let s = self.side_mut(&side);
            let mut side_timed_out = false;

            if let Some(ref pending) = s.pending_open {
                if now.duration_since(pending.sent_at) > pending_timeout {
                    warn!(bot_id = %bot_id, side = side_str, "Pending open order timed out, clearing");
                    s.pending_open = None;
                    timed_out_open = true;
                    side_timed_out = true;
                }
            }
            if let Some(ref pending) = s.pending_close {
                if now.duration_since(pending.sent_at) > pending_timeout {
                    warn!(bot_id = %bot_id, side = side_str, "Pending close order timed out, clearing");
                    s.pending_close = None;
                    timed_out_close = true;
                    side_timed_out = true;
                }
            }

            if side_timed_out {
                timed_out_sides.push(side);
            }
        }

        if timed_out_open || timed_out_close {
            let exec_status = if timed_out_open {
                "open_failed"
            } else {
                "close_failed"
            };
            for side in timed_out_sides {
                if let Some(log_id) = self.side_mut(&side).log_id.take() {
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
    }

    pub(crate) fn matches_pending_order(&self, client_order_id: Option<&str>) -> bool {
        match client_order_id {
            Some(cid) => {
                self.long.pending_open.as_ref().is_some_and(|p| p.client_order_id == cid)
                    || self.short.pending_open.as_ref().is_some_and(|p| p.client_order_id == cid)
                    || self.long.pending_close.as_ref().is_some_and(|p| p.client_order_id == cid)
                    || self.short.pending_close.as_ref().is_some_and(|p| p.client_order_id == cid)
            }
            None => false,
        }
    }

    pub async fn run(mut self, stop: Stop) {
        info!(bot_id = %self.bot.id, "Waiting for first kline event to initialize price...");
        loop {
            tokio::select! {
                ev = self.kline_rx.recv() => {
                    match ev {
                        Ok(event) => {
                            if event.exchange == self.bot.exchange
                                && event.symbol == self.bot.symbol
                                && event.candle.close > 0.0
                            {
                                self.current_price = event.candle.close;
                                info!(bot_id = %self.bot.id, price = self.current_price, "Initial price obtained from kline event");
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(bot_id = %self.bot.id, lagged = n, "KlineEvent lagged while waiting for initial price");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            error!(bot_id = %self.bot.id, "KlineEvent channel closed while waiting for initial price, setting error status");
                            if let Err(e) = self.store.update_bot_status(self.bot.id, "error").await {
                                error!(error = %e, "Failed to update bot status to error");
                            }
                            return;
                        }
                    }
                }
                _ = stop.cancelled() => {
                    return;
                }
            }
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
                    "long" => self.long.last_close_event = Some(event),
                    "short" => self.short.last_close_event = Some(event),
                    _ => {
                        self.long.last_close_event = Some(event.clone());
                        self.short.last_close_event = Some(event);
                    }
                }
            }
            Ok(None) => {}
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to load last closed trade");
            }
        }

        let pe_ok = self.refresh_position_from_pe().await;

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
                            let s = &mut self.long;
                            s.open_client_order_id = Some(client_order_id);
                            s.stop_loss = sl;
                            s.take_profit = tp;
                            s.position_opened_at = Some(position_opened_at);
                        }
                        Some(PositionSide::Short) => {
                            let s = &mut self.short;
                            s.open_client_order_id = Some(client_order_id);
                            s.stop_loss = sl;
                            s.take_profit = tp;
                            s.position_opened_at = Some(position_opened_at);
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
                        _ = stop.cancelled() => {
                            return;
                        }
                        _ = tokio::time::sleep(Duration::from_millis(200)) => {}
                    }
                }
            }
        }

        let skip_llm = if self.has_any_position() {
            if self.long.has_position() && self.long.position_opened_at.is_none() {
                self.long.position_opened_at = Some(tokio::time::Instant::now());
            }
            if self.short.has_position() && self.short.position_opened_at.is_none() {
                self.short.position_opened_at = Some(tokio::time::Instant::now());
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

        let (llm_signal_tx, mut llm_signal_rx) = tokio::sync::mpsc::channel::<()>(1);
        let llm_handle = {
            let interval_secs = self.bot.decide_interval_secs.max(60) as u64;
            spawn_periodic(
                "llm_timer",
                Duration::from_secs(interval_secs),
                false,
                move || {
                    let tx = llm_signal_tx.clone();
                    async move {
                        let _ = tx.send(()).await;
                    }
                },
            )
        };

        loop {
            tokio::select! {
                _ = stop.cancelled() => {
                    break;
                }
                ev = self.kline_rx.recv() => {
                    match ev {
                        Ok(event) => {
                            if event.exchange == self.bot.exchange && event.symbol == self.bot.symbol {
                                if event.candle.close > 0.0 {
                                    self.current_price = event.candle.close;
                                }
                                if !self.paused {
                                    self.on_price_tick().await;
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(bot_id = %self.bot.id, lagged = n, "KlineEvent lagged");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            warn!(bot_id = %self.bot.id, "KlineEvent channel closed");
                            break;
                        }
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

        llm_handle.cancel();
        llm_handle.join().await;

        self.save_position().await;
        self.save_stats().await;
    }
}

pub(super) fn side_str(side: &PositionSide) -> &'static str {
    match side {
        PositionSide::Long => "long",
        PositionSide::Short => "short",
        PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
    }
}

pub(super) fn parse_side_from_client_order_id(cid: &str) -> Option<PositionSide> {
    if cid.starts_with("AOL") || cid.starts_with("ACL") {
        Some(PositionSide::Long)
    } else if cid.starts_with("AOS") || cid.starts_with("ACS") {
        Some(PositionSide::Short)
    } else {
        None
    }
}

pub(super) fn format_close_event(
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
