use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::bot::semi_automatic_grid::ai::{GridAiService, GridAction, GridDecision};
use crate::bot::semi_automatic_grid::ports::*;
use crate::bot::semi_automatic_grid::types::{GridEvent, GridLevel, GridState};

/// 单个网格 bot 的执行 worker
pub struct GridWorker {
    pub(crate) bot: GridBotConfig,
    /// 价格提供者
    price_provider: Arc<dyn PriceProvider>,
    /// 订单执行器
    order_executor: Arc<dyn OrderExecutor>,
    /// AI 决策服务
    ai_service: Arc<GridAiService>,
    /// 数据存储
    store: Arc<dyn GridStore>,
    /// 市场数据提供者
    market_data_provider: Arc<dyn MarketDataProvider>,
    /// 外部事件通道（从 adapter 转换后传入）
    event_rx: broadcast::Receiver<OrderEvent>,
    /// 网格事件广播
    grid_event_tx: broadcast::Sender<GridEvent>,
    /// 网格层状态
    pub(crate) levels: Vec<GridLevel>,
    /// 当前价格
    pub(crate) current_price: f64,
    /// 统计
    pub(crate) total_pnl: f64,
    pub(crate) total_trades: i32,
    pub(crate) grid_filled_count: i32,
    /// 是否暂停
    pub(crate) paused: bool,
    /// order_id -> (level_index, side) 的映射
    pub(crate) order_level_map: HashMap<Uuid, (usize, String)>,
    /// 初始挂单范围（当前价格 ±N 层）
    initial_order_range: usize,
    /// 防重下单：(level_index, side) -> true 表示已发送但尚未收到 on_order_placed
    pending_orders: HashMap<(usize, String), bool>,
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
    ) -> Self {
        let levels = Self::calculate_levels(&bot);

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
            paused: false,
            order_level_map: HashMap::new(),
            initial_order_range: 3,
            pending_orders: HashMap::new(),
        }
    }

    /// 根据网格参数计算所有层级价格
    /// 优先使用 LLM 返回的 grid_levels（含每层 side），回退到 mid_price 判定
    pub(crate) fn calculate_levels(bot: &GridBotConfig) -> Vec<GridLevel> {
        if bot.grid_count <= 0 || bot.upper_price <= 0.0 || bot.lower_price <= 0.0 {
            return vec![];
        }
        let grid_spacing = (bot.upper_price - bot.lower_price) / bot.grid_count as f64;
        let profit_factor = 1.0 + bot.grid_profit_pct / 100.0;
        let mid_price = (bot.upper_price + bot.lower_price) / 2.0;

        let llm_levels: Vec<serde_json::Value> = bot.grid_levels_json
            .as_ref()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();

        (0..bot.grid_count)
            .map(|i| {
                let price = bot.lower_price + grid_spacing * (i as f64 + 0.5);
                let llm_level = llm_levels.iter().find(|v| v["level"].as_i64() == Some(i as i64));
                let side = if let Some(l) = llm_level {
                    l["side"].as_str().unwrap_or("buy").to_string()
                } else {
                    if price < mid_price { "buy".to_string() } else { "sell".to_string() }
                };
                let (buy_price, sell_price) = if side == "buy" {
                    (price, price * profit_factor)
                } else {
                    (price / profit_factor, price)
                };
                let quantity = if price > 0.0 {
                    bot.quantity_per_grid / price
                } else {
                    0.0
                };

                GridLevel {
                    level: i,
                    price,
                    side,
                    buy_price,
                    sell_price,
                    quantity,
                    buy_order_id: None,
                    sell_order_id: None,
                    buy_filled: false,
                    sell_filled: false,
                    hold_quantity: 0.0,
                    avg_buy_price: 0.0,
                    last_fill_price: None,
                }
            })
            .collect()
    }

    // ── 获取实时价格 ──

    pub(crate) async fn fetch_current_price(&self) -> f64 {
        match self.price_provider.get_price(&self.bot.exchange, &self.bot.symbol).await {
            Some(price) if price > 0.0 => price,
            _ => self.current_price,
        }
    }

    // ── 主运行循环 ──

    pub async fn run(&mut self, mut shutdown_rx: mpsc::Receiver<()>) {
        info!(
            bot_id = %self.bot.id,
            symbol = %self.bot.symbol,
            grid_count = self.bot.grid_count,
            "GridWorker starting"
        );

        if self.levels.is_empty() {
            error!(bot_id = %self.bot.id, "No grid levels calculated, check bot parameters");
            return;
        }

        for attempt in 1..=10 {
            self.current_price = self.fetch_current_price().await;
            if self.current_price > 0.0 {
                break;
            }
            warn!(bot_id = %self.bot.id, attempt, "Failed to fetch initial price, retrying in 5s...");
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        if self.current_price <= 0.0 {
            error!(bot_id = %self.bot.id, "Failed to fetch initial price after 10 attempts, worker will continue retrying in main loop");
        } else {
            info!(bot_id = %self.bot.id, price = self.current_price, "Initial price fetched");
        }

        self.load_existing_trades().await;
        self.place_initial_orders().await;

        let mut price_tick = tokio::time::interval(Duration::from_secs(5));

        let (llm_signal_tx, mut llm_signal_rx) = mpsc::channel::<()>(1);
        if self.bot.dynamic_adjust {
            let interval_secs = self.bot.adjust_interval_secs.max(60) as u64;
            info!(bot_id = %self.bot.id, interval_secs, "LLM periodic analysis enabled");
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
        } else {
            info!(bot_id = %self.bot.id, "LLM periodic analysis disabled (dynamic_adjust=false)");
        }

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!(bot_id = %self.bot.id, "GridWorker shutting down");
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

        self.save_stats().await;
    }

    // ── 历史成交 ──

    pub(crate) async fn load_existing_trades(&mut self) {
        let trades = self.store.load_trades(self.bot.id).await.unwrap_or_default();

        let trade_count = trades.len();
        for trade in trades {
            let level_idx = trade.grid_level as usize;
            if level_idx < self.levels.len() {
                let level_side = self.levels[level_idx].side.clone();
                if level_side == "buy" {
                    if trade.side == "buy" {
                        let level = &mut self.levels[level_idx];
                        let old_total = level.avg_buy_price * level.hold_quantity;
                        let new_total = old_total + trade.price * trade.quantity;
                        level.hold_quantity += trade.quantity;
                        level.avg_buy_price = if level.hold_quantity > 0.0 {
                            new_total / level.hold_quantity
                        } else {
                            0.0
                        };
                        level.buy_filled = true;
                        level.last_fill_price = Some(trade.price);
                    } else {
                        let level = &mut self.levels[level_idx];
                        level.hold_quantity = (level.hold_quantity - trade.quantity).max(0.0);
                        level.sell_filled = true;
                        level.last_fill_price = Some(trade.price);
                    }
                } else {
                    if trade.side == "sell" {
                        let level = &mut self.levels[level_idx];
                        let old_total = level.avg_buy_price * level.hold_quantity.abs();
                        let new_total = old_total + trade.price * trade.quantity;
                        level.hold_quantity -= trade.quantity;
                        level.avg_buy_price = if level.hold_quantity.abs() > 0.0 {
                            new_total / level.hold_quantity.abs()
                        } else {
                            0.0
                        };
                        level.sell_filled = true;
                        level.last_fill_price = Some(trade.price);
                    } else {
                        let level = &mut self.levels[level_idx];
                        level.hold_quantity = (level.hold_quantity + trade.quantity).min(0.0);
                        level.buy_filled = true;
                        level.last_fill_price = Some(trade.price);
                    }
                }
            }
            self.total_pnl += trade.pnl;
            self.total_trades += 1;
        }

        info!(
            bot_id = %self.bot.id,
            loaded_trades = trade_count,
            total_pnl = self.total_pnl,
            "Loaded existing grid trades"
        );

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

    // ── 初始挂单 ──

    pub(crate) async fn place_initial_orders(&mut self) {
        if self.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "No current price, skipping initial orders");
            return;
        }

        let current_level_idx = self.find_level_by_price(self.current_price);

        let buy_init_levels: Vec<GridLevel> = self.levels.iter().enumerate()
            .filter(|(i, level)| {
                level.side == "buy"
                    && level.buy_price < self.current_price
                    && !level.buy_filled
                    && level.buy_order_id.is_none()
                    && i.saturating_sub(current_level_idx) <= self.initial_order_range
            })
            .map(|(_, level)| level.clone())
            .collect();

        for level in &buy_init_levels {
            self.place_buy_order(level).await;
        }

        let sell_init_levels: Vec<GridLevel> = self.levels.iter().enumerate()
            .filter(|(i, level)| {
                level.side == "sell"
                    && level.sell_price > self.current_price
                    && !level.sell_filled
                    && level.sell_order_id.is_none()
                    && current_level_idx.saturating_sub(*i) <= self.initial_order_range
            })
            .map(|(_, level)| level.clone())
            .collect();

        for level in &sell_init_levels {
            self.place_sell_order(level).await;
        }

        let close_levels: Vec<GridLevel> = self.levels.iter()
            .filter(|level| level.hold_quantity.abs() > 0.0
                && ((level.side == "buy" && level.hold_quantity > 0.0 && level.sell_order_id.is_none())
                    || (level.side == "sell" && level.hold_quantity < 0.0 && level.buy_order_id.is_none())))
            .cloned()
            .collect();

        for level in &close_levels {
            if level.side == "buy" {
                self.place_sell_order(level).await;
            } else {
                self.place_buy_order(level).await;
            }
        }

        info!(bot_id = %self.bot.id, current_level = current_level_idx, "Initial orders placed");
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

    // ── 价格 tick ──

    pub(crate) async fn on_price_tick(&mut self) {
        if self.current_price <= 0.0 {
            return;
        }

        let buy_open_levels: Vec<GridLevel> = self.levels.iter()
            .filter(|level| {
                level.side == "buy"
                    && self.current_price < level.buy_price
                    && !level.buy_filled
                    && level.buy_order_id.is_none()
            })
            .cloned()
            .collect();

        for level in &buy_open_levels {
            self.place_buy_order(level).await;
        }

        let sell_open_levels: Vec<GridLevel> = self.levels.iter()
            .filter(|level| {
                level.side == "sell"
                    && level.sell_price > self.current_price
                    && !level.sell_filled
                    && level.sell_order_id.is_none()
            })
            .cloned()
            .collect();

        for level in &sell_open_levels {
            self.place_sell_order(level).await;
        }

        let buy_close_levels: Vec<GridLevel> = self.levels.iter()
            .filter(|level| {
                level.side == "buy"
                    && level.hold_quantity > 0.0
                    && self.current_price >= level.sell_price
                    && level.sell_order_id.is_none()
            })
            .cloned()
            .collect();

        for level in &buy_close_levels {
            self.place_sell_order(level).await;
        }

        let sell_close_levels: Vec<GridLevel> = self.levels.iter()
            .filter(|level| {
                level.side == "sell"
                    && level.hold_quantity < 0.0
                    && self.current_price <= level.buy_price
                    && level.buy_order_id.is_none()
            })
            .cloned()
            .collect();

        for level in &sell_close_levels {
            self.place_buy_order(level).await;
        }

        self.broadcast_state();
    }

    // ── 外部事件处理 ──

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
            OrderEvent::LiquidationWarning { symbol, liquidation_price, current_price } => {
                warn!(
                    bot_id = %self.bot.id, %symbol,
                    liquidation_price, current_price,
                    "Liquidation warning"
                );
                self.pause_with_cancel("liquidation warning").await;
            }
        }
    }

    pub(crate) async fn pause_with_cancel(&mut self, reason: &str) {
        if !self.paused {
            self.paused = true;
            let _ = self.order_executor.send_command(OrderCommand::CancelAllOrders {
                symbol: Some(self.bot.symbol.clone()),
            }).await;
            warn!(bot_id = %self.bot.id, "Grid paused due to {}", reason);
        }
    }

    // ── 订单匹配 ──

    pub(crate) async fn on_order_placed(&mut self, order: &OrderInfo) {
        if let Some(&(level_idx, ref side)) = self.order_level_map.get(&order.id) {
            info!(bot_id = %self.bot.id, level = level_idx, side = %side, order_id = %order.id, "Grid order placed (via map)");
            return;
        }

        if let Some(ref coi) = order.client_order_id {
            if let Some((level_idx, side)) = Self::parse_client_order_id(coi, &self.bot.id) {
                if level_idx < self.levels.len() {
                    self.pending_orders.remove(&(level_idx, side.clone()));
                    self.order_level_map.insert(order.id, (level_idx, side.clone()));

                    if side == "buy" {
                        self.levels[level_idx].buy_order_id = Some(order.id);
                    } else {
                        self.levels[level_idx].sell_order_id = Some(order.id);
                    }

                    info!(
                        bot_id = %self.bot.id, level = self.levels[level_idx].level,
                        side = %side, order_id = %order.id,
                        "Grid order placed (via client_order_id)"
                    );
                    return;
                }
            }
        }

        debug!(
            bot_id = %self.bot.id, order_id = %order.id,
            "Order placed event received but no matching grid level"
        );
    }

    fn parse_client_order_id(coi: &str, bot_id: &Uuid) -> Option<(usize, String)> {
        let parts: Vec<&str> = coi.splitn(4, ':').collect();
        if parts.len() == 4 && parts[0] == "grid" {
            if parts[1] != bot_id.to_string() {
                return None;
            }
            if let Ok(level_idx) = parts[2].parse::<usize>() {
                let side = parts[3].to_string();
                return Some((level_idx, side));
            }
        }
        None
    }

    pub(crate) async fn on_order_filled(&mut self, order: &OrderInfo) {
        let side_str = order.side.as_str();

        let matched_idx = if let Some(&(idx, ref side)) = self.order_level_map.get(&order.id) {
            if side == side_str { Some(idx) } else { None }
        } else {
            None
        };

        let idx = match matched_idx {
            Some(i) => i,
            None => {
                debug!(bot_id = %self.bot.id, order_id = %order.id, "Order filled but no matching grid level");
                return;
            }
        };

        let price = order.fill_price.unwrap_or(0.0);
        let level = &mut self.levels[idx];
        let is_buy_match = order.side == OrderSide::Buy;
        let is_sell_match = !is_buy_match;
        let level_num = level.level;
        let level_side = level.side.clone();

        if level_side == "buy" {
            if is_buy_match {
                let old_total = level.avg_buy_price * level.hold_quantity;
                let new_total = old_total + price * order.filled;
                level.hold_quantity += order.filled;
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
                level.hold_quantity = (level.hold_quantity - order.filled).max(0.0);
                level.last_fill_price = Some(price);
            }
        } else {
            if is_sell_match {
                let old_total = level.avg_buy_price * level.hold_quantity.abs();
                let new_total = old_total + price * order.filled;
                level.hold_quantity -= order.filled;
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
                level.hold_quantity = (level.hold_quantity + order.filled).min(0.0);
                level.last_fill_price = Some(price);
            }
        }

        self.order_level_map.remove(&order.id);
        self.pending_orders.remove(&(idx, side_str.to_string()));

        let pnl = if level_side == "buy" {
            if is_sell_match && self.levels[idx].avg_buy_price > 0.0 {
                let buy_cost = self.levels[idx].avg_buy_price * order.filled;
                let sell_revenue = price * order.filled;
                sell_revenue - buy_cost
            } else {
                0.0
            }
        } else {
            if is_buy_match && self.levels[idx].avg_buy_price > 0.0 {
                let sell_revenue = self.levels[idx].avg_buy_price * order.filled;
                let buy_cost = price * order.filled;
                sell_revenue - buy_cost
            } else {
                0.0
            }
        };

        let hold = self.levels[idx].hold_quantity;
        self.total_pnl += pnl;
        self.total_trades += 1;
        self.grid_filled_count += 1;

        if level_side == "buy" && is_sell_match && hold <= 0.0 {
            let rebuy = GridLevel {
                level: level_num, price: self.levels[idx].price,
                side: self.levels[idx].side.clone(),
                buy_price: self.levels[idx].buy_price, sell_price: self.levels[idx].sell_price,
                quantity: self.levels[idx].quantity, buy_order_id: None, sell_order_id: None,
                buy_filled: false, sell_filled: false, hold_quantity: 0.0,
                avg_buy_price: 0.0, last_fill_price: None,
            };
            self.place_buy_order(&rebuy).await;
        } else if level_side == "sell" && is_buy_match && hold >= 0.0 {
            let reoffer = GridLevel {
                level: level_num, price: self.levels[idx].price,
                side: self.levels[idx].side.clone(),
                buy_price: self.levels[idx].buy_price, sell_price: self.levels[idx].sell_price,
                quantity: self.levels[idx].quantity, buy_order_id: None, sell_order_id: None,
                buy_filled: false, sell_filled: false, hold_quantity: 0.0,
                avg_buy_price: 0.0, last_fill_price: None,
            };
            self.place_sell_order(&reoffer).await;
        }

        self.record_trade(level_num, side_str, price, order.filled, pnl).await;

        if is_sell_match {
            let _ = self.grid_event_tx.send(GridEvent::GridTradeClosed { bot_id: self.bot.id, level: level_num, pnl });
        }
        let _ = self.grid_event_tx.send(GridEvent::GridFilled {
            bot_id: self.bot.id, level: level_num, side: side_str.to_string(), price, quantity: order.filled,
        });

        info!(
            bot_id = %self.bot.id, level = level_num, side = %side_str,
            price, quantity = order.filled, pnl, hold,
            "Grid order filled"
        );
    }

    pub(crate) async fn on_order_canceled(&mut self, order_id: Uuid) {
        self.clear_order_id(order_id);
        debug!(bot_id = %self.bot.id, order_id = %order_id, "Grid order canceled");
    }

    pub(crate) fn clear_order_id(&mut self, order_id: Uuid) {
        if let Some((idx, side)) = self.order_level_map.remove(&order_id) {
            self.pending_orders.remove(&(idx, side));
        }
        for level in &mut self.levels {
            if level.buy_order_id == Some(order_id) {
                level.buy_order_id = None;
                self.pending_orders.remove(&(level.level as usize, "buy".to_string()));
            }
            if level.sell_order_id == Some(order_id) {
                level.sell_order_id = None;
                self.pending_orders.remove(&(level.level as usize, "sell".to_string()));
            }
        }
    }

    // ── 下单 ──

    async fn place_buy_order(&mut self, level: &GridLevel) {
        let key = (level.level as usize, "buy".to_string());
        if self.pending_orders.contains_key(&key) {
            return;
        }
        let (amount, reduce_only) = if level.side == "sell" {
            (level.hold_quantity.abs().min(level.quantity), true)
        } else {
            (level.quantity, false)
        };
        let client_order_id = Some(format!("grid:{}:{}:buy", self.bot.id, level.level));
        let cmd = OrderCommand::PlaceOrder {
            symbol: self.bot.symbol.clone(),
            side: OrderSide::Buy,
            amount,
            price: Some(level.buy_price),
            reduce_only,
            client_order_id,
        };
        if let Err(e) = self.order_executor.send_command(cmd).await {
            error!(bot_id = %self.bot.id, level = level.level, error = %e, "Failed to send buy order");
        } else {
            self.pending_orders.insert(key, true);
        }
    }

    async fn place_sell_order(&mut self, level: &GridLevel) {
        let key = (level.level as usize, "sell".to_string());
        if self.pending_orders.contains_key(&key) {
            return;
        }
        let (amount, reduce_only) = if level.side == "sell" {
            (level.quantity, false)
        } else {
            (level.hold_quantity.min(level.quantity), true)
        };
        let client_order_id = Some(format!("grid:{}:{}:sell", self.bot.id, level.level));
        let cmd = OrderCommand::PlaceOrder {
            symbol: self.bot.symbol.clone(),
            side: OrderSide::Sell,
            amount,
            price: Some(level.sell_price),
            reduce_only,
            client_order_id,
        };
        if let Err(e) = self.order_executor.send_command(cmd).await {
            error!(bot_id = %self.bot.id, level = level.level, error = %e, "Failed to send sell order");
        } else {
            self.pending_orders.insert(key, true);
        }
    }

    // ── 数据库 ──

    async fn record_trade(&self, level: i32, side: &str, price: f64, quantity: f64, pnl: f64) {
        let pnl_pct = if price > 0.0 { pnl / (price * quantity) * 100.0 } else { 0.0 };
        let _ = self.store.record_trade(
            self.bot.id, self.bot.user_id, &self.bot.symbol, &self.bot.exchange,
            side, level, price, quantity, pnl, pnl_pct,
        ).await;
    }

    async fn save_stats(&self) {
        let _ = self.store.save_stats(self.bot.id, self.total_pnl, self.total_trades, self.grid_filled_count).await;
    }

    // ── 状态广播 ──

    pub(crate) fn broadcast_state(&self) {
        let state = GridState {
            bot_id: self.bot.id,
            symbol: self.bot.symbol.clone(),
            exchange: self.bot.exchange.clone(),
            levels: self.levels.clone(),
            current_price: self.current_price,
            total_pnl: self.total_pnl,
            total_trades: self.total_trades,
            grid_filled_count: self.grid_filled_count,
            last_tick_at: Utc::now(),
        };
        let _ = self.grid_event_tx.send(GridEvent::StatusUpdate { bot_id: self.bot.id, state });
    }

    // ── LLM 决策 ──

    async fn on_llm_decision(&mut self) {
        info!(bot_id = %self.bot.id, "LLM decision tick");

        if !self.ai_service.is_available_for_user(&self.bot.user_id).await {
            warn!(bot_id = %self.bot.id, "AI service not available, skipping LLM decision");
            let _ = self.grid_event_tx.send(GridEvent::BotError {
                bot_id: self.bot.id,
                error: "LLM decision skipped: AI service not configured".to_string(),
            });
            return;
        }

        let _filled_count = self.levels.iter().filter(|l| l.buy_filled).count();
        let total_hold: f64 = self.levels.iter().map(|l| l.hold_quantity).sum();

        let snapshot = self.market_data_provider.get_market_snapshot(&self.bot.exchange, &self.bot.symbol).await;

        if snapshot.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "Market snapshot has zero price, skipping LLM decision");
            let _ = self.grid_event_tx.send(GridEvent::BotError {
                bot_id: self.bot.id,
                error: "LLM decision skipped: market data unavailable".to_string(),
            });
            return;
        }

        let grid_status = if self.paused { "paused" } else if self.levels.is_empty() { "empty" } else { "running" };

        let grid_levels_json: Vec<serde_json::Value> = self.levels.iter().map(|l| {
            let status = if l.side == "buy" {
                if l.buy_filled && l.sell_filled { "sold" } else if l.buy_filled && l.hold_quantity > 0.0 { "hold" } else { "buy" }
            } else {
                if l.sell_filled && l.buy_filled { "bought" } else if l.sell_filled && l.hold_quantity < 0.0 { "hold" } else { "sell" }
            };
            serde_json::json!({
                "level": l.level,
                "price": l.price,
                "side": l.side,
                "status": status,
                "quantity_usdt": l.quantity * l.price,
                "buy_filled": l.buy_filled,
                "sell_filled": l.sell_filled,
                "hold_quantity": l.hold_quantity,
                "avg_buy_price": if l.avg_buy_price > 0.0 { l.avg_buy_price } else { l.buy_price },
                "last_fill_price": l.last_fill_price,
            })
        }).collect();

        let current_grid_config = if grid_status == "empty" {
            "none".to_string()
        } else {
            serde_json::json!({
                "upper_price": self.bot.upper_price,
                "lower_price": self.bot.lower_price,
                "grid_count": self.bot.grid_count,
                "grid_profit_pct": self.bot.grid_profit_pct,
                "quantity_per_grid": self.bot.quantity_per_grid,
                "grid_levels": grid_levels_json,
            }).to_string()
        };

        let ema_distance_pct = if snapshot.ema50 > 0.0 {
            (snapshot.ema20 - snapshot.ema50) / snapshot.ema50 * 100.0
        } else { 0.0 };

        let account = self.market_data_provider.get_account_balance(&self.bot.exchange).await;
        let margin_usage_rate = if account.total > 0.0 {
            account.used / account.total * 100.0
        } else { 0.0 };

        let template = super::types::DEFAULT_USER_PROMPT_TEMPLATE;

        let user_prompt = template
            .replace("{timestamp}", &chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string())
            .replace("{symbol}", &self.bot.symbol)
            .replace("{total_balance}", &format!("{:.2}", account.total))
            .replace("{available_balance}", &format!("{:.2}", account.free))
            .replace("{used_margin}", &format!("{:.2}", account.used))
            .replace("{margin_usage_rate}", &format!("{:.1}", margin_usage_rate))
            .replace("{leverage}", &self.bot.leverage.to_string())
            .replace("{grid_status}", grid_status)
            .replace("{last_adjust_time}", "N/A")
            .replace("{consecutive_losses}", "0")
            .replace("{current_grid_config}", &current_grid_config)
            .replace("{position_base}", &format!("{:.6}", total_hold))
            .replace("{position_side}", "long")
            .replace("{entry_price}", &format!("{:.2}", self.bot.lower_price))
            .replace("{unrealized_pnl}", &format!("{:.2}", self.total_pnl))
            .replace("{open_orders}", "[]")
            .replace("{funding_rate}", &format!("{:.6}", snapshot.funding_rate))
            .replace("{funding_next_time}", "N/A")
            .replace("{event_flag}", "false")
            .replace("{event_description}", "")
            .replace("{h1_current_price}", &format!("{:.2}", snapshot.current_price))
            .replace("{h1_bb_upper}", &format!("{:.2}", snapshot.bb_upper))
            .replace("{h1_bb_middle}", &format!("{:.2}", snapshot.bb_middle))
            .replace("{h1_bb_lower}", &format!("{:.2}", snapshot.bb_lower))
            .replace("{h1_bb_width_pct}", &format!("{:.2}", snapshot.bb_width))
            .replace("{h1_ema20}", &format!("{:.2}", snapshot.ema20))
            .replace("{h1_ema50}", &format!("{:.2}", snapshot.ema50))
            .replace("{h1_ema_distance_pct}", &format!("{:+.2}", ema_distance_pct))
            .replace("{h1_adx}", &format!("{:.2}", snapshot.adx))
            .replace("{h1_atr}", &format!("{:.4}", snapshot.atr))
            .replace("{h1_atr_sma20}", &format!("{:.4}", snapshot.h1_atr_sma20))
            .replace("{h1_candle_body}", &format!("{:+.4}", snapshot.h1_candle_body))
            .replace("{h1_bars_outside_band}", &snapshot.h1_bars_outside_band.to_string())
            .replace("{h1_bandwidth_5bars_ago}", &format!("{:.2}", snapshot.h1_bandwidth_5bars_ago))
            .replace("{h1_high_20}", &format!("{:.2}", snapshot.h1_high_20))
            .replace("{h1_low_20}", &format!("{:.2}", snapshot.h1_low_20))
            .replace("{nearest_round_up}", &format!("{:.2}", snapshot.nearest_round_up))
            .replace("{nearest_round_down}", &format!("{:.2}", snapshot.nearest_round_down))
            .replace("{m15_current_price}", &format!("{:.2}", snapshot.m15_current_price))
            .replace("{m15_bb_width_pct}", &format!("{:.2}", snapshot.m15_bb_width_pct))
            .replace("{m15_atr}", &format!("{:.4}", snapshot.m15_atr))
            .replace("{m15_atr_sma20}", &format!("{:.4}", snapshot.m15_atr_sma20))
            .replace("{m15_adx}", &format!("{:.2}", snapshot.m15_adx))
            .replace("{m15_bars_outside_band}", &snapshot.m15_bars_outside_band.to_string())
            .replace("{m15_ema20}", &format!("{:.2}", snapshot.m15_ema20))
            .replace("{m15_ema50}", &format!("{:.2}", snapshot.m15_ema50))
            .replace("{h4_ema20}", &format!("{:.2}", snapshot.h4_ema20))
            .replace("{h4_ema50}", &format!("{:.2}", snapshot.h4_ema50))
            .replace("{h4_adx}", &format!("{:.2}", snapshot.h4_adx))
            .replace("{h4_bb_width_pct}", &format!("{:.2}", snapshot.h4_bb_width_pct))
            .replace("{trigger_reason}", "scheduled_15m");

        let system_prompt = self.bot.system_prompt.as_deref().unwrap_or(LLM_RUNTIME_PROMPT);

        let decision = self.ai_service.grid_decision(&self.bot.user_id, system_prompt, &user_prompt).await;

        let action = match decision {
            Some(ref d) => {
                info!(bot_id = %self.bot.id, action = d.action.as_str(), reason = %d.reason, source = "llm", "LLM decision");

                let result = serde_json::json!({
                    "action": d.action.as_str(),
                    "reason": d.reason,
                    "upper_price": d.upper_price,
                    "lower_price": d.lower_price,
                });
                let _ = self.store.save_analysis_log(
                    self.bot.id, "periodic", system_prompt, &user_prompt,
                    &result, None,
                ).await;

                d.action.clone()
            }
            None => {
                let rule_action = self.simple_rule_decision();
                warn!(bot_id = %self.bot.id, action = rule_action.as_str(), source = "rule_fallback", "LLM call failed, falling back to rule-based decision");

                let result = serde_json::json!({
                    "action": rule_action.as_str(),
                    "reason": "LLM call failed, using rule-based fallback",
                });
                let _ = self.store.save_analysis_log(
                    self.bot.id, "periodic", system_prompt, &user_prompt,
                    &result, Some("LLM call failed"),
                ).await;

                let _ = self.grid_event_tx.send(GridEvent::BotError {
                    bot_id: self.bot.id,
                    error: "LLM call failed, using rule-based fallback".to_string(),
                });
                rule_action
            }
        };

        self.execute_decision(&action, decision.as_ref()).await;
        let _ = self.store.update_last_adjusted(self.bot.id).await;
    }

    pub(crate) async fn execute_decision(&mut self, action: &GridAction, decision: Option<&GridDecision>) {
        match action {
            GridAction::PauseGrid => {
                self.pause_with_cancel("LLM decision").await;
            }
            GridAction::RunGrid => {
                if self.paused {
                    self.paused = false;
                    self.place_initial_orders().await;
                    info!(bot_id = %self.bot.id, "Grid resumed by decision");
                }
            }
            GridAction::ReducePosition => {
                let new_qty = self.bot.quantity_per_grid * 0.5;
                let _ = self.store.update_quantity_per_grid(self.bot.id, new_qty).await;
                self.bot.quantity_per_grid = new_qty;
                self.levels = Self::calculate_levels(&self.bot);
                warn!(bot_id = %self.bot.id, new_qty, "Position reduced by decision");
            }
            GridAction::AdjustGrid { .. } => {
                if let Some(d) = decision {
                    self.adjust_grid(d.upper_price, d.lower_price).await;
                }
            }
            GridAction::Hold => {}
        }
    }

    pub(crate) async fn adjust_grid(&mut self, new_upper: Option<f64>, new_lower: Option<f64>) {
        let _ = self.order_executor.send_command(OrderCommand::CancelAllOrders {
            symbol: Some(self.bot.symbol.clone()),
        }).await;

        let mut updated = false;
        if let Some(upper) = new_upper {
            if upper > 0.0 && upper != self.bot.upper_price {
                self.bot.upper_price = upper;
                updated = true;
            }
        }
        if let Some(lower) = new_lower {
            if lower > 0.0 && lower != self.bot.lower_price {
                self.bot.lower_price = lower;
                updated = true;
            }
        }

        if !updated {
            debug!(bot_id = %self.bot.id, "adjust_grid: no parameter changes");
            return;
        }

        let _ = self.store.update_grid_params(self.bot.id, self.bot.upper_price, self.bot.lower_price).await;
        self.recalculate_levels();

        if !self.paused {
            self.place_initial_orders().await;
        }

        info!(
            bot_id = %self.bot.id, new_upper = self.bot.upper_price,
            new_lower = self.bot.lower_price, grid_count = self.levels.len(),
            "Grid adjusted"
        );

        let _ = self.grid_event_tx.send(GridEvent::BotError {
            bot_id: self.bot.id,
            error: format!("Grid adjusted: upper={:.2}, lower={:.2}, levels={}", self.bot.upper_price, self.bot.lower_price, self.levels.len()),
        });
    }

    pub(crate) fn simple_rule_decision(&self) -> GridAction {
        if self.current_price > self.bot.upper_price * 1.02 {
            return GridAction::PauseGrid;
        }
        if self.current_price < self.bot.lower_price * 0.98 {
            return GridAction::PauseGrid;
        }
        if self.paused && self.current_price >= self.bot.lower_price && self.current_price <= self.bot.upper_price {
            return GridAction::RunGrid;
        }
        GridAction::Hold
    }

    pub fn recalculate_levels(&mut self) {
        self.levels = Self::calculate_levels(&self.bot);
        self.order_level_map.clear();
        info!(bot_id = %self.bot.id, grid_count = self.levels.len(), "Grid levels recalculated");
    }
}

const LLM_RUNTIME_PROMPT: &str = r#"你是一位正在管理加密货币网格交易机器人的 AI 助手。你的职责是根据当前市场状态和机器人运行数据，做出最优决策。

## 决策规则
1. **run_grid**: 价格在网格区间内且市场状态适合网格交易时，继续运行
2. **pause_grid**: 价格突破网格区间（超过上下界 2%）、市场转为强趋势、或连续亏损时暂停
3. **adjust_grid**: 市场波动率显著变化，需要调整网格上下界时
4. **reduce_position**: 高波动或连续亏损时，减半仓位
5. **hold**: 当前状态良好，无需操作

## 注意
- 暂停后不会自动恢复，需要明确的 run_grid 指令
- adjust_grid 必须返回新的 upper_price 和 lower_price
- 优先保守操作，避免在不确定时频繁调整"#;

