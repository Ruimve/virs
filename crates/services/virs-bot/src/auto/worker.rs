//! Auto worker — individual auto trading bot execution.

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
use virs_types::enums::PositionSide;
use virs_types::position::{EngineEvent, Position};

// T12: Constants replaced by TimeConfig (loaded from env vars)
// const PENDING_ORDER_TIMEOUT: Duration = Duration::from_secs(60);
// const MAX_POSITION_DURATION: Duration = Duration::from_secs(48 * 3600);

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
    /// 平仓原因：stop_loss/take_profit/position_timeout/llm_decision
    /// 由代码逻辑决定（不由 LLM 决定），用于冷却期判断和 DB 记录
    pub close_reason: String,
    pub entry_price: f64,
    pub position_size: f64,
    /// #14: 下单时刻的未实现盈亏，待接入滑点检测后移除此属性
    #[allow(dead_code)]
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
    /// 当前 LLM 决策日志 ID（handle_llm_result 创建 log 时返回，执行回填时 UPDATE 用）
    /// - 拦截时：UPDATE 设置 intercept_reason + status='intercepted'
    /// - 开仓订单成交：UPDATE 设置 execution_status='open'
    /// - 平仓订单成交：UPDATE 设置 execution_status='close'
    pub(crate) current_log_id: Option<Uuid>,
    /// 当前开仓手续费（平仓时计算总手续费用）
    pub(crate) current_open_fee: f64,
    /// 当前仓位的风控边界（内存态，开仓时由 LLM 决策写入，trailing stop 更新 stop_loss；
    /// 重启时从 qd_auto_trades 表的 open 记录恢复；平仓后清零）
    pub(crate) stop_loss: f64,
    pub(crate) take_profit: f64,
    /// 最近一次平仓事件（用于冷却期判断和 LLM 上下文反思）
    /// 字段：(平仓方向 long/short, 平仓原因, 平仓时间)
    /// 重启时从 qd_auto_trades 表最近一条 closed 记录恢复
    pub(crate) last_close_event: Option<(String, String, chrono::DateTime<chrono::Utc>)>,
    /// T12: 时间配置（从环境变量加载，替代硬编码常量）
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
            current_trade_id: None,
            current_log_id: None,
            current_open_fee: 0.0,
            stop_loss: 0.0,
            take_profit: 0.0,
            last_close_event: None,
            time_config,
        }
    }

    /// 当前仓位方向（"long"/"short"/"none"）
    pub(crate) fn current_side_str(&self) -> String {
        match &self.current_position {
            Some(p) if p.is_open() => match p.side {
                PositionSide::Long => "long".to_string(),
                PositionSide::Short => "short".to_string(),
            },
            _ => "none".to_string(),
        }
    }

    pub(crate) fn is_spot(&self) -> bool {
        self.bot.market_type.is_spot()
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

    /// 检查开仓冷却期。
    /// 规则（按平仓原因分级，绝对时间，不受 decide_interval_secs 影响）：
    ///   - stop_loss：同方向冷却 30 分钟（防止趋势初期反复扫损）
    ///   - take_profit：同方向冷却 15 分钟（防止追高/追低）
    ///   - trend_reversal：反方向冷却 15 分钟（等待新趋势结构形成）
    ///   - 其他（position_timeout/risk_management/llm_decision）：双向冷却 15 分钟
    ///     返回 Some(剩余秒数) 表示仍在冷却中，None 表示可以开仓。
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

    /// 直接查询 PositionEngine 当前 Open 仓位，刷新 current_position 缓存。
    /// 防止 PE broadcast 事件丢失导致缓存失效 → 重复开仓。
    pub(crate) async fn refresh_position_from_pe(&mut self) {
        match self
            .order_executor
            .query_open_position(&self.bot.symbol)
            .await
        {
            Ok(Some(pe_pos))
                if pe_pos.is_open() && pe_pos.size.abs() > 1e-8 =>
            {
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
                    if let Err(e) = self
                        .store
                        .update_position(self.bot.id, self.bot.position_id)
                        .await
                    {
                        warn!(bot_id = %self.bot.id, error = %e, "Failed to update position");
                    }
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
            .get_price(
                &self.bot.exchange,
                &self.bot.symbol,
                self.bot.market_type.as_str(),
            )
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

        // 回填 LLM log 执行状态（仅 LLM 决策触发的订单才有 log_id）
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
                // 精确匹配 pending 状态的 client_order_id
                // open: "auto:{long|short}:{bot_id}"
                // close: "auto:close:{reason}:{bot_id}"
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

    // ── 主运行循环 ──────────────────────────────────────────

    pub async fn run(&mut self, mut shutdown_rx: tokio::sync::mpsc::Receiver<()>) {
        // 获取初始价格
        let max_retries = self.time_config.initial_price_max_retries;
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

        // 加载连续亏损次数
        match self.store.load_consecutive_losses(self.bot.id).await {
            Ok(losses) => {
                self.consecutive_losses = losses;
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to load consecutive losses, starting from 0");
            }
        }

        // 恢复最近平仓事件（用于冷却期判断；即使当前无仓位也需要恢复，防止重启后立即重入）
        match self.store.find_last_closed_trade(self.bot.id).await {
            Ok(Some((side, close_reason, closed_at))) => {
                self.last_close_event = Some((side.clone(), close_reason.clone(), closed_at));
            }
            Ok(None) => {}
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to load last closed trade");
            }
        }

        // 孤儿 trade 检测：bot.position_id 为空但 qd_auto_trades 仍有 open 记录
        // 这种情况通常由 PE 仓位丢失/重启超时导致，标记为 orphaned（保留开仓数据用于回溯）
        if self
            .bot
            .position_id
            .filter(|id| *id != Uuid::nil())
            .is_none()
        {
            match self.store.find_open_trade(self.bot.id).await {
                Ok(Some((trade_id, _sl, _tp, _opened_at))) => {
                    warn!(
                        bot_id = %self.bot.id,
                        trade_id = %trade_id,
                        "Orphaned trade detected: open trade exists but bot.position_id is empty, marking as orphaned"
                    );
                    if let Err(e) = self.store.mark_trade_orphaned(trade_id).await {
                        warn!(bot_id = %self.bot.id, trade_id = %trade_id, error = %e, "Failed to mark trade as orphaned");
                    }
                }
                Ok(None) => {
                    // 无 open trade，正常状态
                }
                Err(e) => {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to check orphaned trade");
                }
            }
        }

        // 如果 bot 有 position_id，等待 PE 推送仓位事件以恢复 current_position
        // 这确保重启后能立即获取仓位状态，不会错过止损止盈检查
        if self
            .bot
            .position_id
            .filter(|id| *id != Uuid::nil())
            .is_some()
        {
            // 同时从 DB 恢复 current_trade_id（用于平仓时 UPDATE 对应的开仓记录）
            // 以及 stop_loss/take_profit（用于恢复内存中的风控边界）
            // T11: 同时恢复 opened_at，用于计算 position_opened_at（避免重启后 48h 超时检查重置）
            match self.store.find_open_trade(self.bot.id).await {
                Ok(Some((trade_id, sl, tp, opened_at))) => {
                    self.current_trade_id = Some(trade_id);
                    self.stop_loss = sl;
                    self.take_profit = tp;
                    // T11: 从 DB opened_at 恢复 position_opened_at
                    // Instant 是单调时钟，不能从 DateTime 直接构造
                    // 通过计算 elapsed 后反推 Instant
                    let elapsed = chrono::Utc::now().signed_duration_since(opened_at);
                    let elapsed_secs = elapsed.num_seconds().max(0) as u64;
                    let elapsed_dur = std::time::Duration::from_secs(elapsed_secs);
                    self.position_opened_at =
                        tokio::time::Instant::now().checked_sub(elapsed_dur);
                    if self.position_opened_at.is_none() {
                        // checked_sub 返回 None（elapsed 过大超过 Instant 范围）— fallback
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
            // 主动从 PE 查询仓位状态（不依赖事件推送，避免 full_sync 只发出 PositionSynced 导致超时）
            self.refresh_position_from_pe().await;
            // 如果主动查询已恢复，跳过事件等待循环
            if self.current_position.is_none() {
                // 主动查询未恢复，等待 PE 事件推送
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

        // 初始 LLM 分析
        let skip_llm = if self.has_position() {
            // T11: 仅在 DB 恢复未设置时才用 Instant::now()（首次开仓场景）
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

        // LLM 周期性决策定时器
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

        // 主事件循环
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

    // ── 价格 tick 处理 ──────────────────────────────────────

    pub(crate) async fn on_price_tick(&mut self) {
        if self.current_price <= 0.0 {
            return;
        }

        self.check_pending_timeout().await;

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
            // 分别判断止损和止盈是否触发
            let stop_triggered = self.stop_loss > 0.0
                && ((side == "long" && self.current_price <= self.stop_loss)
                    || (side == "short" && self.current_price >= self.stop_loss));
            let take_triggered = self.take_profit > 0.0
                && ((side == "long" && self.current_price >= self.take_profit)
                    || (side == "short" && self.current_price <= self.take_profit));

            // 同时触发时优先止盈（盈利出场优先，对策略和心理影响更小）
            let close_reason = if take_triggered {
                "take_profit"
            } else if stop_triggered {
                "stop_loss"
            } else {
                // 理论上不会走到这里（should_close=true 但两者都没触发）
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
            // 同步更新 trade 维度的 stop_loss（异步执行，失败仅记录日志）
            if let Some(trade_id) = self.current_trade_id {
                let store = self.store.clone();
                tokio::spawn(async move {
                    if let Err(e) = store.update_trade_stop_loss(trade_id, new_stop).await {
                        warn!(trade_id = %trade_id, error = %e, "Failed to update trade stop_loss");
                    }
                });
            }
        }
    }

    async fn fetch_current_atr(&self) -> f64 {
        let snapshot = AutoMarketSnapshot::from_base(
            self.market_data_provider
                .get_market_snapshot(
                    &self.bot.exchange,
                    &self.bot.symbol,
                    self.bot.market_type.as_str(),
                )
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

    // ── LLM 决策 ────────────────────────────────────────────

    pub(crate) async fn on_llm_decision(&mut self) {
        if self.is_pending() {
            warn!(bot_id = %self.bot.id, "Pending order in progress, skipping LLM decision");
            return;
        }

        // 决策前直接查询 PE 仓位，刷新 current_position 缓存。
        // 防止 PE 事件丢失（broadcast lag）导致缓存为空 → 误判"无仓位" → 重复开仓。
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

        // 保存 log_id 供后续 apply_pending_open/apply_pending_close 回填 execution_status / intercept_reason
        self.current_log_id = log_id;

        // 执行决策，若被拦截则 UPDATE LLM log 设置 intercept_reason + execution_status
        let intercept_reason = self.execute_decision(&action, decision.as_ref()).await;
        if let Some(reason) = intercept_reason {
            warn!(bot_id = %self.bot.id, action = %action.as_str(), intercept_reason = %reason, "Decision intercepted");
            if let Some(log_id) = self.current_log_id {
                // 拦截时根据 action 类型设置 execution_status
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
            // 拦截后清空 log_id，避免被后续操作误更新
            self.current_log_id = None;
        } else if matches!(action, AutoAction::Hold) {
            // Hold 决策：回填 execution_status='hold'
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
                .get_market_snapshot(
                    &self.bot.exchange,
                    &self.bot.symbol,
                    self.bot.market_type.as_str(),
                )
                .await,
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

        // 构造最近平仓事件描述（用于 LLM 反思，避免反复扫损）
        // 基于 close_reason（代码逻辑字段）：stop_loss/take_profit/position_timeout/llm_decision
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
            market_type: self.bot.market_type.as_str().to_string(),
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

    /// 执行 LLM 决策。
    /// 返回 `Some(拦截原因)` 表示决策被代码拦截（未执行），`None` 表示已执行或无需执行。
    /// 拦截原因会被记录到 LLM log 的 intercept_reason 字段供前端展示。
    ///
    /// ## 拦截顺序（从早到晚，前序拦截后不再检查后续）
    /// 1. **Hold 决策**：LLM 决策为观望，直接返回（非拦截，无日志）
    /// 2. **pending 订单**：有待确认的订单在途，避免重复下单
    /// 3. **置信度不足**（仅开仓）：confidence < 0.6，降级为观望
    /// 4. **市场快照无效**：current_price <= 0，无法计算下单参数
    /// 5. **现货做空**：spot 市场不支持 short
    /// 6. **已有仓位**：避免重复开仓
    /// 7. **冷却期**（仅开仓）：止损/止盈/LLM平仓后同方向重入限制
    /// 8. **无仓位可平**（仅平仓）：ClosePosition 时无持仓
    ///
    /// 拦截 2-8 会返回 `Some(reason)`，由调用方记录到 LLM log 的 intercept_reason
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
                .get_market_snapshot(
                    &self.bot.exchange,
                    &self.bot.symbol,
                    self.bot.market_type.as_str(),
                )
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

                if side == "short" && self.is_spot() {
                    warn!(bot_id = %self.bot.id, "Cannot open short on spot market");
                    return Some("现货市场不支持做空".to_string());
                }
                if self.has_position() {
                    warn!(bot_id = %self.bot.id, side = %side, "Already has position, cannot open");
                    return Some("已有仓位，无法开仓".to_string());
                }
                // 冷却期检查：防止止损/止盈后立即同方向重入被反弹扫损
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
                // 订单发送失败检测：open_position 成功会设置 pending_open
                // 失败时回填 intercept_reason + execution_status='open_failed'（在 on_llm_decision 中处理）
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
                // LLM 决策平仓：close_reason 固定为 llm_decision（不由 LLM 决定原因文本）
                self.close_position("llm_decision").await;
                // 订单发送失败检测：close_position 成功会设置 pending_close
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

        // market_regime 是交易决策的关键参数，不得使用默认值。
        // 若 market_regime 为 None，跳过 AI 分析更新以避免写入错误的 regime。
        let regime = match self.bot.market_regime.as_deref() {
            Some(r) => r,
            None => {
                tracing::warn!(bot_id = %self.bot.id, "market_regime is None — skipping AI analysis update to avoid default value");
                return;
            }
        };

        // 杠杆由用户配置决定（create 接口），不在运行时动态调整
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
            .get_account_balance(&self.bot.exchange, self.bot.market_type.as_str())
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

        // 止损止盈价格来源策略：
        //   ① 优先采用 LLM 在决策中返回的 SL/TP（基于市场结构判断）
        //   ② LLM 未返回或方向不合法（多头 sl>=price / tp<=price，空头反之）时回退到代码公式
        //   ③ 公式兜底：1.5×ATR 止损，3.0×ATR 止盈
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

        // 盈亏比校验：若 < 1.0 则使用公式兜底（防止 LLM 给出不合理 SL/TP）
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

        let client_order_id = format!("auto:{}:{}", side, self.bot.id);

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

    /// 平仓入口。
    /// - `close_reason`: 平仓原因（stop_loss/take_profit/position_timeout/llm_decision）
    ///   由代码逻辑决定（不由 LLM 决定），用于冷却期判断和 DB 记录
    pub(crate) async fn close_position(&mut self, close_reason: &str) {
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
            let client_order_id = format!("auto:close:{}:{}", close_reason, self.bot.id);

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
                        unrealized_pnl,
                        client_order_id,
                        sent_at: tokio::time::Instant::now(),
                    });
                }
                Err(e) => {
                    error!(bot_id = %self.bot.id, error = %e, "Failed to send close position order");
                }
            }
        } else {
            // Fallback: use PlaceOrder with reduce_only when position_id is not available
            let (order_side, position_side) = match side.as_str() {
                "long" => (OrderSide::Sell, Some(BotPositionSide::Long)),
                "short" => (OrderSide::Buy, Some(BotPositionSide::Short)),
                _ => {
                    error!(bot_id = %self.bot.id, side = %side, "Unknown position side, cannot close");
                    return;
                }
            };

            let client_order_id = format!("auto:close:{}:{}", close_reason, self.bot.id);

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
                        close_reason = %close_reason,
                        "Position closing order sent via PlaceOrder, awaiting confirmation"
                    );

                    self.pending_close = Some(PendingClose {
                        side: side.clone(),
                        close_reason: close_reason.to_string(),
                        entry_price,
                        position_size,
                        unrealized_pnl,
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
                    _ => position.is_open(),
                };
                if !is_ours {
                    return;
                }
                // 如果 position_id 还没建立，从 PositionUpdated 事件中恢复
                // 这可以处理 PositionOpened 事件丢失（broadcast lag）的场景
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
                if let Some(pid) = self.bot.position_id {
                    if pid == position.id {
                        self.current_position = None;
                        // 清空 position_id 并持久化，避免重启后尝试恢复已关闭的仓位
                        self.bot.position_id = None;
                        if let Err(e) = self
                            .store
                            .update_position(self.bot.id, self.bot.position_id)
                            .await
                        {
                            warn!(bot_id = %self.bot.id, error = %e, "Failed to update position");
                        }
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

                // Track position_id from order events
                if order.position_id.is_some() && self.bot.position_id.is_none() {
                    self.bot.position_id = order.position_id;
                }

                let fill_price = order
                    .fill_price
                    .or(order.request_price)
                    .unwrap_or_else(|| {
                        // Using current_price as fallback for fill_price can cause
                        // PnL miscalculation if the price has moved since fill.
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
                    // 记录是开仓还是平仓失败（rollback 前判断）
                    let was_open = self.pending_open.is_some();
                    warn!(
                        bot_id = %self.bot.id,
                        reason = %reason,
                        was_open,
                        "Order failed, rolling back pending state"
                    );
                    self.rollback_pending_open();
                    self.rollback_pending_close();
                    // 回填 LLM log 执行状态（仅 LLM 决策触发的订单才有 log_id）
                    // 止盈止损/超时触发的平仓 current_log_id 为 None，不会误回填
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
                    .get_market_snapshot(
                        &self.bot.exchange,
                        &self.bot.symbol,
                        self.bot.market_type.as_str(),
                    )
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

        // 仓位实时状态由 PE 通过 PositionUpdated 事件维护，这里只更新内存中的风控参数
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
        let trade_side = match pending.side.as_str() {
            "long" => "buy",
            "short" => "sell",
            _ => {
                error!(side = %pending.side, "Unknown pending side — skipping trade record");
                return;
            }
        };
        // 开仓时 INSERT 一条 status='open' 的 trade 记录，保存 trade_id 和 open_fee
        // 同时记录本次交易的风控边界（SL/TP）到 trade 维度，便于审计与前端展示
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
                stop_loss,
                take_profit,
            )
            .await
        {
            Ok(trade_id) => {
                self.current_trade_id = Some(trade_id);
                info!(bot_id = %self.bot.id, trade_id = %trade_id, trade_type, stop_loss, take_profit, "Open trade recorded");
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

        // 回填 LLM 日志执行状态：开仓成功
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

        // 平仓 PnL = 价格差收益 - 开仓手续费 - 平仓手续费
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
            quantity = actual_qty, realized_pnl,
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

        // 记录最近平仓事件（用于冷却期判断和 LLM 上下文反思）
        // 字段：(开仓方向, 平仓原因 close_reason, 平仓时间)
        self.last_close_event = Some((
            pending.side.clone(),
            pending.close_reason.clone(),
            chrono::Utc::now(),
        ));

        self.save_position().await;
        self.save_stats().await;

        let close_side = match pending.side.as_str() {
            "long" => "sell",
            "short" => "buy",
            _ => {
                error!(side = %pending.side, "Unknown pending side — skipping trade record");
                return;
            }
        };
        // close_reason 直接使用 pending.close_reason（已为合法值）
        let close_reason = &pending.close_reason;

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
                    Ok(Some((tid, _sl, _tp, _opened_at))) => {
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
                        if let Err(e) = self
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

        // 重置开仓手续费缓存
        self.current_open_fee = 0.0;

        // 回填 LLM 日志执行状态：平仓成功
        // 注：close_reason 不回填到此表，已记录在 qd_auto_trades.close_reason
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
