# Kline Engine Architecture

## 1. Module Decomposition

```
kline/
├── types.rs       — Pure data types, KlineWsClient trait, utility functions
├── aggregator.rs  — Stateless 1m → higher-TF aggregation (pure computation)
├── cache.rs       — In-memory multi-TF ring buffer (no I/O, no network)
├── gap.rs         — Gap detection & initial load orchestration (depends on Source trait)
├── ws.rs          — Binance WebSocket client (implements KlineWsClient trait)
├── source.rs      — REST data source adapter (ccxt adapter, implements KlineSource trait)
├── mod.rs         — KlineEngine orchestrator (wires all atoms together)
├── api.rs         — HTTP/WS API layer (Axum handlers, business boundary)
├── spot/
│   ├── mod.rs     — SpotHandler (现货市场逻辑，通过 KlineWsClient trait 注入 WS)
│   └── TEST_REPORT.md
└── perpetual/
    ├── mod.rs     — PerpetualHandler (永续合约市场逻辑，通过 KlineWsClient trait 注入 WS)
    └── TEST_REPORT.md
```

## 2. Dependency Graph

```
api.rs
  └→ mod.rs (KlineEngine)
       ├→ types.rs      (Candle, Timeframe, KlineEvent, KlineWsClient, WsEvent, config, utils)
       ├→ cache.rs      (SymbolCache)          ─→ types.rs
       ├→ aggregator.rs (Aggregator)           ─→ types.rs, cache.rs
       ├→ gap.rs        (GapDetector)          ─→ types.rs, cache.rs, aggregator.rs, KlineSource trait
       ├→ source.rs     (CcxtKlineSource)      ─→ types.rs, ccxt (external)
       ├→ spot/         (SpotHandler)          ─→ types.rs (KlineWsClient trait)
       ├→ perpetual/    (PerpetualHandler)     ─→ types.rs (KlineWsClient trait)
       └→ persistence   (KlinePersistence trait) ─→ types.rs

ws.rs (BinanceWs) ─→ types.rs (implements KlineWsClient trait)
```

## 3. Exchange-Agnostic Design

### 3.1 KlineWsClient Trait

```rust
#[async_trait]
pub trait KlineWsClient: Send + Sync {
    async fn start(&mut self, update_tx: broadcast::Sender<WsEvent>);
    async fn stop(&mut self);
    async fn subscribe(&self, symbol: &str);
    async fn unsubscribe(&self, symbol: &str);
    fn is_running(&self) -> bool;
}
```

- 定义在 `types.rs` 中，与任何交易所无关
- WS 客户端内部处理 symbol 映射（交易所特有格式 → 内部格式）
- `WsCandleUpdate.symbol` 始终返回内部格式的 symbol

### 3.2 交易所适配器注入

KlineEngine 不直接创建 WS 客户端，而是通过构造函数注入：

```rust
KlineEngine::new(config, source, spot_ws, perpetual_ws)
```

在 `main.rs` 中创建具体的交易所 WS 客户端：

```rust
let spot_ws = Arc::new(Mutex::new(BinanceWs::new_spot(proxy)));
let perpetual_ws = Arc::new(Mutex::new(BinanceWs::new_perpetual(proxy)));
let engine = KlineEngine::new(config, source, spot_ws, perpetual_ws);
```

### 3.3 添加新交易所

只需实现 `KlineWsClient` trait：

```rust
pub struct OkxWs { ... }

#[async_trait]
impl KlineWsClient for OkxWs {
    async fn start(&mut self, update_tx: broadcast::Sender<WsEvent>) { ... }
    async fn stop(&mut self) { ... }
    async fn subscribe(&self, symbol: &str) { ... }
    async fn unsubscribe(&self, symbol: &str) { ... }
    fn is_running(&self) -> bool { ... }
}
```

然后在 `main.rs` 中注入即可，无需修改 KlineEngine 代码。

## 4. Market Type Separation

### 4.1 SpotHandler

- 封装现货市场 WS 客户端管理
- `market_type()` 返回 `MarketType::Spot`
- KlineEngine 在 `subscribe(exchange, symbol, MarketType::Spot)` 时委托给 SpotHandler

### 4.2 PerpetualHandler

- 封装永续合约市场 WS 客户端管理
- `market_type()` 返回 `MarketType::Perpetual`
- KlineEngine 在 `subscribe(exchange, symbol, MarketType::Perpetual)` 时委托给 PerpetualHandler

### 4.3 未来扩展

SpotHandler 和 PerpetualHandler 当前接口对称，差异仅在 market_type 和注入的 WS 客户端。
未来可扩展：
- 永续合约：funding rate 数据处理、不同的 symbol 格式
- 现货：不同的 gap 检测阈值
- 各自独立的配置参数

## 5. Atomic Unit Specification

### 5.1 types.rs — Pure Data Layer
**Responsibility**: Define all shared data types, traits, and pure utility functions.
**Isolation**: No dependency on `crate::models`, `crate::ccxt`, or any I/O.
**Key exports**:
- `Candle`, `Timeframe`, `KlineEvent`, `KlineEventType`
- `KlineWsClient` trait (exchange-agnostic WS interface)
- `WsEvent`, `WsCandleUpdate` (WS event types)
- `KlineEngineConfig` (no exchange-specific fields)
- `subscription_key()`, `align_open_time()`
- `BacktestRangeLimit`, `BacktestRangeInfo`, `AllTimeframesData`

### 5.2 aggregator.rs — Stateless Aggregation
**Responsibility**: Aggregate 1m candles into higher timeframes. Pure computation, no side effects.

### 5.3 cache.rs — In-Memory Ring Buffer
**Responsibility**: Store candles per timeframe with bounded capacity. No I/O.

### 5.4 gap.rs — Gap Detection & Initial Load
**Responsibility**: Detect missing candles and orchestrate backfill/initial load.
**Depends on**: `KlineSource` trait (injectable, mockable)

### 5.5 ws.rs — Binance WebSocket Client
**Responsibility**: Maintain WS connection to Binance, parse kline messages.
**Implements**: `KlineWsClient` trait
**I/O boundary**: Not unit-testable without mocking tungstenite.

### 5.6 source.rs — REST Data Source Adapter
**Responsibility**: Fetch klines via ccxt REST API.
**Implements**: `KlineSource` trait
**I/O boundary**: Depends on `crate::ccxt`.

### 5.7 spot/mod.rs — SpotHandler
**Responsibility**: Manage spot market WS subscriptions via KlineWsClient trait.
**Testable via**: MockSpotWs implementing KlineWsClient.

### 5.8 perpetual/mod.rs — PerpetualHandler
**Responsibility**: Manage perpetual market WS subscriptions via KlineWsClient trait.
**Testable via**: MockPerpetualWs implementing KlineWsClient.

### 5.9 mod.rs — KlineEngine Orchestrator
**Responsibility**: Wire all atoms together, manage subscriptions, route WS updates.
**Testable via**: Mock `KlineSource`, verify subscribe/unsubscribe/event flow.

### 5.10 api.rs — HTTP/WS API Layer
**Responsibility**: Expose kline engine via Axum routes.
**Business boundary**: Depends on `AppState`, Axum types.

## 6. Test Strategy

| Module | Test Type | Mock Required | Priority |
|--------|-----------|---------------|----------|
| types.rs | Unit | None | P0 |
| aggregator.rs | Unit | None | P0 |
| cache.rs | Unit | None | P0 |
| gap.rs | Unit | MockKlineSource | P1 |
| spot/mod.rs | Unit | MockSpotWs (KlineWsClient) | P1 |
| perpetual/mod.rs | Unit | MockPerpetualWs (KlineWsClient) | P1 |
| mod.rs | Integration | MockKlineSource + MockWsClient | P1 |
| ws.rs | Integration | Binance testnet | P2 |
| source.rs | Integration | Binance testnet | P2 |
| api.rs | E2E | Test server | P2 |
