use super::common::*;
use crate::bot::semi_automatic_grid::engine::GridEngine;
use crate::bot::semi_automatic_grid::ports::*;
use crate::bot::semi_automatic_grid::types::{GridCommand, GridEvent};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

fn make_engine() -> (GridEngine, tokio::sync::mpsc::Sender<GridCommand>, broadcast::Sender<GridEvent>, Arc<MockEngineStore>) {
    let store = Arc::new(MockEngineStore::new());
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let market_data_provider = Arc::new(MockMarketDataProvider);
    let (engine, cmd_tx, grid_event_tx) = GridEngine::new(
        store.clone(), ai_service, price_provider, order_executor, market_data_provider, event_tx,
    );
    (engine, cmd_tx, grid_event_tx, store)
}

#[tokio::test]
async fn grid_engine_new() {
    let (engine, cmd_tx, _grid_event_tx, _store) = make_engine();
    let _ = cmd_tx;
    drop(engine);
}

#[tokio::test]
async fn grid_engine_subscribe_events() {
    let (engine, _cmd_tx, grid_event_tx, _store) = make_engine();
    let mut rx = engine.subscribe_events();
    let bot_id = Uuid::new_v4();
    let _ = grid_event_tx.send(GridEvent::BotStarted { bot_id });
    let event = rx.recv().await.unwrap();
    match event {
        GridEvent::BotStarted { bot_id: b } => assert_eq!(b, bot_id),
        _ => panic!("Expected BotStarted"),
    }
}

#[tokio::test]
async fn start_bot_success() {
    let (mut engine, _cmd_tx, grid_event_tx, store) = make_engine();
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;
    let mut event_rx = grid_event_tx.subscribe();
    engine.start_bot(bot.id).await;
    let event = event_rx.recv().await.unwrap();
    match event {
        GridEvent::BotStarted { bot_id } => assert_eq!(bot_id, bot.id),
        _ => panic!("Expected BotStarted"),
    }
    let statuses = store.statuses.lock().await;
    assert!(statuses.contains(&(bot.id, "running".to_string())));
}

#[tokio::test]
async fn start_bot_already_running() {
    let (mut engine, _cmd_tx, _grid_event_tx, store) = make_engine();
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;
    engine.start_bot(bot.id).await;
    engine.start_bot(bot.id).await;
    let statuses = store.statuses.lock().await;
    let running_count = statuses.iter().filter(|(_, s)| s == "running").count();
    assert_eq!(running_count, 1);
}

#[tokio::test]
async fn start_bot_not_found() {
    let (mut engine, _cmd_tx, _grid_event_tx, store) = make_engine();
    let fake_id = Uuid::new_v4();
    engine.start_bot(fake_id).await;
    let statuses = store.statuses.lock().await;
    assert!(statuses.is_empty());
}

#[tokio::test]
async fn start_bot_load_error() {
    let store = Arc::new(MockEngineStore::failing());
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (mut engine, _cmd_tx, _grid_event_tx) = GridEngine::new(
        store, ai_service, price_provider, order_executor, Arc::new(MockMarketDataProvider), event_tx,
    );
    let fake_id = Uuid::new_v4();
    engine.start_bot(fake_id).await;
}

#[tokio::test]
async fn stop_bot() {
    let (mut engine, _cmd_tx, grid_event_tx, store) = make_engine();
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;
    let mut event_rx = grid_event_tx.subscribe();
    engine.start_bot(bot.id).await;
    let _ = event_rx.recv().await;
    engine.stop_bot(bot.id, "user requested").await;
    let event = event_rx.recv().await.unwrap();
    match event {
        GridEvent::BotStopped { bot_id, reason } => {
            assert_eq!(bot_id, bot.id);
            assert_eq!(reason, "user requested");
        }
        _ => panic!("Expected BotStopped"),
    }
    let statuses = store.statuses.lock().await;
    assert!(statuses.contains(&(bot.id, "stopped".to_string())));
}

#[tokio::test]
async fn stop_bot_not_running() {
    let (mut engine, _cmd_tx, _grid_event_tx, store) = make_engine();
    let bot_id = Uuid::new_v4();
    engine.stop_bot(bot_id, "not running").await;
    let statuses = store.statuses.lock().await;
    assert!(statuses.contains(&(bot_id, "stopped".to_string())));
}

#[tokio::test]
async fn pause_and_resume() {
    let (mut engine, _cmd_tx, _grid_event_tx, store) = make_engine();
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;
    engine.start_bot(bot.id).await;
    engine.pause_bot(bot.id).await;
    let statuses = store.statuses.lock().await;
    assert!(statuses.contains(&(bot.id, "paused".to_string())));
    drop(statuses);
    engine.resume_bot(bot.id).await;
    let statuses = store.statuses.lock().await;
    let last_status = statuses.iter().rev().find(|(id, _)| *id == bot.id);
    assert_eq!(last_status, Some(&(bot.id, "running".to_string())));
}

#[tokio::test]
async fn delete_bot() {
    let (mut engine, _cmd_tx, _grid_event_tx, store) = make_engine();
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;
    engine.start_bot(bot.id).await;
    engine.delete_bot(bot.id).await;
    let deleted = store.deleted_bots.lock().await;
    assert!(deleted.contains(&bot.id));
}

#[tokio::test]
async fn delete_bot_not_running() {
    let (mut engine, _cmd_tx, _grid_event_tx, store) = make_engine();
    let bot_id = Uuid::new_v4();
    engine.delete_bot(bot_id).await;
    let deleted = store.deleted_bots.lock().await;
    assert!(deleted.contains(&bot_id));
}

#[tokio::test]
async fn adjust_grid() {
    let (mut engine, _cmd_tx, grid_event_tx, store) = make_engine();
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;
    let mut event_rx = grid_event_tx.subscribe();
    engine.start_bot(bot.id).await;
    let _ = event_rx.recv().await;
    engine.adjust_grid(bot.id).await;
    let event = event_rx.recv().await.unwrap();
    match event {
        GridEvent::BotStarted { bot_id } => assert_eq!(bot_id, bot.id),
        _ => panic!("Expected BotStarted after adjust_grid"),
    }
}

#[tokio::test]
async fn adjust_grid_bot_not_found() {
    let (mut engine, _cmd_tx, _grid_event_tx, _store) = make_engine();
    let fake_id = Uuid::new_v4();
    engine.adjust_grid(fake_id).await;
}

#[tokio::test]
async fn shutdown_all() {
    let (mut engine, _cmd_tx, _grid_event_tx, store) = make_engine();
    let bot1 = make_bot_config();
    let bot2 = make_bot_config();
    store.add_bot(bot1.clone()).await;
    store.add_bot(bot2.clone()).await;
    engine.start_bot(bot1.id).await;
    engine.start_bot(bot2.id).await;
    engine.shutdown_all().await;
    let statuses = store.statuses.lock().await;
    assert!(statuses.contains(&(bot1.id, "stopped".to_string())));
    assert!(statuses.contains(&(bot2.id, "stopped".to_string())));
}

#[tokio::test]
async fn restore_running_bots() {
    let store = Arc::new(MockEngineStore::new());
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (mut engine, _cmd_tx, grid_event_tx) = GridEngine::new(
        store.clone(), ai_service, price_provider, order_executor, Arc::new(MockMarketDataProvider), event_tx,
    );
    let mut event_rx = grid_event_tx.subscribe();
    engine.restore_running_bots().await;
    let event = event_rx.recv().await.unwrap();
    match event {
        GridEvent::BotStarted { bot_id } => assert_eq!(bot_id, bot.id),
        _ => panic!("Expected BotStarted from restore"),
    }
    let statuses = store.statuses.lock().await;
    assert!(statuses.contains(&(bot.id, "running".to_string())));
}

#[tokio::test]
async fn restore_running_bots_empty() {
    let store = Arc::new(MockEngineStore::new());
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (mut engine, _cmd_tx, _grid_event_tx) = GridEngine::new(
        store.clone(), ai_service, price_provider, order_executor, Arc::new(MockMarketDataProvider), event_tx,
    );
    engine.restore_running_bots().await;
    let statuses = store.statuses.lock().await;
    assert!(statuses.is_empty());
}

#[tokio::test]
async fn restore_running_bots_load_error() {
    let store = Arc::new(MockEngineStore::failing());
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (mut engine, _cmd_tx, _grid_event_tx) = GridEngine::new(
        store, ai_service, price_provider, order_executor, Arc::new(MockMarketDataProvider), event_tx,
    );
    engine.restore_running_bots().await;
}

// ── 多 bot 同时运行 ──

#[tokio::test]
async fn start_multiple_bots() {
    let (mut engine, _cmd_tx, grid_event_tx, store) = make_engine();
    let bot1 = make_bot_config();
    let bot2 = make_bot_config();
    let bot3 = make_bot_config();
    store.add_bot(bot1.clone()).await;
    store.add_bot(bot2.clone()).await;
    store.add_bot(bot3.clone()).await;

    engine.start_bot(bot1.id).await;
    engine.start_bot(bot2.id).await;
    engine.start_bot(bot3.id).await;

    let statuses = store.statuses.lock().await;
    let running_count = statuses.iter().filter(|(_, s)| s == "running").count();
    assert_eq!(running_count, 3);
}

#[tokio::test]
async fn stop_one_bot_others_still_running() {
    let (mut engine, _cmd_tx, grid_event_tx, store) = make_engine();
    let bot1 = make_bot_config();
    let bot2 = make_bot_config();
    store.add_bot(bot1.clone()).await;
    store.add_bot(bot2.clone()).await;

    engine.start_bot(bot1.id).await;
    engine.start_bot(bot2.id).await;

    let mut event_rx = grid_event_tx.subscribe();
    let _ = event_rx.recv().await;
    let _ = event_rx.recv().await;

    engine.stop_bot(bot1.id, "user requested").await;

    let statuses = store.statuses.lock().await;
    let bot1_last = statuses.iter().rev().find(|(id, _)| *id == bot1.id);
    let bot2_last = statuses.iter().rev().find(|(id, _)| *id == bot2.id);
    assert_eq!(bot1_last, Some(&(bot1.id, "stopped".to_string())));
    assert_eq!(bot2_last, Some(&(bot2.id, "running".to_string())));
}

// ── stop_bot 发送 BotStopped 事件 ──

#[tokio::test]
async fn stop_bot_emits_event() {
    let (mut engine, _cmd_tx, grid_event_tx, store) = make_engine();
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;
    let mut event_rx = grid_event_tx.subscribe();
    engine.start_bot(bot.id).await;
    let _ = event_rx.recv().await;

    engine.stop_bot(bot.id, "manual stop").await;

    let event = event_rx.recv().await.unwrap();
    match event {
        GridEvent::BotStopped { bot_id, reason } => {
            assert_eq!(bot_id, bot.id);
            assert_eq!(reason, "manual stop");
        }
        _ => panic!("Expected BotStopped"),
    }
}

// ── delete_bot 发送 BotStopped 事件 ──

#[tokio::test]
async fn delete_bot_emits_stopped_event() {
    let (mut engine, _cmd_tx, grid_event_tx, store) = make_engine();
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;
    let mut event_rx = grid_event_tx.subscribe();
    engine.start_bot(bot.id).await;
    let _ = event_rx.recv().await;

    engine.delete_bot(bot.id).await;

    let event = event_rx.recv().await.unwrap();
    match event {
        GridEvent::BotStopped { bot_id, reason } => {
            assert_eq!(bot_id, bot.id);
            assert_eq!(reason, "deleted");
        }
        _ => panic!("Expected BotStopped from delete"),
    }
}

// ── restore_running_bots 多个 bot ──

#[tokio::test]
async fn restore_running_bots_multiple() {
    let store = Arc::new(MockEngineStore::new());
    let bot1 = make_bot_config();
    let bot2 = make_bot_config();
    store.add_bot(bot1.clone()).await;
    store.add_bot(bot2.clone()).await;

    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (mut engine, _cmd_tx, grid_event_tx) = GridEngine::new(
        store.clone(), ai_service, price_provider, order_executor, Arc::new(MockMarketDataProvider), event_tx,
    );
    let mut event_rx = grid_event_tx.subscribe();
    engine.restore_running_bots().await;

    let event1 = event_rx.recv().await.unwrap();
    let event2 = event_rx.recv().await.unwrap();
    let mut restored_ids = vec![];
    for event in [event1, event2] {
        match event {
            GridEvent::BotStarted { bot_id } => restored_ids.push(bot_id),
            _ => panic!("Expected BotStarted"),
        }
    }
    assert!(restored_ids.contains(&bot1.id));
    assert!(restored_ids.contains(&bot2.id));
}

// ── resume_bot 重新启动 ──

#[tokio::test]
async fn resume_bot_starts_worker_again() {
    let (mut engine, _cmd_tx, grid_event_tx, store) = make_engine();
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;

    engine.start_bot(bot.id).await;
    engine.pause_bot(bot.id).await;

    let mut event_rx = grid_event_tx.subscribe();
    engine.resume_bot(bot.id).await;

    let event = event_rx.recv().await.unwrap();
    match event {
        GridEvent::BotStarted { bot_id } => assert_eq!(bot_id, bot.id),
        _ => panic!("Expected BotStarted from resume"),
    }
}

// ── adjust_grid 重新启动 bot ──

#[tokio::test]
async fn adjust_grid_restarts_bot_with_new_params() {
    let (mut engine, _cmd_tx, grid_event_tx, store) = make_engine();
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;

    let mut event_rx = grid_event_tx.subscribe();
    engine.start_bot(bot.id).await;
    let _ = event_rx.recv().await;

    engine.adjust_grid(bot.id).await;

    let event = event_rx.recv().await.unwrap();
    match event {
        GridEvent::BotStarted { bot_id } => assert_eq!(bot_id, bot.id),
        _ => panic!("Expected BotStarted after adjust"),
    }
}

// ── shutdown_all 清理所有 workers ──

#[tokio::test]
async fn shutdown_all_sends_stopped_events() {
    let (mut engine, _cmd_tx, grid_event_tx, store) = make_engine();
    let bot1 = make_bot_config();
    let bot2 = make_bot_config();
    store.add_bot(bot1.clone()).await;
    store.add_bot(bot2.clone()).await;

    engine.start_bot(bot1.id).await;
    engine.start_bot(bot2.id).await;

    let mut event_rx = grid_event_tx.subscribe();
    engine.shutdown_all().await;

    let mut stopped_ids = vec![];
    for _ in 0..2 {
        match event_rx.recv().await.unwrap() {
            GridEvent::BotStopped { bot_id, reason } => {
                assert_eq!(reason, "engine shutdown");
                stopped_ids.push(bot_id);
            }
            _ => {}
        }
    }
    assert!(stopped_ids.contains(&bot1.id));
    assert!(stopped_ids.contains(&bot2.id));
}

// ── stop_bot 发送 CancelAllOrders ──

#[tokio::test]
async fn stop_bot_sends_cancel_all_orders() {
    let store = Arc::new(MockEngineStore::new());
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (mut engine, _cmd_tx, _grid_event_tx) = GridEngine::new(
        store.clone(), ai_service, price_provider, order_executor.clone(), Arc::new(MockMarketDataProvider), event_tx,
    );
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;

    engine.start_bot(bot.id).await;
    engine.stop_bot(bot.id, "test").await;

    let commands = order_executor.commands().await;
    let cancel_cmd = commands.iter().find(|c| {
        matches!(c, GridOrderCommand::CancelAllOrders { .. })
    });
    assert!(cancel_cmd.is_some(), "stop_bot should send CancelAllOrders");
}
