use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};
use tracing::{info, warn};
use uuid::Uuid;

use crate::bot::semi_automatic_grid::ai::GridAiService;
use crate::bot::semi_automatic_grid::ports::*;
use crate::bot::semi_automatic_grid::types::{GridCommand, GridEvent};
use crate::bot::semi_automatic_grid::worker::GridWorker;
use crate::engine::kline::KlineEngine;
use crate::engine::kline::types::MarketType;

/// 网格引擎
///
/// 管理所有网格 bot 的生命周期，包括启动、停止、暂停、恢复、删除和调整。
/// 通过 mpsc channel 接收外部命令，通过 broadcast channel 推送事件给前端。
pub struct GridEngine {
    /// 数据存储端口
    store: Arc<dyn GridStore>,
    /// AI 决策服务
    ai_service: Arc<GridAiService>,
    /// 价格提供者端口
    price_provider: Arc<dyn PriceProvider>,
    /// 订单执行器端口
    order_executor: Arc<dyn OrderExecutor>,
    /// 市场数据提供者端口
    market_data_provider: Arc<dyn MarketDataProvider>,
    /// 外部订单事件广播源
    event_tx: broadcast::Sender<OrderEvent>,
    /// 网格事件广播（推送给前端 WebSocket）
    grid_event_tx: broadcast::Sender<GridEvent>,
    /// 命令接收端（启动时取出，仅一次）
    cmd_rx: Option<mpsc::Receiver<GridCommand>>,
    /// bot_id -> worker 任务句柄
    workers: HashMap<Uuid, tokio::task::JoinHandle<()>>,
    /// bot_id -> 关闭信号发送端
    shutdown_txs: HashMap<Uuid, mpsc::Sender<()>>,
    /// bot_id -> 调整信号发送端
    adjust_txs: HashMap<Uuid, mpsc::Sender<()>>,
    /// bot_id -> 交易对名称（用于停止时撤单）
    bot_symbols: HashMap<Uuid, String>,
    /// K 线引擎（用于订阅实时 K 线数据）
    kline_engine: Option<Arc<KlineEngine>>,
}

impl GridEngine {
    /// 创建 GridEngine 实例
    ///
    /// 返回 (engine, command_sender, event_sender) 三元组
    pub fn new(
        store: Arc<dyn GridStore>,
        ai_service: Arc<GridAiService>,
        price_provider: Arc<dyn PriceProvider>,
        order_executor: Arc<dyn OrderExecutor>,
        market_data_provider: Arc<dyn MarketDataProvider>,
        event_tx: broadcast::Sender<OrderEvent>,
        kline_engine: Option<Arc<KlineEngine>>,
    ) -> (Self, mpsc::Sender<GridCommand>, broadcast::Sender<GridEvent>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        let (grid_event_tx, _) = broadcast::channel(256);

        let engine = Self {
            store,
            ai_service,
            price_provider,
            order_executor,
            market_data_provider,
            event_tx,
            grid_event_tx: grid_event_tx.clone(),
            cmd_rx: Some(cmd_rx),
            workers: HashMap::new(),
            shutdown_txs: HashMap::new(),
            adjust_txs: HashMap::new(),
            bot_symbols: HashMap::new(),
            kline_engine,
        };

        (engine, cmd_tx, grid_event_tx)
    }

    /// 订阅网格事件广播
    pub fn subscribe_events(&self) -> broadcast::Receiver<GridEvent> {
        self.grid_event_tx.subscribe()
    }

    /// 启动引擎主循环
    ///
    /// 先恢复之前运行中的 bot，然后进入命令接收循环
    pub async fn run(&mut self) {
        let mut cmd_rx = self.cmd_rx.take().expect("GridEngine already running");

        self.restore_running_bots().await;

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                GridCommand::StartBot { bot_id } => self.start_bot(bot_id).await,
                GridCommand::StopBot { bot_id } => self.stop_bot(bot_id, "user requested").await,
                GridCommand::PauseBot { bot_id } => self.pause_bot(bot_id).await,
                GridCommand::ResumeBot { bot_id } => self.resume_bot(bot_id).await,
                GridCommand::DeleteBot { bot_id, close_position } => self.delete_bot(bot_id, close_position).await,
                GridCommand::AdjustGrid { bot_id } => self.adjust_grid(bot_id).await,
                GridCommand::Shutdown => {
                    self.shutdown_all().await;
                    break;
                }
            }
        }

        info!("GridEngine shutdown complete");
    }

    /// 恢复之前运行中的 bot
    ///
    /// 从数据库加载状态为 "running" 的 bot，先标记为 "stopped" 再重新启动
    pub(crate) async fn restore_running_bots(&mut self) {
        let running_bots = self.store.load_running_bots().await.unwrap_or_default();

        for bot in running_bots {
            info!(bot_id = %bot.id, name = %bot.name, "Restoring running grid bot");
            let _ = self.store.update_bot_status(bot.id, "stopped").await;
            self.start_bot(bot.id).await;
        }
    }

    /// 启动指定 bot
    ///
    /// 创建 GridWorker 并在独立 tokio 任务中运行
    pub(crate) async fn start_bot(&mut self, bot_id: Uuid) {
        if self.workers.contains_key(&bot_id) {
            warn!(bot_id = %bot_id, "Bot already running");
            return;
        }

        let bot = match self.store.load_bot(bot_id).await {
            Ok(Some(b)) => b,
            Ok(None) => {
                warn!(bot_id = %bot_id, "Bot not found");
                return;
            }
            Err(e) => {
                warn!(bot_id = %bot_id, error = %e, "Failed to load bot");
                return;
            }
        };

        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);
        let (adjust_tx, adjust_rx) = mpsc::channel::<()>(1);
        let event_rx = self.event_tx.subscribe();
        let grid_event_tx = self.grid_event_tx.clone();
        let store = self.store.clone();
        let price_provider = self.price_provider.clone();
        let order_executor = self.order_executor.clone();
        let ai_service = self.ai_service.clone();
        let market_data_provider = self.market_data_provider.clone();
        let bot_symbol = bot.symbol.clone();

        /* 订阅 K 线引擎以获取实时价格更新 */
        if let Some(ref engine) = self.kline_engine {
            if let Err(e) = engine.subscribe(&bot.exchange, &bot.symbol, MarketType::Perpetual).await {
                warn!(bot_id = %bot_id, exchange = %bot.exchange, symbol = %bot.symbol, error = %e, "Failed to subscribe KlineEngine");
            }
        }

        let handle = tokio::spawn(async move {
            let mut worker = GridWorker::new(
                bot, price_provider, order_executor, ai_service, store,
                market_data_provider, event_rx, grid_event_tx,
            );
            worker.run(shutdown_rx, adjust_rx).await;
        });

        self.workers.insert(bot_id, handle);
        self.shutdown_txs.insert(bot_id, shutdown_tx);
        self.adjust_txs.insert(bot_id, adjust_tx);
        self.bot_symbols.insert(bot_id, bot_symbol);

        let _ = self.store.update_bot_status(bot_id, "running").await;
        let _ = self.grid_event_tx.send(GridEvent::BotStarted { bot_id });
        info!(bot_id = %bot_id, "Grid bot started");
    }

    /// 停止指定 bot
    ///
    /// 取消所有挂单并关闭 worker 任务
    pub(crate) async fn stop_bot(&mut self, bot_id: Uuid, reason: &str) {
        self.stop_bot_with_symbol(bot_id, reason, None).await;
    }

    /// 带交易对名称的停止 bot
    ///
    /// 优先使用缓存的 symbol，回退到数据库查询，确保撤单时能传递正确的 symbol
    async fn stop_bot_with_symbol(&mut self, bot_id: Uuid, reason: &str, symbol: Option<String>) {
        let cancel_symbol = match symbol {
            Some(s) => Some(s),
            None => {
                if let Some(s) = self.bot_symbols.get(&bot_id).cloned() {
                    Some(s)
                } else {
                    self.store.load_bot(bot_id).await.ok().flatten().map(|b| b.symbol.clone())
                }
            }
        };

        let _ = self.order_executor.send_command(OrderCommand::CancelAllOrders {
            symbol: cancel_symbol,
        }).await;

        self.graceful_shutdown_worker(bot_id).await;

        let _ = self.store.update_bot_status(bot_id, "stopped").await;
        let _ = self.grid_event_tx.send(GridEvent::BotStopped { bot_id, reason: reason.to_string() });
        info!(bot_id = %bot_id, "Grid bot stopped: {}", reason);
    }

    /// 暂停指定 bot
    ///
    /// 取消所有挂单、关闭 worker 任务，但保留 bot 状态为 "paused"
    pub(crate) async fn pause_bot(&mut self, bot_id: Uuid) {
        let symbol = if let Some(s) = self.bot_symbols.get(&bot_id).cloned() {
            Some(s)
        } else {
            self.store.load_bot(bot_id).await.ok().flatten().map(|b| b.symbol.clone())
        };
        let _ = self.order_executor.send_command(OrderCommand::CancelAllOrders {
            symbol,
        }).await;

        self.graceful_shutdown_worker(bot_id).await;

        let _ = self.store.update_bot_status(bot_id, "paused").await;
        info!(bot_id = %bot_id, "Grid bot paused");
    }

    /// 优雅关闭 worker 任务
    ///
    /// 发送关闭信号，等待最多 5 秒，超时则强制 abort
    async fn graceful_shutdown_worker(&mut self, bot_id: Uuid) {
        if let Some(tx) = self.shutdown_txs.remove(&bot_id) {
            let _ = tx.send(()).await;
        }
        self.adjust_txs.remove(&bot_id);
        self.bot_symbols.remove(&bot_id);
        if let Some(handle) = self.workers.remove(&bot_id) {
            let abort_handle = handle.abort_handle();
            match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                Ok(Ok(())) => {
                    info!(bot_id = %bot_id, "Grid worker exited gracefully");
                }
                Ok(Err(e)) => {
                    warn!(bot_id = %bot_id, error = %e, "Grid worker exited with error");
                }
                Err(_) => {
                    abort_handle.abort();
                    warn!(bot_id = %bot_id, "Grid worker shutdown timed out, aborted");
                }
            }
        }
    }

    /// 恢复指定 bot
    ///
    /// 先标记为 "stopped" 再重新启动（复用 start_bot 逻辑）
    pub(crate) async fn resume_bot(&mut self, bot_id: Uuid) {
        let _ = self.store.update_bot_status(bot_id, "stopped").await;
        self.start_bot(bot_id).await;
    }

    /// 删除指定 bot
    ///
    /// 可选平仓：先取消挂单再发送 CloseAllPositions 命令
    pub(crate) async fn delete_bot(&mut self, bot_id: Uuid, close_position: bool) {
        let bot_info = self.store.load_bot(bot_id).await.ok().flatten();
        let symbol = bot_info.as_ref().map(|b| b.symbol.clone());
        let exchange = bot_info.as_ref().map(|b| b.exchange.clone());

        if close_position {
            if let (Some(ref sym), Some(ref ex)) = (&symbol, &exchange) {
                let _ = self.order_executor.send_command(OrderCommand::CancelAllOrders {
                    symbol: Some(sym.clone()),
                }).await;

                let _ = self.order_executor.send_command(OrderCommand::CloseAllPositions {
                    symbol: sym.clone(),
                    exchange: ex.clone(),
                }).await;

                info!(bot_id = %bot_id, symbol = %sym, "Close position requested before deletion");
            } else {
                warn!(bot_id = %bot_id, "Cannot close position: bot not found in store");
            }
        }

        self.stop_bot_with_symbol(bot_id, "deleted", symbol).await;
        let _ = self.store.delete_bot(bot_id).await;
        info!(bot_id = %bot_id, close_position, "Grid bot deleted");
    }

    /// 触发指定 bot 的网格调整
    ///
    /// 通过 adjust channel 通知 worker 重新加载配置
    pub(crate) async fn adjust_grid(&mut self, bot_id: Uuid) {
        if let Some(adjust_tx) = self.adjust_txs.get(&bot_id) {
            if let Err(e) = adjust_tx.send(()).await {
                warn!(bot_id = %bot_id, error = %e, "Failed to send adjust signal to worker");
            } else {
                info!(bot_id = %bot_id, "Adjust signal sent to running worker");
            }
        } else {
            warn!(bot_id = %bot_id, "Cannot adjust: bot not running or adjust channel missing");
        }
    }

    /// 关闭所有 bot 并停止引擎
    pub(crate) async fn shutdown_all(&mut self) {
        let bot_ids: Vec<Uuid> = self.workers.keys().copied().collect();
        for id in bot_ids {
            self.stop_bot(id, "engine shutdown").await;
        }
    }
}
