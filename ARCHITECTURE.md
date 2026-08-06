# VIRS 项目架构

## 目录结构

```
virs/
├── crates/
│   ├── infra/              ← 基建层：通用技术基础设施
│   │   ├── virs-error/     错误类型（VirsError / BotError / ExchangeError / ApiError）
│   │   ├── virs-task/      tokio 任务调度（spawn / spawn_periodic / TaskHandle）
│   │   ├── virs-utils/     认证 + 加密（JWT / PBKDF2 / AES-GCM）
│   │   ├── virs-config/    应用配置（AppConfig / TimeConfig / SerdeHex）
│   │   └── virs-ws/        WebSocket 连接管理器（自动重连 / 心跳 / 多路订阅）
│   │
│   ├── core/               ← 基础包层：领域类型 + 纯计算 + 协议适配
│   │   ├── virs-type/      全领域类型定义（ExchangePe trait / Position / Order / Kline）
│   │   ├── virs-indicator/ 技术指标计算（SMA / EMA / RSI / MACD / ATR / Bollinger）
│   │   └── virs-ccxt/      交易所协议适配器（Binance REST / WS / 签名 / 事件解析）
│   │
│   ├── domain/             ← 业务包层：交易业务逻辑
│   │   └── virs-exchange/  Paper 撮合引擎 + PaperModeExchange 路由 + Exchanges 注册表
│   │
│   └── services/           ← 服务层：运行时服务
│       ├── virs-market/        行情服务（KlineEngine / OrderBookEngine / 缓存 / 聚合）
│       ├── virs-position/      持仓服务（PositionEngine / 持久化）
│       ├── virs-api/           API 服务（Axum 路由 / 认证 / CRUD / WebSocket 推送）
│       ├── virs-trading-bot/   交易 Bot（AutoEngine / AI 决策 / 风控 / 下单执行）
│       └── virs-tactical-bot/  策略引擎（定时评估 / LLM 优化 / 热切换）
│
├── app/
│   └── virs-app/           ← 应用层：组合根 + main 入口
│
└── frontend/               ← 前端（React / TypeScript）
```

## 分层职责

### 基建层 `crates/infra/`

通用技术基础设施，不含任何交易领域知识。任何项目都可以复用。

| crate | 职责 | 关键类型 |
|-------|------|---------|
| `virs-error` | 统一错误类型，支持分类（Exchange/Bot/Api）和上下文链 | `VirsError`, `BotError`, `ExchangeError` |
| `virs-task` | tokio 异步任务调度，支持周期执行和优雅停止 | `spawn()`, `spawn_periodic()`, `TaskHandle` |
| `virs-utils` | 用户认证（JWT 签发/验证）和数据加密（PBKDF2 / AES-GCM） | `create_token()`, `verify_token()`, `encrypt()` |
| `virs-config` | 应用配置解析，从环境变量加载 | `AppConfig`, `TimeConfig` |
| `virs-ws` | WebSocket 连接管理器，支持自动重连、心跳、多路订阅 | `WsManager`, `WsSubscription` |

**依赖方向：** 不依赖任何其他 VIRS crate（`virs-error` 除外，部分 crate 依赖它）。

### 基础包层 `crates/core/`

领域类型定义、纯数学计算和协议适配。定义了系统的"语言"，但不包含交易决策逻辑。

| crate | 职责 | 关键类型 |
|-------|------|---------|
| `virs-type` | 全领域类型定义：ExchangePe trait、Position、Order、Kline、Ticker、FundingRate 等 | `ExchangePe`, `Position`, `Order`, `Kline`, `MarketInfo` |
| `virs-indicator` | 技术指标计算：原子指标（SMA/EMA/RSI/MACD/ATR/Bollinger）、衍生指标（EMA 交叉、ATR 百分比等） | `IndicatorSet`, `IndicatorSpec`, `compute()` |
| `virs-ccxt` | 交易所协议适配器：Binance REST API 调用、WebSocket 客户端、HMAC-SHA256 签名、事件解析 | `BinanceExchange`, `create_exchange()`, `create_kline_ws()` |

**设计原则：**
- `virs-type` 定义 `ExchangePe` trait，是所有交易所实现的抽象接口
- `virs-ccxt` 像"数据库驱动"——知道 Binance 的线协议（API 端点、字段名、事件结构），但不做业务决策
- `virs-ccxt` 的 `create_exchange()` 返回 `Box<dyn ExchangePe>`，调用方获得 trait object

**依赖方向：** 依赖 infra 层，不依赖 domain/services。

### 业务包层 `crates/domain/`

交易业务逻辑：模拟交易引擎和交易所路由。

| crate | 职责 | 关键类型 |
|-------|------|---------|
| `virs-exchange` | Paper 撮合引擎（模拟下单/持仓/保证金/P&L）+ PaperModeExchange 路由层（公共数据→real，私有数据→paper）+ Exchanges 注册表 | `PaperExchangeAdapter`, `PaperModeExchange`, `Exchanges` |

**PaperModeExchange 路由表：**

| 路由目标 | 方法 |
|---------|------|
| → real | get_ticker, get_klines, get_klines_range, get_funding_rate, get_symbols, get_min_qty, ping, get_api_restrictions, get_position_mode, create_listen_key, create_kline_ws, create_orderbook_ws |
| → paper | get_balance, get_positions, place_order, cancel_order, cancel_all_orders, subscribe_order_updates, on_price_tick, restore_positions |
| → both | set_leverage（先 real 后 paper） |

**依赖方向：** 依赖 infra + core 层，不依赖 services。

### 服务层 `crates/services/`

运行时服务，每个 crate 是一个独立的服务模块。

| crate | 职责 | 关键类型 |
|-------|------|---------|
| `virs-market` | 行情服务：K 线引擎（拉取/补缺/缓存）、订单簿引擎、聚合器 | `KlineEngine`, `OrderBookEngine` |
| `virs-position` | 持仓服务：持仓引擎（开/平仓、P&L 更新）、持久化 | `PositionEngine` |
| `virs-api` | API 服务：Axum 路由、JWT 认证、CRUD 处理器、WebSocket 推送 | `AppState`, `router()`, 各 `handlers/` |
| `virs-trading-bot` | 交易 Bot：AutoEngine（AI 决策循环）、风控、下单执行、worker | `AutoEngine`, `AutoWorker` |
| `virs-tactical-bot` | 策略引擎：定时评估策略绩效、LLM 优化 prompt、热切换通知 | `StrategyEngine`, `StrategyEvaluator`, `StrategyOptimizer` |

**依赖方向：** 依赖 infra + core + domain 层。

### 应用层 `app/virs-app/`

组合根：负责把各层组件组装在一起，是唯一知道所有具体实现的层。

| 职责 | 说明 |
|------|------|
| `main.rs` | 程序入口，初始化数据库、交易所、行情服务、API 服务器 |
| `engine_manager.rs` | 交易所实例管理：调用 `virs_ccxt::create_exchange()` 创建实例 → 注册到 `Exchanges` → 构建 `PaperModeExchange`（paper 模式） |

**依赖方向：** 依赖所有层。

## 依赖方向总览

```
infra          ← core          ← domain         ← services        ← app
(5 crates)      (3 crates)      (1 crate)        (5 crates)        (1 crate)

virs-error      virs-type       virs-exchange    virs-market       virs-app
virs-task       virs-indicator                   virs-position
virs-utils      virs-ccxt                        virs-api
virs-config                                      virs-trading-bot
virs-ws                                          virs-tactical-bot
```

**规则：依赖只向下不向上。** 例如 infra 不依赖 core，core 不依赖 domain。

## ExchangePe Trait 方法分类

```
ExchangePe (21 方法 + 2 默认回调 + 2 默认 WS 工厂)
│
├── 元信息 (2)
│   ├── name() → &str
│   └── market_type() → MarketType
│
├── 公共数据 (12) — PaperModeExchange 路由到 real
│   ├── get_ticker, get_klines, get_klines_range
│   ├── get_funding_rate, get_symbols, get_min_qty
│   ├── ping, get_api_restrictions
│   ├── get_position_mode, create_listen_key
│   └── create_kline_ws, create_orderbook_ws (默认 NotSupported)
│
├── 私有数据 (8) — PaperModeExchange 路由到 paper
│   ├── get_balance, get_positions
│   ├── place_order, cancel_order, cancel_all_orders
│   ├── subscribe_order_updates
│   └── on_price_tick, restore_positions (默认空实现)
│
└── 双写 (1) — PaperModeExchange 调用 real + paper
    └── set_leverage
```

## ExchangePe 三个实现者

| 实现者 | 位置 | 性质 |
|--------|------|------|
| `BinanceExchange` | `core/virs-ccxt` | 真实交易所连接，全部方法实际实现 |
| `PaperExchangeAdapter` | `domain/virs-exchange` | 模拟交易引擎，公共数据返回 NotSupported |
| `PaperModeExchange` | `domain/virs-exchange` | 路由层，公共数据→real，私有数据→paper |

## StrategyEngine 工作流程

```
每个分析周期（默认 1 小时）：
  1. 遍历 PromptLoader 中所有 Auto 策略
  2. StrategyEvaluator.evaluate() ← TradeHistoryProvider.query_trades()
  3. 判断 needs_optimization（综合评分 < 0.45 且交易数 >= 10）
  4. StrategyOptimizer.optimize() → LLM 分析弱点 + 输出改进 prompt
  5. save_template() 写入磁盘（版本号递增）
  6. PromptLoader.upsert() 热切换内存缓存
  7. watch::channel 通知 virs-trading-bot
```

## "binance" 概念分布

| 位置 | 性质 | 合理? |
|------|------|:---:|
| `core/virs-ccxt/src/adapter/binance/` | 协议实现（URL、请求格式、响应解析） | 是 |
| `BinanceExchange::name()` 返回 `"binance"` | 运行时标识符 | 是 |
| `create_exchange("binance" => ...)` | 工厂选择 | 是 |
| 前端 ConfigureExchange UI 文案 | 用户需要知道是币安 | 是 |
| 业务层（services/）源码 | — | 不出现 |
