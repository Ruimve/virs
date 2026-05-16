use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::bot::semi_automatic_grid::ai::GridAiService;
use crate::bot::semi_automatic_grid::ports::*;
use crate::bot::semi_automatic_grid::types::GridLevel;

/// 单个网格 bot 的执行 worker
///
/// 管理网格层状态、订单映射、持仓跟踪等核心运行时数据
pub struct GridWorker {
    /// 网格 bot 配置
    pub(crate) bot: GridBotConfig,
    /// 价格提供者（用于获取实时价格）
    price_provider: Arc<dyn PriceProvider>,
    /// 订单执行器（用于发送下单/撤单命令）
    order_executor: Arc<dyn OrderExecutor>,
    /// AI 决策服务（用于 LLM 周期性分析）
    ai_service: Arc<GridAiService>,
    /// 数据存储（用于持久化交易记录和统计）
    store: Arc<dyn GridStore>,
    /// 市场数据提供者（用于获取快照和余额）
    market_data_provider: Arc<dyn MarketDataProvider>,
    /// 外部事件通道（从 adapter 转换后传入的订单事件）
    event_rx: broadcast::Receiver<OrderEvent>,
    /// 网格事件广播（向引擎和前端推送状态变更）
    grid_event_tx: broadcast::Sender<crate::bot::semi_automatic_grid::types::GridEvent>,
    /// 网格层状态列表
    pub(crate) levels: Vec<GridLevel>,
    /// 当前价格
    pub(crate) current_price: f64,
    /// 已实现盈亏累计
    pub(crate) total_pnl: f64,
    /// 总成交次数
    pub(crate) total_trades: i32,
    /// 网格完成次数（买卖配对完成）
    pub(crate) grid_filled_count: i32,
    /// 连续亏损配对次数（用于 AI 决策和风控）
    pub(crate) consecutive_losses: i32,
    /// 是否暂停
    pub(crate) paused: bool,
    /// order_id -> (level_index, side) 的映射（用于订单事件匹配）
    pub(crate) order_level_map: HashMap<Uuid, (usize, String)>,
    /// 初始挂单范围（当前价格 ±N 层）
    initial_order_range: usize,
    /// 防重下单：(level_index, side) -> true 表示已发送但尚未收到 on_order_placed
    pending_orders: HashMap<(usize, String), bool>,
}

impl GridWorker {
    /// 创建新的 GridWorker 实例
    ///
    /// 根据配置计算初始网格层级，初始化所有运行时状态
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
            order_level_map: HashMap::new(),
            initial_order_range: 3,
            pending_orders: HashMap::new(),
        }
    }

    /// 根据网格参数计算所有层级价格（委托给 utils::levels）
    pub(crate) fn calculate_levels(bot: &GridBotConfig) -> Vec<GridLevel> {
        crate::bot::semi_automatic_grid::utils::calculate_levels(bot)
    }
}

mod adjust;
mod events;
mod orders;
mod state;
