# Semi-Automatic Grid 网格交易模块使用文档

## 概述

`semi_automatic_grid` 是一个半自动网格交易引擎，采用六边形架构（Hexagonal / Ports & Adapters），核心业务逻辑与外部依赖完全解耦。支持 LLM 驱动的动态调整和规则回退策略。

## 快速开始（单一入口）

### 入口方法：`GridEngine::new()`

整个模块的对外使用入口集中在 `GridEngine::new()`，返回 `(GridEngine, CommandSender, EventSender)` 三元组：

```rust
use bot::semi_automatic_grid::GridEngine;

let (mut engine, cmd_tx, event_tx) = GridEngine::new(
    store,              // Arc<dyn GridStore>
    ai_service,         // Arc<GridAiService>
    price_provider,     // Arc<dyn PriceProvider>
    order_executor,     // Arc<dyn OrderExecutor>
    market_data_provider, // Arc<dyn MarketDataProvider>
    order_event_tx,     // broadcast::Sender<OrderEvent>
    kline_engine,       // Option<Arc<KlineEngine>>
);

// 启动引擎主循环
tokio::spawn(async move { engine.run().await });

// 发送控制命令
cmd_tx.send(GridCommand::StartBot { bot_id }).await?;
```

### 最小化组装

在 `main.rs` 中只需组装 7 个依赖组件即可启动：

```rust
// 1. 数据存储
let store = Arc::new(adapters::PgGridStore::new(db_pool));

// 2. 价格提供者
let price_provider = Arc::new(adapters::ExchangePriceProvider::new(exchange_registry)
    .with_kline_engine(kline_engine));

// 3. 市场数据提供者
let market_data_provider = Arc::new(adapters::ExchangeMarketDataProvider::new(exchange_registry)
    .with_kline_engine(kline_engine));

// 4. 订单执行器（含 Paper 交易切换）
let real_executor = Arc::new(adapters::PeOrderExecutor::new(pe_cmd_tx, exchange_registry));
let paper_executor = Arc::new(trading::paper::PaperOrderExecutor::new(grid_event_tx));
let order_executor = Arc::new(adapters::SwitchableOrderExecutor::new(real_executor, paper_executor));

// 5. AI 服务
let credential_store = Box::new(adapters::PgCredentialStore::new(db_pool, encryption_key));
let llm_resolver = Box::new(adapters::DefaultLlmResolver::new(ai_config));
let ai_service = Arc::new(ai::GridAiService::new(llm_resolver, credential_store));

// 6. 订单事件广播
let (order_event_tx, order_event_rx) = broadcast::channel(256);

// 7. 创建引擎
let (mut engine, cmd_tx, event_tx) = GridEngine::new(
    store, ai_service, price_provider, order_executor,
    market_data_provider, order_event_tx, Some(kline_engine),
);
```

## 架构

```
semi_automatic_grid/
├── mod.rs          模块入口，导出 GridEngine
├── engine.rs       GridEngine - 管理所有 bot 生命周期
├── worker/         GridWorker - 单个 bot 的运行时
│   ├── mod.rs      Worker 结构体和构造函数
│   ├── state.rs    主运行循环和价格获取
│   ├── orders.rs   订单管理（挂单、过滤、方向参数化）
│   ├── events.rs   事件处理（成交、持仓更新、盈亏计算）
│   └── adjust.rs   LLM 决策和网格调整
├── ai.rs           GridAiService - LLM 调用和决策解析
├── ports.rs        Port trait 定义（6 个核心接口）
├── types.rs        数据类型（GridLevel, GridCommand, GridEvent 等）
├── adapters.rs     Adapter 实现（PostgreSQL, Exchange, PE 桥接）
└── utils/          工具函数
    ├── indicators.rs  多周期技术指标计算
    ├── levels.rs      网格层级计算
    ├── holdings.rs    持仓更新和盈亏计算
    ├── prompt.rs      LLM Prompt 模板渲染
    └── ai_client.rs   LLM API 调用
```

## 核心 Port 接口

| Port | 职责 | Adapter 实现 |
|------|------|-------------|
| `GridStore` | Bot 配置、交易记录、统计持久化 | `PgGridStore` |
| `PriceProvider` | 获取实时价格 | `ExchangePriceProvider` |
| `MarketDataProvider` | 获取市场快照和账户余额 | `ExchangeMarketDataProvider` |
| `OrderExecutor` | 发送下单/撤单/平仓命令 | `PeOrderExecutor` / `SwitchableOrderExecutor` |
| `CredentialStore` | 加载用户 LLM API 凭证 | `PgCredentialStore` |
| `LlmProviderResolver` | 解析 LLM 连接参数 | `DefaultLlmResolver` |

## 命令与事件

### 控制命令（通过 `cmd_tx` 发送）

| 命令 | 说明 |
|------|------|
| `StartBot { bot_id }` | 启动指定 bot |
| `StopBot { bot_id }` | 停止指定 bot |
| `PauseBot { bot_id }` | 暂停（取消挂单，保留状态） |
| `ResumeBot { bot_id }` | 恢复运行 |
| `DeleteBot { bot_id, close_position }` | 删除 bot（可选平仓） |
| `AdjustGrid { bot_id }` | 触发重新加载配置并调整 |
| `Shutdown` | 关闭整个引擎 |

### 事件广播（通过 `event_tx` 接收）

| 事件 | 说明 |
|------|------|
| `BotStarted` | Bot 启动完成 |
| `BotStopped` | Bot 停止（含原因） |
| `BotError` | 运行错误 |
| `GridAdjusted` | 网格参数调整完成 |
| `GridFilled` | 网格层成交 |
| `GridTradeClosed` | 买卖配对完成（含 PnL） |
| `PriceUpdate` | 价格更新 |
| `StatusUpdate` | 完整状态快照推送 |
| `AnalysisLog` | AI 分析日志 |

## Worker 运行循环

每个 bot 在独立的 tokio 任务中运行 `GridWorker::run()`，主循环通过 `select!` 处理 5 类事件：

1. **Shutdown 信号** → 优雅退出
2. **Adjust 信号** → 重新加载配置并调整网格
3. **Price Tick**（5 秒）→ 检查挂单条件
4. **LLM 决策**（可配置间隔）→ AI 分析并执行决策
5. **订单事件** → 处理成交、更新持仓

## LLM 决策流程

```
on_llm_decision()
  → build_llm_prompt()        获取市场快照 + 渲染 prompt
  → ai_service.grid_decision() 调用 LLM API
  → handle_llm_result()       记录日志 / 回退规则
  → execute_decision()        执行决策（run/pause/adjust/reduce/hold）
```

当 LLM 不可用时，自动回退到规则策略：
- 价格突破上界 2% → 暂停
- 价格跌破下界 2% → 暂停
- 暂停中价格回到区间内 → 恢复

## 网格层级计算

```
calculate_levels(bot_config)
  → extract_level_params()     验证参数，计算间距/利润因子/中间价
  → determine_level_side()     LLM 配置优先 → 价格与中间价比较回退
  → compute_buy_sell_prices()  buy 层: buy=price, sell=price*factor
                               sell 层: buy=price/factor, sell=price
  → compute_quantity()         quantity = amount_per_grid / price
```

## 持仓与盈亏

- **持仓更新**：`apply_fill_to_level()` 根据层级方向和成交方向更新持仓量和加权均价
- **已实现盈亏**：`calculate_fill_pnl()` 仅在平仓方向成交时计算
- **未实现盈亏**：`compute_unrealized_pnl()` 基于当前价格与均价的差值乘以持仓量
- **连续亏损**：`update_consecutive_losses()` 用于 AI 决策和风控

## Paper 交易

通过 `SwitchableOrderExecutor` 在运行时切换真实/Paper 模式。Paper 执行器模拟成交，不发送到交易所。

## PE 事件桥接

`adapters::convert_pe_event()` 将 Position Engine 的 `EngineEvent` 转换为网格模块的 `OrderEvent`，在独立的 tokio 协程中转发。

## 数据库表

| 表名 | 用途 |
|------|------|
| `qd_grid_bots` | Bot 配置和运行状态 |
| `qd_grid_trades` | 交易记录流水 |
| `qd_grid_analysis_logs` | AI 分析日志 |
| `qd_ai_credentials` | 用户 LLM API 凭证（加密存储） |
