use super::common::*;
use crate::bot::semi_automatic_grid::engine::GridEngine;
use crate::bot::semi_automatic_grid::ports::*;
use crate::bot::semi_automatic_grid::types::{GridCommand, GridEvent};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

#[tokio::test]
async fn grid_engine_new() {
    let store = Arc::new(MockEngineStore::new());
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (engine, cmd_tx, grid_event_tx) = GridEngine::new(
        store, ai_service, price_provider, order_executor, event_tx,
    );
    let bot_id = Uuid::new_v4();
    cmd_tx.send(GridCommand::Shutdown).await.unwrap();
    let _rx = grid_event_tx.subscribe();
    drop(engine);
}

#[tokio::test]
async fn grid_engine_subscribe_events() {
    let store = Arc::new(MockEngineStore::new());
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (engine, _cmd_tx, grid_event_tx) = GridEngine::new(
        store, ai_service, price_provider, order_executor, event_tx,
    );
    let mut rx = engine.subscribe_events();
    let bot_id = Uuid::new_v4();
    let _ = grid_event_tx.send(GridEvent::BotStarted { bot_id });
    let event = rx.recv().await.unwrap();
    match event {
        GridEvent::BotStarted { bot_id: b } => assert_eq!(b, bot_id),
        _ => panic!("Expected BotStarted"),
    }
    drop(engine);
}

#[tokio::test]
async fn start_bot_success() {
    let store = Arc::new(MockEngineStore::new());
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (mut engine, _cmd_tx, grid_event_tx) = GridEngine::new(
        store.clone(), ai_service, price_provider, order_executor, event_tx,
    );
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
    let store = Arc::new(MockEngineStore::new());
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (mut engine, _cmd_tx, _grid_event_tx) = GridEngine::new(
        store.clone(), ai_service, price_provider, order_executor, event_tx,
    );
    engine.start_bot(bot.id).await;
    engine.start_bot(bot.id).await;
    let statuses = store.statuses.lock().await;
    let running_count = statuses.iter().filter(|(_, s)| s == "running").count();
    assert_eq!(running_count, 1);
}

#[tokio::test]
async fn start_bot_not_found() {
    let store = Arc::new(MockEngineStore::new());
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (mut engine, _cmd_tx, _grid_event_tx) = GridEngine::new(
        store.clone(), ai_service, price_provider, order_executor, event_tx,
    );
    let fake_id = Uuid::new_v4();
    engine.start_bot(fake_id).await;
    let statuses = store.statuses.lock().await;
    assert!(statuses.is_empty());
}

#[tokio::test]
async fn stop_bot() {
    let store = Arc::new(MockEngineStore::new());
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (mut engine, _cmd_tx, grid_event_tx) = GridEngine::new(
        store.clone(), ai_service, price_provider, order_executor, event_tx,
    );
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
async fn pause_and_resume() {
    let store = Arc::new(MockEngineStore::new());
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (mut engine, _cmd_tx, _grid_event_tx) = GridEngine::new(
        store.clone(), ai_service, price_provider, order_executor, event_tx,
    );
    engine.start_bot(bot.id).await;
    engine.pause_bot(bot.id).await;
    let statuses = store.statuses.lock().await;
    assert!(statuses.contains(&(bot.id, "paused".to_string())));
    drop(statuses);
    engine.resume_bot(bot.id).await;
    let statuses = store.statuses.lock().await;
    assert!(statuses.contains(&(bot.id, "running".to_string())));
    drop(statuses);
    engine.stop_bot(bot.id, "test cleanup").await;
}

#[tokio::test]
async fn delete_bot() {
    let store = Arc::new(MockEngineStore::new());
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (mut engine, _cmd_tx, _grid_event_tx) = GridEngine::new(
        store.clone(), ai_service, price_provider, order_executor, event_tx,
    );
    engine.start_bot(bot.id).await;
    engine.delete_bot(bot.id).await;
    let deleted = store.deleted_bots.lock().await;
    assert!(deleted.contains(&bot.id));
}

#[tokio::test]
async fn adjust_grid() {
    let store = Arc::new(MockEngineStore::new());
    let bot = make_bot_config();
    store.add_bot(bot.clone()).await;
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (mut engine, _cmd_tx, grid_event_tx) = GridEngine::new(
        store.clone(), ai_service, price_provider, order_executor, event_tx,
    );
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
async fn shutdown_all() {
    let store = Arc::new(MockEngineStore::new());
    let bot1 = make_bot_config();
    let bot2 = make_bot_config();
    store.add_bot(bot1.clone()).await;
    store.add_bot(bot2.clone()).await;
    let order_executor = Arc::new(MockOrderExecutor::new());
    let (event_tx, _) = broadcast::channel(16);
    let ai_service = make_mock_ai_service();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let (mut engine, _cmd_tx, _grid_event_tx) = GridEngine::new(
        store.clone(), ai_service, price_provider, order_executor, event_tx,
    );
    engine.start_bot(bot1.id).await;
    engine.start_bot(bot2.id).await;
    engine.shutdown_all().await;
    let statuses = store.statuses.lock().await;
    assert!(statuses.contains(&(bot1.id, "stopped".to_string())));
    assert!(statuses.contains(&(bot2.id, "stopped".to_string())));
}
