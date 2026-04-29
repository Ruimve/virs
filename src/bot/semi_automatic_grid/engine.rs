use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::{broadcast, mpsc};
use tracing::{info, warn};
use uuid::Uuid;

use crate::engine::strategy::StrategyEngine;
use crate::bot::semi_automatic_grid::ai::GridAiService;
use crate::bot::semi_automatic_grid::types::{GridCommand, GridEvent};
use crate::bot::semi_automatic_grid::worker::GridWorker;
use crate::models::{GridBot, StrategyStatus};
use crate::engine::position::types::EngineCommand;

pub struct GridEngine {
    db: PgPool,
    /// 策略引擎（用于获取实时价格）
    strategy_engine: Arc<StrategyEngine>,
    /// AI 决策服务
    ai_service: Arc<GridAiService>,
    /// Position Engine 命令通道
    pe_cmd_tx: mpsc::Sender<crate::engine::position::types::EngineCommand>,
    /// Position Engine 事件通道
    pe_event_rx: broadcast::Receiver<crate::engine::position::types::EngineEvent>,
    /// 网格事件广播（发送给前端）
    event_tx: broadcast::Sender<GridEvent>,
    /// 网格命令接收
    cmd_rx: Option<mpsc::Receiver<GridCommand>>,
    /// 运行中的 bot workers
    workers: HashMap<Uuid, tokio::task::JoinHandle<()>>,
    /// 运行中 bot 的 shutdown channel
    shutdown_txs: HashMap<Uuid, mpsc::Sender<()>>,
}

impl GridEngine {
    pub fn new(
        db: PgPool,
        strategy_engine: Arc<StrategyEngine>,
        ai_service: Arc<GridAiService>,
        pe_cmd_tx: mpsc::Sender<crate::engine::position::types::EngineCommand>,
        pe_event_rx: broadcast::Receiver<crate::engine::position::types::EngineEvent>,
    ) -> (Self, mpsc::Sender<GridCommand>, broadcast::Sender<GridEvent>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        let (event_tx, _) = broadcast::channel(256);

        let engine = Self {
            db,
            strategy_engine,
            ai_service,
            pe_cmd_tx,
            pe_event_rx,
            event_tx: event_tx.clone(),
            cmd_rx: Some(cmd_rx),
            workers: HashMap::new(),
            shutdown_txs: HashMap::new(),
        };

        (engine, cmd_tx, event_tx)
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<GridEvent> {
        self.event_tx.subscribe()
    }

    /// 启动引擎主循环
    pub async fn run(&mut self) {
        let mut cmd_rx = self.cmd_rx.take().expect("GridEngine already running");

        // 恢复数据库中状态为 running 的 bot
        self.restore_running_bots().await;

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                GridCommand::StartBot { bot_id } => self.start_bot(bot_id).await,
                GridCommand::StopBot { bot_id } => self.stop_bot(bot_id, "user requested").await,
                GridCommand::PauseBot { bot_id } => self.pause_bot(bot_id).await,
                GridCommand::ResumeBot { bot_id } => self.resume_bot(bot_id).await,
                GridCommand::DeleteBot { bot_id } => self.delete_bot(bot_id).await,
                GridCommand::AdjustGrid { bot_id } => self.adjust_grid(bot_id).await,
                GridCommand::Shutdown => {
                    self.shutdown_all().await;
                    break;
                }
            }
        }

        info!("GridEngine shutdown complete");
    }

    async fn restore_running_bots(&mut self) {
        let running_bots: Vec<GridBot> =
            sqlx::query_as("SELECT * FROM qd_grid_bots WHERE status = 'running'")
                .fetch_all(&self.db)
                .await
                .unwrap_or_default();

        for bot in running_bots {
            info!(
                bot_id = %bot.id,
                name = %bot.name,
                "Restoring running grid bot"
            );
            // 标记为 stopped 然后重新启动
            sqlx::query("UPDATE qd_grid_bots SET status = 'stopped' WHERE id = $1")
                .bind(bot.id)
                .execute(&self.db)
                .await
                .ok();
            self.start_bot(bot.id).await;
        }
    }

    async fn start_bot(&mut self, bot_id: Uuid) {
        if self.workers.contains_key(&bot_id) {
            warn!(bot_id = %bot_id, "Bot already running");
            return;
        }

        let bot: Option<GridBot> =
            sqlx::query_as("SELECT * FROM qd_grid_bots WHERE id = $1")
                .bind(bot_id)
                .fetch_optional(&self.db)
                .await
                .unwrap_or(None);

        let bot = match bot {
            Some(b) => b,
            None => {
                warn!(bot_id = %bot_id, "Bot not found");
                return;
            }
        };

        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);
        let pe_cmd_tx = self.pe_cmd_tx.clone();
        let pe_event_rx = self.pe_event_rx.resubscribe();
        let event_tx = self.event_tx.clone();
        let db = self.db.clone();
        let strategy_engine = self.strategy_engine.clone();
        let ai_service = self.ai_service.clone();

        let handle = tokio::spawn(async move {
            let mut worker = GridWorker::new(bot, db, strategy_engine, ai_service, pe_cmd_tx, pe_event_rx, event_tx);
            worker.run(shutdown_rx).await;
        });

        self.workers.insert(bot_id, handle);
        self.shutdown_txs.insert(bot_id, shutdown_tx);

        // 更新数据库状态
        sqlx::query(
            "UPDATE qd_grid_bots SET status = 'running', started_at = NOW(), updated_at = NOW() WHERE id = $1",
        )
        .bind(bot_id)
        .execute(&self.db)
        .await
        .ok();

        let _ = self
            .event_tx
            .send(GridEvent::BotStarted { bot_id });
        info!(bot_id = %bot_id, "Grid bot started");
    }

    async fn stop_bot(&mut self, bot_id: Uuid, reason: &str) {
        // 撤销所有挂单
        let _ = self
            .pe_cmd_tx
            .send(EngineCommand::CancelAllOrders {
                position_id: None,
                symbol: None, // TODO: 传入具体 symbol
            })
            .await;

        if let Some(tx) = self.shutdown_txs.remove(&bot_id) {
            let _ = tx.send(()).await;
        }
        if let Some(handle) = self.workers.remove(&bot_id) {
            handle.abort();
        }

        sqlx::query(
            "UPDATE qd_grid_bots SET status = 'stopped', stopped_at = NOW(), updated_at = NOW() WHERE id = $1",
        )
        .bind(bot_id)
        .execute(&self.db)
        .await
        .ok();

        let _ = self.event_tx.send(GridEvent::BotStopped {
            bot_id,
            reason: reason.to_string(),
        });
        info!(bot_id = %bot_id, "Grid bot stopped: {}", reason);
    }

    async fn pause_bot(&mut self, bot_id: Uuid) {
        if let Some(tx) = self.shutdown_txs.remove(&bot_id) {
            let _ = tx.send(()).await;
        }
        if let Some(handle) = self.workers.remove(&bot_id) {
            handle.abort();
        }

        sqlx::query("UPDATE qd_grid_bots SET status = 'paused', updated_at = NOW() WHERE id = $1")
            .bind(bot_id)
            .execute(&self.db)
            .await
            .ok();
        info!(bot_id = %bot_id, "Grid bot paused");
    }

    async fn resume_bot(&mut self, bot_id: Uuid) {
        sqlx::query("UPDATE qd_grid_bots SET status = 'stopped', updated_at = NOW() WHERE id = $1")
            .bind(bot_id)
            .execute(&self.db)
            .await
            .ok();
        self.start_bot(bot_id).await;
    }

    async fn delete_bot(&mut self, bot_id: Uuid) {
        self.stop_bot(bot_id, "deleted").await;
        sqlx::query("DELETE FROM qd_grid_bots WHERE id = $1")
            .bind(bot_id)
            .execute(&self.db)
            .await
            .ok();
        info!(bot_id = %bot_id, "Grid bot deleted");
    }

    async fn adjust_grid(&mut self, bot_id: Uuid) {
        // 重新加载 bot 参数并重启 worker
        if let Some(handle) = self.workers.remove(&bot_id) {
            handle.abort();
            info!(bot_id = %bot_id, "Grid bot worker aborted for adjustment");
        }

        let bot: Option<GridBot> =
            sqlx::query_as("SELECT * FROM qd_grid_bots WHERE id = $1")
                .bind(bot_id)
                .fetch_optional(&self.db)
                .await
                .unwrap_or(None);

        if let Some(bot) = bot {
            if bot.status == StrategyStatus::Running {
                self.start_bot(bot_id).await;
            }
        }
    }

    async fn shutdown_all(&mut self) {
        let bot_ids: Vec<Uuid> = self.workers.keys().copied().collect();
        for id in bot_ids {
            self.stop_bot(id, "engine shutdown").await;
        }
    }
}
