use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::bot::semi_automatic_grid::ai::GridAiService;
use crate::bot::semi_automatic_grid::ports::*;
use crate::bot::semi_automatic_grid::types::GridLevel;

/** 单个网格 bot 的执行 worker

管理网格层状态、持仓跟踪等核心运行时数据 */
pub struct GridWorker {
/** 网格 bot 配置 */
    pub(crate) bot: GridBotConfig,
/** 价格提供者（用于获取实时价格） */
    price_provider: Arc<dyn PriceProvider>,
/** 订单执行器（用于发送下单/撤单命令） */
    order_executor: Arc<dyn OrderExecutor>,
/** AI 决策服务（用于 LLM 周期性分析） */
    ai_service: Arc<GridAiService>,
/** 数据存储（用于持久化交易记录和统计） */
    store: Arc<dyn GridStore>,
/** 市场数据提供者（用于获取快照和余额） */
    market_data_provider: Arc<dyn MarketDataProvider>,
/** 外部事件通道（从 adapter 转换后传入的订单事件） */
    event_rx: broadcast::Receiver<OrderEvent>,
/** 网格事件广播（向引擎和前端推送状态变更） */
    grid_event_tx: broadcast::Sender<crate::bot::semi_automatic_grid::types::GridEvent>,
/** 网格层状态列表 */
    pub(crate) levels: Vec<GridLevel>,
/** 当前价格 */
    pub(crate) current_price: f64,
/** 已实现盈亏累计 */
    pub(crate) total_pnl: f64,
/** 总成交次数 */
    pub(crate) total_trades: i32,
/** 网格完成次数（买卖配对完成） */
    pub(crate) grid_filled_count: i32,
/** 连续亏损配对次数（用于 AI 决策和风控） */
    pub(crate) consecutive_losses: i32,
/** 是否暂停 */
    pub(crate) paused: bool,
/** 初始挂单范围（当前价格 ±N 层） */
    initial_order_range: usize,
/** 防重下单：(level_index, side) 表示已发送 PlaceOrder 但尚未收到 OrderPlaced 确认 */
    pending_orders: HashSet<(usize, String)>,
}

impl GridWorker {
/** 创建新的 GridWorker 实例

根据配置计算初始网格层级，初始化所有运行时状态 */
    pub fn new(
        bot: GridBotConfig,
        price_provider: Arc<dyn PriceProvider>,
        order_executor: Arc<dyn OrderExecutor>,
        ai_service: Arc<GridAiService>,
        store: Arc<dyn GridStore>,
        market_data_provider: Arc<dyn MarketDataProvider>,
        event_rx: broadcast::Receiver<OrderEvent>,
        grid_event_tx: broadcast::Sender<crate::bot::semi_automatic_grid::types::GridEvent>,
    ) -> Self {
        let levels = crate::bot::semi_automatic_grid::utils::calculate_levels(&bot);

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
        }
    }

/** 根据网格参数计算所有层级价格（委托给 utils::levels） */
    pub(crate) fn calculate_levels(bot: &GridBotConfig) -> Vec<GridLevel> {
        crate::bot::semi_automatic_grid::utils::calculate_levels(bot)
    }

/** 通过 order_id 反向查找对应的层级索引和方向

遍历 levels 的 buy_order_id / sell_order_id 字段进行匹配，
替代原先的 order_level_map HashMap */
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

/** 取消指定层级的挂单

通过 order_id 精确撤销单个挂单，而非 CancelAllOrders */
    pub(crate) async fn cancel_level_order(&mut self, level_idx: usize, side: &str) {
        let order_id = if side == "buy" {
            self.levels[level_idx].buy_order_id
        } else {
            self.levels[level_idx].sell_order_id
        };

        if let Some(oid) = order_id {
            let _ = self.order_executor.send_command(OrderCommand::CancelOrder {
                order_id: oid,
                symbol: self.bot.symbol.clone(),
            }).await;
        }
    }
}

mod adjust;
mod events;
mod orders;
mod state;
