use std::collections::HashSet;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::broadcast;
use tracing::{error, info, warn};
use uuid::Uuid;
use virs_types::client_order_id;

use crate::grid::ai::{GridAction, GridAiDecision, GridAiService};
use crate::grid::ports::*;
use crate::grid::types::{GridEvent, GridLevel, GridState};
use crate::grid::utils;
use crate::strategy::prompt::{PromptLoader, StrategyType};

pub enum OrderDir {
    Buy,
    Sell,
}

pub struct GridWorker {
    pub(crate) bot: GridBotConfig,
    price_provider: Arc<dyn PriceProvider>,
    order_executor: Arc<dyn OrderExecutor>,
    ai_service: Arc<GridAiService>,
    store: Arc<dyn GridStore>,
    market_data_provider: Arc<dyn MarketDataProvider>,
    event_rx: broadcast::Receiver<OrderEvent>,
    grid_event_tx: broadcast::Sender<GridEvent>,
    pub(crate) levels: Vec<GridLevel>,
    pub(crate) current_price: f64,
    pub(crate) total_pnl: f64,
    pub(crate) total_trades: i32,
    pub(crate) grid_filled_count: i32,
    pub(crate) consecutive_losses: i32,
    pub(crate) paused: bool,
    initial_order_range: usize,
    pending_orders: HashSet<(usize, String)>,

    pub(crate) time_config: virs_config::TimeConfig,
    pub(crate) prompt_loader: PromptLoader,
}

impl GridWorker {
    pub fn new(
        bot: GridBotConfig,
        price_provider: Arc<dyn PriceProvider>,
        order_executor: Arc<dyn OrderExecutor>,
        ai_service: Arc<GridAiService>,
        store: Arc<dyn GridStore>,
        market_data_provider: Arc<dyn MarketDataProvider>,
        event_rx: broadcast::Receiver<OrderEvent>,
        grid_event_tx: broadcast::Sender<GridEvent>,
        time_config: virs_config::TimeConfig,
        prompt_loader: PromptLoader,
    ) -> Self {
        let levels = utils::calculate_levels(&bot, 0.0);
        Self {
            bot,
            price_provider,
            order_executor,
            ai_service,
            store,
            market_data_provider,
            event_rx,
            grid_event_tx,
            levels,
            current_price: 0.0,
            total_pnl: 0.0,
            total_trades: 0,
            grid_filled_count: 0,
            consecutive_losses: 0,
            paused: false,
            initial_order_range: 3,
            pending_orders: HashSet::new(),
            time_config,
            prompt_loader,
        }
    }

    pub async fn run(
        &mut self,
        mut shutdown_rx: tokio::sync::mpsc::Receiver<()>,
        mut adjust_rx: tokio::sync::mpsc::Receiver<()>,
    ) {
        if self.levels.is_empty() {
            if self.bot.dynamic_adjust {
                warn!(bot_id = %self.bot.id, "No grid levels yet, will trigger initial LLM analysis after price fetch");
            } else {
                error!(bot_id = %self.bot.id, "No grid levels calculated and dynamic_adjust is disabled");
                return;
            }
        }

        let max_retries = self.time_config.retry.initial_price_max_retries;
        for attempt in 1..=max_retries {
            self.current_price = self.fetch_current_price().await;
            if self.current_price > 0.0 {
                break;
            }
            warn!(bot_id = %self.bot.id, attempt, "Failed to fetch initial price, retrying...");
            tokio::time::sleep(std::time::Duration::from_secs(
                self.time_config.price_poll_interval_secs,
            ))
            .await;
        }
        if self.current_price <= 0.0 {
            error!(bot_id = %self.bot.id, "Failed to fetch initial price after {} attempts", max_retries);
        }

        self.load_existing_trades().await;

        if self.bot.upper_price <= 0.0 || self.bot.lower_price <= 0.0 || self.levels.is_empty() {
            self.on_llm_decision().await;
        }

        if !self.levels.is_empty() {
            self.place_initial_orders().await;
        }

        let mut price_tick = tokio::time::interval(std::time::Duration::from_secs(
            self.time_config.price_poll_interval_secs,
        ));

        let (llm_signal_tx, mut llm_signal_rx) = tokio::sync::mpsc::channel::<()>(1);
        if self.bot.dynamic_adjust {
            let interval_secs = self.bot.adjust_interval_secs.max(60) as u64;
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
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
                Some(()) = adjust_rx.recv() => {
                    self.on_adjust_signal().await;
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
                            warn!(bot_id = %self.bot.id, lagged = n, "Event lagged, clearing pending orders");
                            self.clear_pending_orders();
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            warn!(bot_id = %self.bot.id, "Event channel closed");
                            break;
                        }
                    }
                }
            }
        }

        self.save_stats().await;
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

    pub(crate) async fn on_price_tick(&mut self) {
        if self.current_price <= 0.0 {
            return;
        }

        let levels = self.filter_levels(|l| {
            l.side == "buy"
                && self.current_price < l.buy_price
                && !l.buy_filled
                && l.buy_order_id.is_none()
        });
        self.place_orders_for_levels(&levels, OrderDir::Buy).await;

        let levels = self.filter_levels(|l| {
            l.side == "sell"
                && l.sell_price > self.current_price
                && !l.sell_filled
                && l.sell_order_id.is_none()
        });
        self.place_orders_for_levels(&levels, OrderDir::Sell).await;

        let levels = self.filter_levels(|l| {
            l.side == "buy"
                && l.hold_quantity > 0.0
                && self.current_price >= l.sell_price
                && l.sell_order_id.is_none()
        });
        self.place_orders_for_levels(&levels, OrderDir::Sell).await;

        let levels = self.filter_levels(|l| {
            l.side == "sell"
                && l.hold_quantity < 0.0
                && self.current_price <= l.buy_price
                && l.buy_order_id.is_none()
        });
        self.place_orders_for_levels(&levels, OrderDir::Buy).await;

        self.broadcast_state();
    }

    pub(crate) async fn place_initial_orders(&mut self) {
        if self.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "No current price, skipping initial orders");
            return;
        }

        let current_level_idx = self.find_level_by_price(self.current_price);
        let range = self.initial_order_range;

        let levels: Vec<GridLevel> = self
            .levels
            .iter()
            .enumerate()
            .filter(|(i, l)| {
                l.side == "buy"
                    && l.buy_price < self.current_price
                    && !l.buy_filled
                    && l.buy_order_id.is_none()
                    && (*i as i32 - current_level_idx as i32).abs() <= range as i32
            })
            .map(|(_, l)| l.clone())
            .collect();
        self.place_orders_for_levels(&levels, OrderDir::Buy).await;

        let levels: Vec<GridLevel> = self
            .levels
            .iter()
            .enumerate()
            .filter(|(i, l)| {
                l.side == "sell"
                    && l.sell_price > self.current_price
                    && !l.sell_filled
                    && l.sell_order_id.is_none()
                    && (*i as i32 - current_level_idx as i32).abs() <= range as i32
            })
            .map(|(_, l)| l.clone())
            .collect();
        self.place_orders_for_levels(&levels, OrderDir::Sell).await;

        let close_levels: Vec<GridLevel> = self
            .levels
            .iter()
            .filter(|l| {
                l.hold_quantity.abs() > 0.0
                    && ((l.side == "buy" && l.hold_quantity > 0.0 && l.sell_order_id.is_none())
                        || (l.side == "sell" && l.hold_quantity < 0.0 && l.buy_order_id.is_none()))
            })
            .cloned()
            .collect();

        for level in &close_levels {
            let dir = if level.side == "buy" {
                OrderDir::Sell
            } else {
                OrderDir::Buy
            };
            self.place_order(level, &dir).await;
        }
    }

    fn filter_levels(&self, predicate: impl Fn(&GridLevel) -> bool) -> Vec<GridLevel> {
        self.levels
            .iter()
            .filter(|l| predicate(l))
            .cloned()
            .collect()
    }

    async fn place_orders_for_levels(&mut self, levels: &[GridLevel], dir: OrderDir) {
        for level in levels {
            self.place_order(level, &dir).await;
        }
    }

    pub(crate) async fn place_order(&mut self, level: &GridLevel, dir: &OrderDir) {
        let (side, price, key_side) = match dir {
            OrderDir::Buy => (OrderSide::Buy, level.buy_price, "buy"),
            OrderDir::Sell => (OrderSide::Sell, level.sell_price, "sell"),
        };

        let key = (level.level as usize, key_side.to_string());
        if self.pending_orders.contains(&key) {
            return;
        }

        let (amount, is_close, position_side) = match (dir, level.side.as_str()) {
            (OrderDir::Buy, "sell") => (
                level.hold_quantity.abs().min(level.quantity),
                true,
                BotPositionSide::Short,
            ),
            (OrderDir::Buy, _) => (level.quantity, false, BotPositionSide::Long),
            (OrderDir::Sell, "sell") => (level.quantity, false, BotPositionSide::Short),
            (OrderDir::Sell, _) => (
                level.hold_quantity.min(level.quantity),
                true,
                BotPositionSide::Long,
            ),
        };

        let position_side_str = if level.side == "sell" {
            "short"
        } else {
            "long"
        };
        let client_order_id = Some(client_order_id::format_grid_order(
            self.bot.id,
            level.level,
            !is_close,
            position_side_str,
        ));

        let cmd = if is_close {
            OrderCommand::PlaceOrder {
                symbol: self.bot.symbol.clone(),
                side,
                amount,
                price: Some(price),
                position_side: Some(position_side),
                position_id: None,
                client_order_id,
            }
        } else {
            OrderCommand::OpenPosition {
                symbol: self.bot.symbol.clone(),
                side: position_side,
                order_side: side,
                amount,
                leverage: self.bot.leverage.max(1) as u32,
                price: Some(price),
                client_order_id,
            }
        };

        if let Err(e) = self.order_executor.send_command(cmd).await {
            error!(bot_id = %self.bot.id, level = level.level, side = key_side, error = %e, "Failed to send order");
        } else {
            self.pending_orders.insert(key);
        }
    }

    pub(crate) async fn place_buy_order(&mut self, level: &GridLevel) {
        self.place_order(level, &OrderDir::Buy).await;
    }

    pub(crate) async fn place_sell_order(&mut self, level: &GridLevel) {
        self.place_order(level, &OrderDir::Sell).await;
    }

    pub(crate) async fn on_order_event(&mut self, event: OrderEvent) {
        match event {
            OrderEvent::OrderPlaced { order } => {
                if order.symbol != self.bot.symbol {
                    return;
                }
                self.on_order_placed(&order).await;
            }
            OrderEvent::OrderFilled { order } => {
                if order.symbol != self.bot.symbol {
                    return;
                }
                self.on_order_filled(&order).await;
            }
            OrderEvent::OrderCanceled { order_id, symbol } => {
                if let Some(ref sym) = symbol {
                    if sym != &self.bot.symbol {
                        return;
                    }
                }
                self.on_order_canceled(order_id).await;
            }
            OrderEvent::OrderFailed { order_id, reason } => {
                warn!(bot_id = %self.bot.id, order_id = %order_id, reason = %reason, "Order failed");
                self.clear_order_id(order_id);
            }
            OrderEvent::RiskAlert { level, message } => {
                warn!(bot_id = %self.bot.id, level = %level, message = %message, "Risk alert");
                if level == "CloseAll" {
                    self.pause_with_cancel("CloseAll risk alert").await;
                }
            }
        }
    }

    async fn on_order_placed(&mut self, order: &OrderInfo) {
        if let Some(ref coi) = order.client_order_id {
            if let Some((level_idx, side)) = Self::parse_client_order_id(coi) {
                if level_idx < self.levels.len() {
                    self.pending_orders.remove(&(level_idx, side.clone()));
                    if side == "buy" {
                        self.levels[level_idx].buy_order_id = Some(order.id);
                    } else {
                        self.levels[level_idx].sell_order_id = Some(order.id);
                    }
                }
            }
        }
    }

    fn parse_client_order_id(coi: &str) -> Option<(usize, String)> {
        client_order_id::parse_grid_order(coi)
    }

    async fn on_order_filled(&mut self, order: &OrderInfo) {
        let side_str = order.side.as_str();

        let idx = match self.find_level_by_order_id(order.id) {
            Some((i, ref side)) if side == side_str => i,
            _ => {
                warn!(bot_id = %self.bot.id, order_id = %order.id, side = %side_str, "Order filled but not matched to any grid level");
                return;
            }
        };

        let price = order.fill_price.unwrap_or_else(|| {
            error!(bot_id = %self.bot.id, order_id = %order.id, "Order filled but no fill_price — falling back to current_price (PnL may be inaccurate)");
            self.current_price
        });
        let level_side = self.levels[idx].side.clone();
        let level_num = self.levels[idx].level;
        let entry_price = self.levels[idx].avg_buy_price;

        let is_open = !is_close_trade(&level_side, side_str);

        apply_fill_to_level(&mut self.levels[idx], side_str, price, order.filled);

        self.pending_orders.remove(&(idx, side_str.to_string()));

        let pnl = calculate_fill_pnl(&level_side, side_str, entry_price, price, order.filled);
        let hold = self.levels[idx].hold_quantity;
        self.total_pnl += pnl;
        self.total_trades += 1;
        if is_close_trade(&level_side, side_str) {
            self.grid_filled_count += 1;
        }
        self.update_consecutive_losses(pnl);

        if is_open {
            let client_order_id = order.client_order_id.as_deref().unwrap_or("unknown");
            if self.record_open_trade(level_num, client_order_id).await {
                self.levels[idx].open_client_order_id = Some(client_order_id.to_string());
            }
        } else {
            let close_client_order_id = order.client_order_id.as_deref().unwrap_or("unknown");
            let open_client_order_id = if let Some(ref oid) = self.levels[idx].open_client_order_id
            {
                Some(oid.clone())
            } else {
                self.store
                    .find_open_trade(self.bot.id, level_num)
                    .await
                    .ok()
                    .flatten()
            };
            if let Some(open_oid) = open_client_order_id {
                self.record_close_trade(open_oid, close_client_order_id, level_num)
                    .await;
            } else {
                warn!(bot_id = %self.bot.id, level = level_num, side = %side_str, price, quantity = order.filled, pnl, "No open trade found for close, recording as orphaned");
                if let Err(e) = self
                    .store
                    .record_orphaned_close_trade(
                        self.bot.id,
                        self.bot.user_id,
                        &self.bot.symbol,
                        &self.bot.exchange,
                        level_num,
                        close_client_order_id,
                    )
                    .await
                {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to record orphaned close trade");
                }
            }
            self.levels[idx].open_client_order_id = None;
        }

        self.place_reverse_order_if_cycle_complete(idx, &level_side, side_str, hold)
            .await;

        if is_close_trade(&level_side, side_str) {
            if let Err(e) = self.grid_event_tx.send(GridEvent::GridTradeClosed {
                bot_id: self.bot.id,
                level: level_num,
                pnl,
            }) {
                tracing::warn!(error = %e, event = "GridTradeClosed", "Failed to send event — receiver may be dropped");
            }
        }
        if let Err(e) = self.grid_event_tx.send(GridEvent::GridFilled {
            bot_id: self.bot.id,
            level: level_num,
            side: side_str.to_string(),
            price,
            quantity: order.filled,
        }) {
            tracing::warn!(error = %e, event = "GridFilled", "Failed to send event — receiver may be dropped");
        }

        self.save_stats().await;
    }

    async fn place_reverse_order_if_cycle_complete(
        &mut self,
        idx: usize,
        level_side: &str,
        trade_side: &str,
        hold: f64,
    ) {
        let cycle_complete = if level_side == "buy" {
            is_close_trade(level_side, trade_side) && hold <= 0.0
        } else {
            is_close_trade(level_side, trade_side) && hold >= 0.0
        };

        if !cycle_complete {
            return;
        }

        let reset_level = self.levels[idx].reset_for_relist();
        self.levels[idx] = reset_level.clone();
        if level_side == "buy" {
            self.place_buy_order(&reset_level).await;
        } else {
            self.place_sell_order(&reset_level).await;
        }
    }

    async fn on_order_canceled(&mut self, order_id: Uuid) {
        self.clear_order_id(order_id);
    }

    pub(crate) fn clear_order_id(&mut self, order_id: Uuid) {
        if let Some((idx, side)) = self.find_level_by_order_id(order_id) {
            if side == "buy" {
                self.levels[idx].buy_order_id = None;
            } else {
                self.levels[idx].sell_order_id = None;
            }
            self.pending_orders.remove(&(idx, side));
        }
    }

    pub(crate) fn clear_pending_orders(&mut self) {
        self.pending_orders.clear();
        for level in &mut self.levels {
            level.buy_order_id = None;
            level.sell_order_id = None;
        }
    }

    pub(crate) async fn pause_with_cancel(&mut self, reason: &str) {
        if !self.paused {
            self.paused = true;
            if let Err(e) = self
                .order_executor
                .send_command(OrderCommand::CancelAllOrders {
                    symbol: Some(self.bot.symbol.clone()),
                })
                .await
            {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to send CancelAllOrders command");
            }
            warn!(bot_id = %self.bot.id, "Grid paused due to {}", reason);
        }
    }

    pub(crate) async fn load_existing_trades(&mut self) {
        let mut trades = match self.store.load_trades(self.bot.id).await {
            Ok(t) => t,
            Err(e) => {
                error!(
                    bot_id = %self.bot.id,
                    error = %e,
                    "Failed to load existing grid trades, skipping restore"
                );
                return;
            }
        };
        let max_dist = self.grid_spacing();
        trades.sort_by_key(|t| t.opened_at);

        for trade in &trades {
            if let Some(level_idx) = self.find_level_by_price_within(trade.open_price, max_dist) {
                apply_fill_to_level(
                    &mut self.levels[level_idx],
                    &trade.open_side,
                    trade.open_price,
                    trade.open_quantity,
                );
                if let (Some(close_side), Some(close_price), Some(close_qty)) =
                    (&trade.close_side, trade.close_price, trade.close_quantity)
                {
                    apply_fill_to_level(
                        &mut self.levels[level_idx],
                        close_side,
                        close_price,
                        close_qty,
                    );
                }
            } else {
                warn!(bot_id = %self.bot.id, trade_price = trade.open_price, grid_level = trade.grid_level, "Trade could not be matched to any grid level");
            }
            self.total_pnl += trade.pnl;
            self.total_trades += 1;
            if trade.close_side.is_some() {
                self.total_trades += 1;
                self.grid_filled_count += 1;
            }
        }

        for pnl in trades.iter().rev().filter_map(|t| {
            if t.close_side.is_some() {
                Some(t.pnl)
            } else {
                None
            }
        }) {
            self.update_consecutive_losses(pnl);
        }

        for trade in &trades {
            if trade.close_side.is_none() {
                if let Some(level_idx) =
                    self.levels.iter().position(|l| l.level == trade.grid_level)
                {
                    self.levels[level_idx].open_client_order_id =
                        Some(trade.open_client_order_id.clone());
                }
            }
        }

        self.reset_completed_cycles();
    }

    fn reset_completed_cycles(&mut self) {
        for level in &mut self.levels {
            let cycle_complete = if level.side == "buy" {
                level.buy_filled && level.sell_filled && level.hold_quantity <= 0.0
            } else {
                level.sell_filled && level.buy_filled && level.hold_quantity >= 0.0
            };
            if cycle_complete {
                level.buy_filled = false;
                level.sell_filled = false;
                level.buy_order_id = None;
                level.sell_order_id = None;
                level.hold_quantity = 0.0;
                level.avg_buy_price = 0.0;
            }
        }
    }

    pub(crate) fn grid_spacing(&self) -> f64 {
        if self.levels.len() > 1 {
            (self.bot.upper_price - self.bot.lower_price) / self.levels.len() as f64
        } else {
            0.0
        }
    }

    pub(crate) fn update_consecutive_losses(&mut self, pnl: f64) {
        if pnl < 0.0 {
            self.consecutive_losses += 1;
        } else if pnl > 0.0 {
            self.consecutive_losses = 0;
        }
    }

    pub(crate) fn find_level_by_price(&self, price: f64) -> usize {
        let mut closest = 0;
        let mut min_diff = f64::MAX;
        for (i, level) in self.levels.iter().enumerate() {
            let diff = (price - level.price).abs();
            if diff < min_diff {
                min_diff = diff;
                closest = i;
            }
        }
        closest
    }

    pub(crate) fn find_level_by_price_within(&self, price: f64, max_dist: f64) -> Option<usize> {
        if price.is_nan() || price.is_infinite() {
            tracing::error!(price, "Price is NaN or infinite — cannot find grid level");
            return None;
        }
        let (idx, dist) = self
            .levels
            .iter()
            .enumerate()
            .map(|(i, l)| (i, (l.price - price).abs()))
            .min_by(|a, b| {
                if a.1.is_nan() || b.1.is_nan() {
                    tracing::warn!(
                        "NaN detected in grid level price comparison — treating as equal"
                    );
                    std::cmp::Ordering::Equal
                } else {
                    a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                }
            })?;
        if max_dist <= 0.0 || dist <= max_dist {
            Some(idx)
        } else {
            None
        }
    }

    pub(crate) fn find_level_by_order_id(&self, order_id: Uuid) -> Option<(usize, String)> {
        for (idx, level) in self.levels.iter().enumerate() {
            if level.buy_order_id == Some(order_id) {
                return Some((idx, "buy".to_string()));
            }
            if level.sell_order_id == Some(order_id) {
                return Some((idx, "sell".to_string()));
            }
        }
        None
    }

    pub(crate) fn compute_unrealized_pnl(&self) -> f64 {
        if self.current_price <= 0.0 {
            return 0.0;
        }
        self.levels
            .iter()
            .filter(|l| l.hold_quantity.abs() > 0.0 && l.avg_buy_price > 0.0)
            .map(|l| {
                if l.hold_quantity > 0.0 {
                    (self.current_price - l.avg_buy_price) * l.hold_quantity
                } else {
                    (l.avg_buy_price - self.current_price) * l.hold_quantity.abs()
                }
            })
            .sum()
    }

    pub(crate) async fn save_stats(&self) {
        let levels_json = serde_json::to_value(&self.levels).ok();
        if let Err(e) = self
            .store
            .save_stats(
                self.bot.id,
                self.total_pnl,
                self.compute_unrealized_pnl(),
                self.total_trades,
                self.grid_filled_count,
                levels_json.as_ref(),
            )
            .await
        {
            error!(bot_id = %self.bot.id, error = %e, "save_stats failed");
        }
    }

    pub(crate) fn broadcast_state(&self) {
        let state = GridState {
            bot_id: self.bot.id,
            symbol: self.bot.symbol.clone(),
            exchange: self.bot.exchange.clone(),
            levels: self.levels.clone(),
            current_price: self.current_price,
            total_pnl: self.total_pnl,
            unrealized_pnl: self.compute_unrealized_pnl(),
            total_trades: self.total_trades,
            grid_filled_count: self.grid_filled_count,
            last_tick_at: Utc::now(),
        };
        if let Err(e) = self.grid_event_tx.send(GridEvent::StatusUpdate {
            bot_id: self.bot.id,
            state,
        }) {
            tracing::warn!(error = %e, event = "StatusUpdate", "Failed to send event — receiver may be dropped");
        }
    }

    async fn record_open_trade(&self, level: i32, client_order_id: &str) -> bool {
        match self
            .store
            .record_open_trade(
                self.bot.id,
                self.bot.user_id,
                &self.bot.symbol,
                &self.bot.exchange,
                level,
                client_order_id,
            )
            .await
        {
            Ok(()) => {
                info!(bot_id = %self.bot.id, level, client_order_id, "Open trade recorded");
                true
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, level, error = %e, "Failed to record open trade");
                false
            }
        }
    }

    async fn record_close_trade(
        &self,
        open_client_order_id: String,
        close_client_order_id: &str,
        level: i32,
    ) {
        if let Err(e) = self
            .store
            .close_trade(&open_client_order_id, close_client_order_id)
            .await
        {
            warn!(bot_id = %self.bot.id, level, open_client_order_id = %open_client_order_id, error = %e, "Failed to close trade record");
        } else {
            info!(bot_id = %self.bot.id, level, open_client_order_id = %open_client_order_id, "Close trade recorded");
        }
    }

    pub(crate) async fn on_llm_decision(&mut self) {
        let is_initial =
            self.bot.upper_price <= 0.0 || self.bot.lower_price <= 0.0 || self.levels.is_empty();

        let (system_prompt, user_prompt) = match self.build_llm_prompt().await {
            Some(p) => p,
            None => return,
        };

        let decision_result = self
            .ai_service
            .analyze(&self.bot, &system_prompt, &user_prompt)
            .await;
        let (decision, raw_llm_response, llm_model) = match decision_result {
            Ok((d, m)) => (Some(d), None, m),
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "LLM call failed");
                (None, None, String::new())
            }
        };

        let action = self
            .handle_llm_result(
                &decision,
                &system_prompt,
                &user_prompt,
                raw_llm_response.as_ref(),
                is_initial,
                &llm_model,
            )
            .await;
        self.execute_decision(&action, decision.as_ref()).await;
        if !matches!(action, GridAction::Hold) {
            if let Err(e) = self.store.update_last_adjusted(self.bot.id).await {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to update last adjusted");
            }
        }
    }

    async fn build_llm_prompt(&self) -> Option<(String, String)> {
        let snapshot = self
            .market_data_provider
            .get_market_snapshot(&self.bot.exchange, &self.bot.symbol)
            .await;
        if snapshot.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "Market snapshot has zero price, skipping LLM decision");
            return None;
        }

        let grid_status = if self.paused {
            "paused"
        } else if self.levels.is_empty() {
            "empty"
        } else {
            "running"
        };

        let current_price = self.current_price;
        let long_qty: f64 = self
            .levels
            .iter()
            .filter(|l| l.hold_quantity > 0.0)
            .map(|l| l.hold_quantity)
            .sum();
        let short_qty: f64 = self
            .levels
            .iter()
            .filter(|l| l.hold_quantity < 0.0)
            .map(|l| l.hold_quantity.abs())
            .sum();
        let long_cost: f64 = self
            .levels
            .iter()
            .filter(|l| l.hold_quantity > 0.0 && l.avg_buy_price > 0.0)
            .map(|l| l.avg_buy_price * l.hold_quantity)
            .sum();
        let short_cost: f64 = self
            .levels
            .iter()
            .filter(|l| l.hold_quantity < 0.0 && l.avg_buy_price > 0.0)
            .map(|l| l.avg_buy_price * l.hold_quantity.abs())
            .sum();
        let long_avg = if long_qty > 0.0 {
            long_cost / long_qty
        } else {
            0.0
        };
        let short_avg = if short_qty > 0.0 {
            short_cost / short_qty
        } else {
            0.0
        };
        let long_pnl = if long_qty > 0.0 && current_price > 0.0 {
            (current_price - long_avg) * long_qty
        } else {
            0.0
        };
        let short_pnl = if short_qty > 0.0 && current_price > 0.0 {
            (short_avg - current_price) * short_qty
        } else {
            0.0
        };

        let position_info = if long_qty <= 0.0 && short_qty <= 0.0 {
            "none".to_string()
        } else {
            let mut s = String::new();
            if long_qty > 0.0 {
                s.push_str(&format!(
                    "- Long: 币数 {:.6}, 均价 {:.2}, 未实现盈亏 {:.2} USDT",
                    long_qty, long_avg, long_pnl
                ));
            }
            if short_qty > 0.0 {
                if !s.is_empty() {
                    s.push('\n');
                }
                s.push_str(&format!(
                    "- Short: 币数 {:.6}, 均价 {:.2}, 未实现盈亏 {:.2} USDT",
                    short_qty, short_avg, short_pnl
                ));
            }
            s
        };

        let current_grid_config = format_grid_config(
            grid_status,
            self.bot.upper_price,
            self.bot.lower_price,
            self.bot.grid_count,
            self.bot.grid_profit_pct,
            self.bot.quantity_per_grid,
            &self.levels,
        );

        let account = self
            .market_data_provider
            .get_account_balance(&self.bot.exchange)
            .await;

        let indicators: crate::common::indicators::MarketIndicators =
            serde_json::from_value(snapshot.indicators_json.clone()).unwrap_or_else(|e| {
                tracing::warn!(
                    error = %e,
                    "Failed to deserialize indicators_json for grid LLM — using all-zero defaults"
                );
                crate::common::indicators::MarketIndicators::default()
            });

        // 优先使用策略文件（STRATEGIES_DIR/grid/{strategy_file}.json）。
        // 未配置或未找到时回退到 crate 内硬编码的 DEFAULT_* 常量。
        let (system_prompt, user_prompt) =
            if let Some(file_name) = self.bot.strategy_file.as_deref() {
                match self
                    .prompt_loader
                    .get(StrategyType::Grid, file_name)
                    .await
                {
                    Some(tpl) => {
                        let user = utils::prompt::render_user_prompt(
                            &tpl.user_prompt_template,
                            &indicators,
                            account.total,
                            account.free,
                            account.used,
                            self.bot.leverage,
                            grid_status,
                            &self
                                .bot
                                .last_adjusted_at
                                .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                                .unwrap_or_else(|| "N/A".to_string()),
                            self.consecutive_losses,
                            &current_grid_config,
                            &position_info,
                            false,
                            "",
                            "scheduled_15m",
                        );
                        let system = self
                            .bot
                            .system_prompt
                            .as_deref()
                            .unwrap_or(&tpl.system_prompt)
                            .to_string();
                        (system, user)
                    }
                    None => {
                        warn!(
                            bot_id = %self.bot.id,
                            strategy_file = file_name,
                            "Strategy file not found in loader — falling back to built-in default prompt"
                        );
                        let user = utils::prompt::render_user_prompt(
                            crate::grid::types::DEFAULT_USER_PROMPT_TEMPLATE,
                            &indicators,
                            account.total,
                            account.free,
                            account.used,
                            self.bot.leverage,
                            grid_status,
                            &self
                                .bot
                                .last_adjusted_at
                                .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                                .unwrap_or_else(|| "N/A".to_string()),
                            self.consecutive_losses,
                            &current_grid_config,
                            &position_info,
                            false,
                            "",
                            "scheduled_15m",
                        );
                        let system = self
                            .bot
                            .system_prompt
                            .as_deref()
                            .unwrap_or(crate::grid::types::DEFAULT_SYSTEM_PROMPT)
                            .to_string();
                        (system, user)
                    }
                }
            } else {
                let user = utils::prompt::render_user_prompt(
                    crate::grid::types::DEFAULT_USER_PROMPT_TEMPLATE,
                    &indicators,
                    account.total,
                    account.free,
                    account.used,
                    self.bot.leverage,
                    grid_status,
                    &self
                        .bot
                        .last_adjusted_at
                        .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                        .unwrap_or_else(|| "N/A".to_string()),
                    self.consecutive_losses,
                    &current_grid_config,
                    &position_info,
                    false,
                    "",
                    "scheduled_15m",
                );
                let system = self
                    .bot
                    .system_prompt
                    .as_deref()
                    .unwrap_or(crate::grid::types::DEFAULT_SYSTEM_PROMPT)
                    .to_string();
                (system, user)
            };
        Some((system_prompt, user_prompt))
    }

    async fn handle_llm_result(
        &mut self,
        decision: &Option<GridAiDecision>,
        system_prompt: &str,
        user_prompt: &str,
        _raw_llm_response: Option<&serde_json::Value>,
        is_initial: bool,
        llm_model: &str,
    ) -> GridAction {
        match decision {
            Some(d) => {
                let result = serde_json::json!({
                    "decision": { "action": d.action, "reason": d.reason, "confidence": d.confidence },
                    "grid": { "upper_price": d.upper_price, "lower_price": d.lower_price, "grid_count": d.grid_count, "grid_profit_pct": d.grid_profit_pct },
                    "risk": { "quantity_per_grid": d.quantity_per_grid },
                    "market": { "market_regime": d.market_regime },
                    "analysis": d.analysis,
                    "risk_warning": d.risk_warning,
                });
                if let Err(e) = self
                    .store
                    .save_analysis_log(
                        self.bot.id,
                        if is_initial { "initial" } else { "periodic" },
                        system_prompt,
                        user_prompt,
                        &result,
                        None,
                        llm_model,
                    )
                    .await
                {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to save analysis log");
                }

                GridAction::from_str(&d.action, d.upper_price, d.lower_price)
            }
            None => {
                let rule_action = self.simple_rule_decision();
                warn!(bot_id = %self.bot.id, action = rule_action.as_str(), source = "rule_fallback", "LLM call failed, falling back to rule-based decision");

                let result = serde_json::json!({ "action": rule_action.as_str(), "reason": "LLM call failed, using rule-based fallback" });
                if let Err(e) = self
                    .store
                    .save_analysis_log(
                        self.bot.id,
                        if is_initial { "initial" } else { "periodic" },
                        system_prompt,
                        user_prompt,
                        &result,
                        Some("LLM call failed"),
                        llm_model,
                    )
                    .await
                {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to save analysis log");
                }

                if let Err(e) = self.grid_event_tx.send(GridEvent::BotError {
                    bot_id: self.bot.id,
                    error: "LLM call failed, using rule-based fallback".to_string(),
                }) {
                    tracing::warn!(error = %e, event = "BotError", "Failed to send event — receiver may be dropped");
                }
                rule_action
            }
        }
    }

    pub(crate) async fn execute_decision(
        &mut self,
        action: &GridAction,
        decision: Option<&GridAiDecision>,
    ) {
        if matches!(action, GridAction::Hold) {
            return;
        }

        let needs_params = self.bot.upper_price <= 0.0 || self.bot.lower_price <= 0.0;
        let allow_structure_change =
            needs_params || matches!(action, GridAction::AdjustGrid { .. });

        let mut structure_changed = false;
        if let Some(d) = decision {
            structure_changed = self
                .apply_llm_params(d, needs_params, allow_structure_change)
                .await;
        }

        match action {
            GridAction::PauseGrid => {
                self.pause_with_cancel("LLM decision").await;
            }
            GridAction::RunGrid => {
                if self.paused {
                    self.paused = false;
                    self.save_stats().await;
                    self.place_initial_orders().await;
                }
            }
            GridAction::ReducePosition => {
                let new_qty = (self.bot.quantity_per_grid * 0.5).max(1.0);
                self.bot.quantity_per_grid = new_qty;
                if let Err(e) = self
                    .store
                    .update_quantity_per_grid(self.bot.id, new_qty)
                    .await
                {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to update quantity per grid");
                }
                if let Err(e) = self
                    .order_executor
                    .send_command(OrderCommand::CancelAllOrders {
                        symbol: Some(self.bot.symbol.clone()),
                    })
                    .await
                {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to send CancelAllOrders command");
                }
                self.recalculate_levels();
                self.save_stats().await;
                if !self.paused {
                    self.place_initial_orders().await;
                }
                warn!(bot_id = %self.bot.id, quantity_per_grid = self.bot.quantity_per_grid, "Position reduced by decision");
            }
            GridAction::AdjustGrid {
                upper_price,
                lower_price,
            } => {
                if needs_params {
                    if !self.levels.is_empty() && !self.paused {
                        self.place_initial_orders().await;
                    }
                } else {
                    self.adjust_grid(Some(*upper_price), Some(*lower_price), structure_changed)
                        .await;
                }
            }
            GridAction::Hold => {
                warn!(bot_id = %self.bot.id, "Hold action reached execute_action, skipping");
            }
        }
    }

    async fn apply_llm_params(
        &mut self,
        d: &GridAiDecision,
        needs_params: bool,
        allow_structure_change: bool,
    ) -> bool {
        let mut structure_changed = false;

        if !d.market_regime.is_empty() {
            self.bot.market_regime = Some(d.market_regime.clone());
        }
        if d.quantity_per_grid > 0.0 {
            self.bot.quantity_per_grid = d.quantity_per_grid.max(1.0);
        }

        if allow_structure_change {
            if d.grid_count > 0 && d.grid_count != self.bot.grid_count {
                self.bot.grid_count = d.grid_count;
                structure_changed = true;
            }
            if d.grid_profit_pct > 0.0
                && (d.grid_profit_pct - self.bot.grid_profit_pct).abs() > f64::EPSILON
            {
                self.bot.grid_profit_pct = d.grid_profit_pct;
                structure_changed = true;
            }
            if needs_params {
                if d.upper_price > 0.0 {
                    self.bot.upper_price = d.upper_price;
                    structure_changed = true;
                }
                if d.lower_price > 0.0 {
                    self.bot.lower_price = d.lower_price;
                    structure_changed = true;
                }
                if self.bot.upper_price > 0.0 && self.bot.lower_price > 0.0 {
                    if let Err(e) = self
                        .store
                        .update_grid_params(self.bot.id, self.bot.upper_price, self.bot.lower_price)
                        .await
                    {
                        warn!(bot_id = %self.bot.id, error = %e, "Failed to update grid params");
                    }
                }
            }
        }

        if structure_changed {
            self.recalculate_levels();
        }

        if let Err(e) = self
            .store
            .update_ai_analysis(
                self.bot.id,
                self.bot.market_regime.as_deref().unwrap_or("ranging"),
                self.bot.upper_price,
                self.bot.lower_price,
                self.bot.grid_count,
                self.bot.grid_profit_pct,
                self.bot.quantity_per_grid,
                self.bot.leverage,
                &d.analysis,
            )
            .await
        {
            warn!(bot_id = %self.bot.id, error = %e, "Failed to update AI analysis");
        }

        self.save_stats().await;
        structure_changed
    }

    pub(crate) fn simple_rule_decision(&self) -> GridAction {
        if self.current_price > self.bot.upper_price * 1.02 {
            return GridAction::PauseGrid;
        }
        if self.current_price < self.bot.lower_price * 0.98 {
            return GridAction::PauseGrid;
        }
        if self.paused
            && self.current_price >= self.bot.lower_price
            && self.current_price <= self.bot.upper_price
        {
            return GridAction::RunGrid;
        }
        GridAction::Hold
    }

    pub async fn on_adjust_signal(&mut self) {
        match self.store.load_bot(self.bot.id).await {
            Ok(Some(updated_bot)) => {
                let price_changed = (updated_bot.upper_price - self.bot.upper_price).abs()
                    > f64::EPSILON
                    || (updated_bot.lower_price - self.bot.lower_price).abs() > f64::EPSILON;
                let structure_changed = updated_bot.grid_count != self.bot.grid_count
                    || (updated_bot.grid_profit_pct - self.bot.grid_profit_pct).abs()
                        > f64::EPSILON;

                if price_changed || structure_changed {
                    if structure_changed {
                        self.bot.grid_count = updated_bot.grid_count;
                        self.bot.grid_profit_pct = updated_bot.grid_profit_pct;
                    }
                    let new_upper =
                        if (updated_bot.upper_price - self.bot.upper_price).abs() > f64::EPSILON {
                            Some(updated_bot.upper_price)
                        } else {
                            None
                        };
                    let new_lower =
                        if (updated_bot.lower_price - self.bot.lower_price).abs() > f64::EPSILON {
                            Some(updated_bot.lower_price)
                        } else {
                            None
                        };
                    self.adjust_grid(new_upper, new_lower, structure_changed)
                        .await;
                } else {
                    self.bot.quantity_per_grid = updated_bot.quantity_per_grid;
                    self.bot.dynamic_adjust = updated_bot.dynamic_adjust;
                    self.bot.adjust_interval_secs = updated_bot.adjust_interval_secs;
                }
                self.bot.system_prompt = updated_bot.system_prompt;
                self.bot.leverage = updated_bot.leverage;
                self.bot.market_regime = updated_bot.market_regime;
                self.bot.grid_levels_json = updated_bot.grid_levels_json;
            }
            Ok(None) => {
                warn!(bot_id = %self.bot.id, "Adjust signal received but bot not found in store");
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Adjust signal received but failed to load bot from store");
            }
        }
    }

    pub async fn adjust_grid(
        &mut self,
        new_upper: Option<f64>,
        new_lower: Option<f64>,
        force_recalculate: bool,
    ) {
        let mut updated = false;
        if let Some(upper) = new_upper {
            if upper > 0.0 && upper != self.bot.upper_price {
                updated = true;
            }
        }
        if let Some(lower) = new_lower {
            if lower > 0.0 && lower != self.bot.lower_price {
                updated = true;
            }
        }

        if !updated && !force_recalculate {
            return;
        }

        if let Err(e) = self
            .order_executor
            .send_command(OrderCommand::CancelAllOrders {
                symbol: Some(self.bot.symbol.clone()),
            })
            .await
        {
            warn!(bot_id = %self.bot.id, error = %e, "Failed to send CancelAllOrders command");
        }

        if let Some(upper) = new_upper {
            if upper > 0.0 {
                self.bot.upper_price = upper;
            }
        }
        if let Some(lower) = new_lower {
            if lower > 0.0 {
                self.bot.lower_price = lower;
            }
        }

        if self.bot.upper_price <= self.bot.lower_price {
            warn!(bot_id = %self.bot.id, upper = self.bot.upper_price, lower = self.bot.lower_price, "adjust_grid: upper_price <= lower_price, skipping recalculate");
            return;
        }

        if let Err(e) = self
            .store
            .update_grid_params(self.bot.id, self.bot.upper_price, self.bot.lower_price)
            .await
        {
            warn!(bot_id = %self.bot.id, error = %e, "Failed to update grid params");
        }
        self.recalculate_levels();
        self.save_stats().await;

        if !self.paused {
            self.place_initial_orders().await;
        }

        if let Err(e) = self.grid_event_tx.send(GridEvent::GridAdjusted {
            bot_id: self.bot.id,
            upper_price: self.bot.upper_price,
            lower_price: self.bot.lower_price,
            level_count: self.levels.len(),
        }) {
            tracing::warn!(error = %e, event = "GridAdjusted", "Failed to send event — receiver may be dropped");
        }
    }

    pub fn recalculate_levels(&mut self) {
        let old_levels = std::mem::take(&mut self.levels);
        let holdings: Vec<GridLevel> = old_levels
            .iter()
            .filter(|l| l.hold_quantity.abs() > 0.0 || l.buy_filled || l.sell_filled)
            .cloned()
            .collect();
        drop(old_levels);

        self.levels = utils::calculate_levels(&self.bot, self.current_price);
        let max_dist = self.grid_spacing();

        let mut matched = std::collections::HashSet::new();
        for old in &holdings {
            if let Some(idx) = self.find_level_by_price_within(old.price, max_dist) {
                if matched.contains(&idx) {
                    warn!(bot_id = %self.bot.id, old_price = old.price, new_idx = idx, "Multiple old holdings matched to same new level, skipping later one");
                    continue;
                }
                matched.insert(idx);
                let level = &mut self.levels[idx];
                let side_changed = old.side != level.side;
                if side_changed {
                    warn!(bot_id = %self.bot.id, old_side = %old.side, new_side = %level.side, old_price = old.price, "Level side changed after grid adjustment, clearing holdings");
                    level.buy_filled = false;
                    level.sell_filled = false;
                    level.last_fill_price = None;
                    level.open_client_order_id = None;
                    level.buy_order_id = None;
                    level.sell_order_id = None;
                } else {
                    level.hold_quantity = old.hold_quantity;
                    level.avg_buy_price = old.avg_buy_price;
                    level.buy_filled = old.buy_filled;
                    level.sell_filled = old.sell_filled;
                    level.last_fill_price = old.last_fill_price;
                    level.open_client_order_id = old.open_client_order_id.clone();
                }
            }
        }

        self.pending_orders.clear();
    }
}

fn apply_fill_to_level(level: &mut GridLevel, trade_side: &str, price: f64, quantity: f64) {
    let is_buy = trade_side == "buy";
    if level.side == "buy" {
        if is_buy {
            let old_total = level.avg_buy_price * level.hold_quantity;
            let new_total = old_total + price * quantity;
            level.hold_quantity += quantity;
            level.avg_buy_price = if level.hold_quantity > 0.0 {
                new_total / level.hold_quantity
            } else {
                0.0
            };
            level.buy_filled = true;
            level.buy_order_id = None;
            level.last_fill_price = Some(price);
        } else {
            level.sell_filled = true;
            level.sell_order_id = None;
            level.hold_quantity = (level.hold_quantity - quantity).max(0.0);
            level.last_fill_price = Some(price);
        }
    } else {
        if !is_buy {
            let old_total = level.avg_buy_price * level.hold_quantity.abs();
            let new_total = old_total + price * quantity;
            level.hold_quantity -= quantity;
            level.avg_buy_price = if level.hold_quantity.abs() > 0.0 {
                new_total / level.hold_quantity.abs()
            } else {
                0.0
            };
            level.sell_filled = true;
            level.sell_order_id = None;
            level.last_fill_price = Some(price);
        } else {
            level.buy_filled = true;
            level.buy_order_id = None;
            level.hold_quantity = (level.hold_quantity + quantity).min(0.0);
            level.last_fill_price = Some(price);
        }
    }
}

fn calculate_fill_pnl(
    level_side: &str,
    trade_side: &str,
    entry_price: f64,
    fill_price: f64,
    quantity: f64,
) -> f64 {
    let is_close = is_close_trade(level_side, trade_side);
    if !is_close || entry_price <= 0.0 {
        return 0.0;
    }
    if level_side == "buy" {
        fill_price * quantity - entry_price * quantity
    } else {
        entry_price * quantity - fill_price * quantity
    }
}

fn is_close_trade(level_side: &str, trade_side: &str) -> bool {
    if level_side == "buy" {
        trade_side == "sell"
    } else {
        trade_side == "buy"
    }
}

fn format_grid_config(
    grid_status: &str,
    upper_price: f64,
    lower_price: f64,
    grid_count: i32,
    grid_profit_pct: f64,
    quantity_per_grid: f64,
    levels: &[GridLevel],
) -> String {
    if grid_status == "empty" {
        return "none".to_string();
    }

    let mut md = String::new();
    md.push_str(&format!("- 上界价格：{:.2}\n", upper_price));
    md.push_str(&format!("- 下界价格：{:.2}\n", lower_price));
    md.push_str(&format!("- 网格数量：{}\n", grid_count));
    md.push_str(&format!("- 网格利润：{:.2}%\n", grid_profit_pct));
    md.push_str(&format!("- 每格金额：{:.2} USDT\n\n", quantity_per_grid));
    md.push_str("| 层级 | 价格 | 方向 | 状态 | 金额(USDT) | 持仓量 | 均价 |\n");
    md.push_str("|------|------|------|------|------------|--------|------|\n");
    for l in levels {
        let status = if l.side == "buy" {
            if l.buy_filled && l.sell_filled {
                "closed"
            } else if l.buy_filled && l.hold_quantity > 0.0 {
                "holding"
            } else if l.buy_order_id.is_some() {
                "pending_buy"
            } else {
                "waiting"
            }
        } else {
            if l.sell_filled && l.buy_filled {
                "closed"
            } else if l.sell_filled && l.hold_quantity < 0.0 {
                "holding"
            } else if l.sell_order_id.is_some() {
                "pending_sell"
            } else {
                "waiting"
            }
        };
        let avg_price = if l.avg_buy_price > 0.0 {
            l.avg_buy_price
        } else {
            l.buy_price
        };
        md.push_str(&format!(
            "| {} | {:.2} | {} | {} | {:.2} | {:.6} | {:.2} |\n",
            l.level,
            l.price,
            l.side,
            status,
            l.quantity * l.price,
            l.hold_quantity,
            avg_price
        ));
    }
    md
}
