# VIRS Position Engine 设计文档

> 独立模块 | 交易所无关 | 通用设计 | PostgreSQL 持久化

---

## 1. 概述

### 1.1 背景与目标

VIRS 当前的策略引擎仅在内存中维护仓位状态（CyclePositionInfo），没有与交易所实际仓位同步的机制，也没有持久化。网格机器人和全自动交易机器人都需要一个完整的仓位管理引擎来支撑实际交易。

本文档设计一个独立的、通用的仓位管理引擎（Position Engine），作为 VIRS 的核心基础设施，服务于所有交易策略。

### 1.2 设计原则

- **独立模块**：位于 `src/position_engine/` 目录，不引用 `src/` 下其他模块的路径
- **交易所无关**：通过注入外部 Exchange 实例实现，不自己调用交易所 API
- **通用设计**：网格、全自动交易、手动交易等场景均可复用
- **多实例并行**：每个引擎实例绑定一个交易所账户，支持同一进程内运行多个引擎实例（不同账户/交易所），也支持跨进程部署
- **持久化**：PostgreSQL 存储仓位、订单、成交记录
- **事件驱动**：通过 channel/callback 模式通知上层业务

---

## 2. 模块结构

### 2.1 目录结构

```
src/position_engine/
├── mod.rs              # 模块入口，导出公开 API
├── config.rs           # 引擎配置（风控参数、同步间隔等）
├── exchange.rs         # 交易所抽象 trait（由外部注入实现）
├── types.rs            # 核心类型（Position, Order, Trade, Side, Status 等）
├── engine.rs           # 引擎主体（状态机、同步循环、命令分发）
├── risk.rs             # 风控模块（仓位上限、回撤保护、强平预警）
├── tracker.rs          # 盈亏追踪（已实现/未实现盈亏、最大回撤）
├── persistence.rs      # 持久化（PostgreSQL 读写）
└── error.rs            # 错误类型定义
```

### 2.2 模块依赖关系

| 模块 | 依赖 | 职责 |
|------|------|------|
| mod.rs | 所有子模块 | 导出 PositionEngine 等公开类型 |
| config.rs | 无 | 引擎配置结构体 |
| exchange.rs | types.rs, error.rs | Exchange trait 定义（不含实现） |
| types.rs | 无 | 核心数据类型定义 |
| engine.rs | 所有子模块 | 引擎主循环、命令分发 |
| risk.rs | types.rs | 风控规则检查 |
| tracker.rs | types.rs | 盈亏计算与统计 |
| persistence.rs | types.rs, config.rs | 数据库 CRUD |
| error.rs | 无 | 统一错误类型 |

---

## 3. 核心类型 (types.rs)

### 3.1 基础枚举

| 枚举 | 值 | 说明 |
|------|-----|------|
| Side | Buy / Sell | 交易方向 |
| PositionSide | Long / Short / Both | 仓位方向 |
| OrderType | Limit / Market / StopMarket / TakeProfitMarket | 订单类型 |
| OrderStatus | Pending → Open → PartiallyFilled → Filled / Canceled / Failed | 订单状态机 |
| PositionStatus | Empty → Opening → Open → Closing → Closed | 仓位状态机 |
| MarketType | Spot / Perpetual | 市场类型 |
| EngineState | Created / Running / Paused / ShuttingDown / Stopped | 引擎运行状态 |

### 3.2 辅助结构体

```rust
/// 行情快照
pub struct Ticker {
    pub symbol: String,
    pub price: f64,           // 最新价
    pub bid: f64,             // 买一价
    pub ask: f64,             // 卖一价
    pub volume_24h: f64,      // 24h 成交量
    pub timestamp: i64,
}

/// 账户余额
pub struct Balance {
    pub asset: String,
    pub free: f64,            // 可用余额
    pub used: f64,            // 已用（挂单占用）
    pub total: f64,           // 总计
}

/// 交易所返回的仓位快照（用于同步比对）
pub struct ExchangePosition {
    pub symbol: String,
    pub side: PositionSide,
    pub size: f64,
    pub entry_price: f64,
    pub leverage: u32,
    pub unrealized_pnl: f64,
    pub liquidation_price: Option<f64>,
}

/// 资金费率
pub struct FundingRate {
    pub symbol: String,
    pub rate: f64,            // 当前费率（如 0.0001 = 0.01%）
    pub next_funding_time: i64,
}

/// WebSocket 成交回报事件
pub enum WsFeedEvent {
    /// 订单状态更新（成交、部分成交、撤销）
    OrderUpdate {
        exchange_order_id: String,
        symbol: String,
        status: OrderStatus,
        filled: f64,
        remaining: f64,
        price: Option<f64>,      // 本次成交价
        amount: Option<f64>,     // 本次成交量
        commission: f64,         // 本次手续费
        timestamp: i64,
    },
    /// WebSocket 连接状态变化
    ConnectionChanged {
        connected: bool,
    },
}
```

### 3.3 Position（仓位）

| 字段 | 类型 | 说明 |
|------|------|------|
| id | Uuid | 仓位唯一标识 |
| engine_id | String | 引擎实例标识（支持多引擎并行） |
| strategy_id | Option\<Uuid\> | 关联的策略 ID |
| exchange | String | 交易所名称 |
| symbol | String | 交易对 |
| side | PositionSide | Long / Short |
| status | PositionStatus | 仓位状态 |
| size | f64 | 持仓数量 |
| entry_price | f64 | **加权平均实际成交价**（非请求价） |
| current_price | f64 | 当前价格 |
| leverage | u32 | 杠杆倍数 |
| margin | f64 | 保证金 |
| unrealized_pnl | f64 | 未实现盈亏 |
| realized_pnl | f64 | 已实现盈亏 |
| stop_loss | Option\<f64\> | 止损价 |
| take_profit | Option\<f64\> | 止盈价 |
| liquidation_price | Option\<f64\> | 强平价 |
| opened_at | DateTime\<Utc\> | 开仓时间 |
| updated_at | DateTime\<Utc\> | 最后更新时间 |
| closed_at | Option\<DateTime\<Utc\>\> | 平仓时间 |
| metadata | serde_json::Value | 扩展字段（网格层级、策略参数等） |

### 3.4 Order（订单）

| 字段 | 类型 | 说明 |
|------|------|------|
| id | Uuid | 本地订单 ID |
| position_id | Uuid | 关联仓位 ID |
| exchange_order_id | Option\<String\> | 交易所订单 ID |
| client_order_id | Option\<String\> | 客户端订单 ID |
| exchange | String | 交易所 |
| symbol | String | 交易对 |
| side | Side | 交易方向 |
| order_type | OrderType | 订单类型 |
| request_price | Option\<f64\> | **请求价格**（Limit 单的委托价） |
| fill_price | Option\<f64\> | **实际加权平均成交价**（未成交时为 None） |
| amount | f64 | 委托数量 |
| filled | f64 | 已成交数量 |
| remaining | f64 | 未成交数量 |
| status | OrderStatus | 订单状态 |
| reduce_only | bool | 是否只减仓 |
| fee | f64 | 手续费 |
| slippage | Option\<f64\> | **滑点**（Market 单：实际成交价 - 下单时 ticker 价） |
| created_at | DateTime\<Utc\> | 创建时间 |
| updated_at | DateTime\<Utc\> | 最后更新时间 |

### 3.5 Trade（成交记录）

| 字段 | 类型 | 说明 |
|------|------|------|
| id | Uuid | 成交记录 ID |
| position_id | Uuid | 关联仓位 ID |
| order_id | Uuid | 关联订单 ID |
| exchange | String | 交易所 |
| symbol | String | 交易对 |
| side | Side | 成交方向 |
| price | f64 | **实际成交价** |
| amount | f64 | **本次成交量** |
| fee | f64 | 手续费 |
| pnl | f64 | 本笔盈亏（平仓时计算） |
| trade_type | String | open / close / partial_close |
| created_at | DateTime\<Utc\> | 成交时间 |

---

## 4. 交易所抽象层 (exchange.rs)

### 4.1 设计思路

Position Engine **不自己实现交易所 API 调用**，而是定义一个 Exchange trait，由外部（主程序）在创建引擎时注入具体实现。

这样做的好处：
- Position Engine 代码中**不出现任何外部模块路径**，保持独立
- 复用已有的 ccxt 适配层（签名、分页、错误处理、类型转换都已实现）
- 新增交易所只需在主程序中写适配器，Position Engine 完全不动

### 4.2 Exchange Trait

```rust
/// Position Engine 定义的交易所抽象
/// 由主程序在创建引擎时注入具体实现（适配 ccxt 的 Binance/Bybit/OKX）
#[async_trait]
pub trait Exchange: Send + Sync {
    fn name(&self) -> &str;
    fn market_type(&self) -> MarketType;

    // 行情数据
    async fn get_ticker(&self, symbol: &str) -> Result<Ticker>;
    async fn get_balance(&self) -> Result<Balance>;
    async fn get_positions(&self, symbol: Option<&str>) -> Result<Vec<ExchangePosition>>;
    async fn get_funding_rate(&self, symbol: &str) -> Result<FundingRate>;

    // 交易
    async fn place_order(&self, params: PlaceOrderParams) -> Result<Order>;
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<Order>;
    async fn cancel_all_orders(&self, symbol: &str) -> Result<Vec<Order>>;
    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>>;
    async fn get_order(&self, symbol: &str, order_id: &str) -> Result<Order>;

    // 永续合约特有
    async fn set_leverage(&self, symbol: &str, leverage: u32) -> Result<()>;

    // WebSocket 成交回报
    /// 订阅成交回报流，通过 channel 推送给引擎
    /// 返回一个 receiver，引擎在 WsFeed Loop 中消费
    async fn subscribe_order_updates(&self, symbols: &[&str])
        -> Result<mpsc::Receiver<WsFeedEvent>>;
}

/// 下单参数
pub struct PlaceOrderParams {
    pub symbol: String,
    pub side: Side,                        // Buy / Sell
    pub order_type: OrderType,
    pub amount: f64,
    pub price: Option<f64>,
    pub reduce_only: bool,
    pub position_side: Option<PositionSide>, // Long / Short / None(单向模式)
}
```

### 4.3 外部适配方式

主程序创建引擎时，将现有的 `CcxtAdapter`（或任何实现了类似接口的实例）适配为 Position Engine 的 Exchange trait：

```rust
// 主程序中的适配器（位于 src/position_engine/ 外部）
struct CcxtExchangeAdapter {
    inner: Box<dyn virs::exchange::Exchange>,
}

#[async_trait]
impl position_engine::Exchange for CcxtExchangeAdapter {
    async fn place_order(&self, params: PlaceOrderParams) -> Result<Order> {
        // 转换参数格式，调用 self.inner.place_order_with_options(...)
        // 转换返回值格式
    }
    // ... 其他方法类似
}

// 创建引擎
let engine = PositionEngine::new(
    config,
    Box::new(CcxtExchangeAdapter { inner: ccxt_exchange }),
    db_pool,
);
```

### 4.4 支持的交易所

| 交易所 | 市场类型 | 适配来源 |
|--------|----------|----------|
| Binance | Perpetual (USDT-M) | 已有 CcxtAdapter |
| Bybit | Perpetual (USDT) | 已有 CcxtAdapter |
| OKX | Perpetual (USDT) | 已有 CcxtAdapter |

后续新增交易所只需在主程序中新增适配器，Position Engine 无需修改。

---

## 5. 引擎核心 (engine.rs)

### 5.1 引擎结构

```rust
pub struct PositionEngine {
    config: EngineConfig,
    exchange: Box<dyn Exchange>,        // 注入的交易所实例
    db: PgPool,
    positions: DashMap<(String, String, PositionSide), Position>,  // (exchange, symbol, side) -> Position
    orders: DashMap<String, Order>,                // order_id -> Order
    cmd_rx: mpsc::Receiver<EngineCommand>,
    event_tx: broadcast::Sender<EngineEvent>,
    ws_feed_rx: mpsc::Receiver<WsFeedEvent>,       // WebSocket 成交回报
    risk_checker: RiskChecker,
    tracker: PnlTracker,
    state: EngineState,                             // 引擎运行状态
}
```

### 5.2 命令系统

外部通过 mpsc channel 发送命令，引擎异步处理并通过 broadcast channel 发出事件。

| 命令 | 参数 | 说明 |
|------|------|------|
| OpenPosition | symbol, side, amount, price?, leverage?, stop_loss?, take_profit? | 开仓 |
| ClosePosition | position_id, amount?, order_type? | 平仓（部分/全部） |
| ModifyPosition | position_id, stop_loss?, take_profit? | 修改止损止盈 |
| PlaceOrder | symbol, side, order_type, amount, price?, reduce_only? | 通用下单 |
| CancelOrder | symbol, order_id | 撤单 |
| CancelAllOrders | symbol | 批量撤单 |
| SyncPositions | - | 立即触发仓位同步 |
| Shutdown | - | 优雅关闭 |

### 5.3 事件系统

引擎通过 broadcast channel 发出事件，上层业务（网格机器人、全自动交易、WebSocket 推送等）订阅并做出响应。

| 事件 | 触发时机 | 说明 |
|------|----------|------|
| PositionOpened | 开仓成功 | 含仓位完整信息 |
| PositionClosed | 平仓完成 | 含实现盈亏 |
| PositionModified | 止损/止盈修改 | 含新参数 |
| OrderPlaced | 订单提交成功 | 含请求参数 |
| OrderFilled | 订单完全成交 | 含实际成交价、量、费用 |
| OrderPartiallyFilled | 订单部分成交 | 含已成交/未成交量、当前加权均价 |
| OrderCanceled | 订单撤销 | 含撤销原因 |
| OrderFailed | 订单失败 | 含错误信息 |
| RiskAlert | 触发风控规则 | 含风控类型和详情 |
| PositionSynced | 仓位同步完成 | 含差异信息 |
| LiquidationWarning | 接近强平价 | 含当前仓位和强平价 |

### 5.4 主循环

引擎启动后运行四个并行循环（详见第 13、14 章）：

**循环 1：命令分发 (Command Loop)**

1. 从 cmd_rx 接收命令
2. 风控检查（开仓/下单前检查风控规则）
3. 执行交易操作（调用 Exchange trait）
4. 更新本地状态（DashMap + 数据库）
5. 发送事件（broadcast channel）

**循环 2：仓位同步 (Sync Loop)**

1. 定时从交易所获取实际仓位和未完成订单
2. 与本地状态比对，检测差异（外部操作、强平等）
3. 同步本地状态并发出 PositionSynced 事件
4. 检查强平预警（距强平价 < 20% 时触发）

**循环 3：WebSocket 消费 (WsFeed Loop)**

1. 从 ws_feed_rx 接收交易所推送的成交回报
2. 根据 exchange_order_id 查找本地 Order
3. 更新 Order 状态，写入 Trade 记录
4. 更新 Position，发出 OrderFilled/OrderPartiallyFilled/PositionClosed 事件

**循环 4：轮询兜底 (Polling Loop)**

1. 定时查询所有活跃订单状态
2. 与本地状态比对，补齐 WebSocket 遗漏的成交记录

---

## 6. 成交价与滑点处理

### 6.1 问题分析

实际成交价与请求价格可能存在差异，这是仓位管理的核心问题之一：

| 场景 | 请求价 | 实际成交价 | 差异原因 | 影响 |
|------|--------|-----------|----------|------|
| Limit 单完全成交 | 100.00 | 100.00 | 无差异 | 无 |
| Limit 单部分成交 | 100.00 | 100.00 | 无差异，但量可能不够 | 需要处理剩余量 |
| Market 单 | null | 100.05 | 滑点 | 盈亏计算必须用实际成交价 |
| StopMarket 触发 | null | 99.95 | 滑点 + 穿透 | 止损可能偏离预期 |
| 部分成交后撤单 | 100.00 | 100.00 (50%) | 只成交了一半 | 仓位大小与预期不符 |
| 网格密集挂单 | 100.00 | 100.00 | 正常 | 多档同时成交时顺序不确定 |

### 6.2 处理策略

**仓位入场价计算**：

```
entry_price = Σ(fill_price_i × fill_amount_i) / Σ(fill_amount_i)
```

每次部分成交或完全成交时，重新计算加权平均入场价。**绝不使用请求价格作为入场价**。

**滑点记录**：

- Market 单/StopMarket 单下单前记录当前 ticker 价格作为基准价
- 成交后计算 `slippage = actual_fill_price - base_price`
- 记录到 Order.slippage 字段，用于后续滑点分析和策略优化

**部分成交处理**：

1. 每次收到 PartiallyFilled 事件，更新 Order.filled 和 Order.remaining
2. 按已成交量更新 Position.size 和 Position.entry_price
3. 如果 Position.size 变为 0（全部反向成交），触发 PositionClosed 事件
4. 剩余未成交部分继续等待，直到 Filled/Canceled/超时

**成交回调流程**：

```
Exchange 返回成交 → 更新 Order 状态 → 写入 Trade 记录
  → 重算 Position.entry_price（加权平均）
  → 更新 Position.size
  → 计算 unrealized_pnl
  → 发出 OrderFilled/OrderPartiallyFilled 事件
  → 如果仓位归零 → 发出 PositionClosed 事件（含 realized_pnl）
```

---

## 7. 手续费处理

### 7.1 手续费来源

手续费有两种获取方式：

| 方式 | 来源 | 用途 |
|------|------|------|
| **实际扣费（绝对值）** | 交易所成交回报（WsFeedEvent.commission） | 记录到 Order.fee / Trade.fee，用于精确盈亏计算 |
| **费率（百分比）** | 交易所 API 查询（启动时缓存） | 预估成本、判断挂单是否划算、回测模拟 |

### 7.2 费率查询

**Exchange trait 扩展**：

```rust
pub trait Exchange: Send + Sync {
    // ... 已有方法 ...

    /// 查询账户 Maker/Taker 费率
    async fn get_fee_rates(&self, symbol: &str) -> Result<FeeRates>;
}

pub struct FeeRates {
    pub symbol: String,
    pub maker_rate: f64,    // 如 0.0002 = 0.02%
    pub taker_rate: f64,    // 如 0.0005 = 0.05%
}
```

**各交易所 API**：

| 交易所 | API | 返回字段 |
|--------|-----|----------|
| Binance | `GET /fapi/v1/commissionRate` | makerCommission / takerCommission |
| Bybit | `GET /v5/account/fee-rate?category=linear&symbol=XXX` | makerFee / takerFee |
| OKX | `GET /api/v5/account/trade-fee?instId=XXX` | maker / taker |

引擎启动时查询一次并缓存，后续不再重复请求。

### 7.3 盈亏计算中的手续费

**开仓成本**：开仓手续费应计入成本，提高实际入场价：

```
effective_entry_price = entry_price × (1 + taker_rate)   // 吃单开仓
effective_entry_price = entry_price × (1 - maker_rate)   // 挂单开仓（费率从卖出中扣除）
```

**平仓盈亏**（修正第 8 章公式）：

```
单次平仓 pnl = (close_price - entry_price) × close_amount × direction - open_fee - close_fee
```

其中：
- `open_fee`：本次平仓对应的开仓部分手续费（按比例分摊）
- `close_fee`：本次平仓的手续费（WsFeedEvent.commission）

**简化处理**：由于实际手续费已在成交回报中返回，盈亏计算直接使用实际值：

```
单次平仓 pnl = close_amount × (close_price - entry_price) × direction - trade.fee
```

### 7.4 资金费率成本

持仓期间的资金费率支出/收入应累计到 Position.realized_pnl：

```
每次结算时：
  funding_cost = position.size × funding_rate × mark_price
  Position.realized_pnl -= funding_cost   // 正费率为支出，负费率为收入
```

资金费率在同步循环中检查（每 8h 结算一次），通过 `get_funding_rate()` 获取当前费率，与上次结算时的费率对比，如果发生变化则记录。

### 7.5 手续费币种

| 币种 | 影响 | 处理方式 |
|------|------|----------|
| USDT 本位 | 直接影响盈亏 | fee 直接作为成本扣除 |
| 币本位（如 BNB） | 不直接影响盈亏，但减少持仓量 | 记录 fee_currency，不参与盈亏计算，单独统计 |

Order 和 Trade 中新增 `fee_currency` 字段：

```rust
pub fee_currency: String,  // "USDT" / "BNB" / ...
```

---

## 8. 部分平仓处理

### 8.1 核心规则

| 规则 | 说明 |
|------|------|
| **入场价不变** | 部分平仓只减 size，entry_price 保持不变（成本基础不变） |
| **仓位状态不变** | 部分平仓后 size > 0 仍为 Open，仅 size = 0 时才转为 Closed |
| **realized_pnl 累计** | 每次部分平仓独立计算 pnl，累加到 Position.realized_pnl |
| **止损/止盈不联动** | 部分平仓后止损止盈价格不变，但风险敞口已减小 |

### 8.2 盈亏计算

```
单次部分平仓 pnl = (close_price - entry_price) × close_amount × direction
```

- Long 仓位：direction = +1
- Short 仓位：direction = -1
- entry_price 始终使用加权平均入场价（第 6 章已定义）

**示例**：

```
开仓：Long BTC, entry_price = 70000, size = 1.0
部分平仓 1：close_price = 72000, close_amount = 0.3
  → pnl = (72000 - 70000) × 0.3 × 1 = +600
  → Position: size = 0.7, entry_price = 70000 (不变), realized_pnl = 600

部分平仓 2：close_price = 71000, close_amount = 0.5
  → pnl = (71000 - 70000) × 0.5 × 1 = +500
  → Position: size = 0.2, entry_price = 70000 (不变), realized_pnl = 1100

全部平仓：close_price = 69000, close_amount = 0.2
  → pnl = (69000 - 70000) × 0.2 × 1 = -200
  → Position: size = 0, status = Closed, realized_pnl = 900
```

### 8.3 处理流程

```
收到 ClosePosition(position_id, amount=0.3)
  → 检查 amount <= position.size
  → 调用 exchange.place_order(reduce_only=true, amount=0.3)
  → 等待成交
  → 成交后：
      1. 写入 Trade 记录（trade_type = "partial_close"）
      2. 计算 pnl = (fill_price - entry_price) × fill_amount × direction
      3. Position.size -= fill_amount
      4. Position.realized_pnl += pnl
      5. Position.entry_price 不变
      6. 如果 Position.size == 0 → status = Closed，发出 PositionClosed 事件
      7. 如果 Position.size > 0 → status 不变，发出 OrderFilled 事件
```

### 8.4 多次部分平仓的一致性保证

| 场景 | 处理方式 |
|------|----------|
| 并发平仓请求 | 同一仓位串行处理（通过 DashMap 的 position_id 锁保证） |
| 部分平仓成交量不足 | 按实际成交量处理，剩余量继续等待 |
| 部分平仓期间仓位被外部同步覆盖 | 以交易所实际仓位为准，重新计算差异 |
| 部分平仓后立即再平仓 | 第二次请求检查剩余 size，不足则拒绝 |

---

## 9. 风控模块 (risk.rs)

### 9.1 风控规则

| 规则 | 参数 | 触发动作 | 默认值 |
|------|------|----------|--------|
| 单品种仓位上限 | max_position_per_symbol | 拒绝开仓 | 100% 总权益 |
| 总仓位上限 | max_total_position | 拒绝开仓 | 300% 总权益 |
| 单笔下单上限 | max_order_amount | 拒绝下单 | 30% 单品种仓位 |
| 回撤保护 | max_drawdown_pct | 暂停所有交易 + 平仓 | 15% |
| 杠杆上限 | max_leverage | 拒绝开仓/调杠杆 | 20x |
| 资金费率异常 | funding_rate_threshold | 发出警告/减仓 | ±0.1% |
| 强平预警 | liquidation_buffer_pct | 发出警告 + 可选自动减仓 | 20% |
| 连续亏损 | max_consecutive_losses | 减半仓位 | 5 次（按引擎维度） |

### 9.2 风控流程

每次开仓/下单前执行风控检查：

1. 检查仓位上限（单品种 + 总仓位）
2. 检查回撤是否超限
3. 检查杠杆是否超限
4. 检查可用保证金是否充足
5. 通过则执行，失败则返回 RiskAlert 事件

资金费率检查在**同步循环**中执行（每 10s），不阻塞下单流程：
- 费率超过阈值 → 发出 RiskAlert（severity: warning）
- 连续 3 次超阈值 → 建议上层减仓（RiskAlert + 建议动作）

---

## 10. 盈亏追踪 (tracker.rs)

### 10.1 追踪指标

| 指标 | 计算方式 | 更新频率 |
|------|----------|----------|
| 未实现盈亏 | (current_price - entry_price) × size × direction | 每次同步循环 |
| 已实现盈亏 | 累计每笔平仓成交的 PnL | 每笔成交时 |
| 总盈亏 | realized_pnl + unrealized_pnl | 每次同步循环 |
| 最大回撤 | peak_equity - current_equity | 每次同步循环 |
| 胜率 | profit_trades / total_trades | 每笔成交时 |
| 平均盈亏比 | avg(pnl / cost) | 每笔成交时 |
| 盈亏比 | total_pnl / total_margin | 每次同步循环 |

### 10.2 维度归因

盈亏数据支持按以下维度归因：

- 交易所 (exchange)
- 交易对 (symbol)
- 策略 ID (strategy_id)
- 时间范围（日/周/月）

---

## 11. 持久化 (persistence.rs)

### 11.1 数据库表

| 表名 | 用途 | 核心字段 |
|------|------|----------|
| pe_positions | 仓位记录 | id, engine_id, strategy_id, exchange, symbol, side, status, size, entry_price, leverage, margin, unrealized_pnl, realized_pnl, stop_loss, take_profit, liquidation_price, opened_at, updated_at, closed_at, metadata |
| pe_orders | 订单记录 | id, position_id, exchange_order_id, exchange, symbol, side, order_type, request_price, fill_price, amount, filled, remaining, status, reduce_only, fee, slippage, created_at, updated_at |
| pe_trades | 成交记录 | id, position_id, order_id, exchange, symbol, side, price, amount, fee, pnl, trade_type, created_at |
| pe_pnl_snapshots | 盈亏快照 | id, engine_id, timestamp, equity, unrealized_pnl, realized_pnl, total_pnl, max_drawdown, open_positions_count |
| pe_events | 风控事件 | id, engine_id, event_type, symbol, message, severity, created_at |

### 11.2 写入策略

- **仓位变更**：每次状态变化时写入（开仓/平仓/修改/同步）
- **订单变更**：创建/成交/撤销时写入
- **成交记录**：每笔成交时写入（不可变）
- **盈亏快照**：每次同步循环写入（周期性快照）
- **风控事件**：每次触发风控时写入

---

## 12. 网格交易集成方案

### 12.1 网格调整时的仓位处理

网格调整是仓位管理引擎的典型场景，处理流程：

1. **暂停网格**（停止新下单）
2. **取消所有未成交挂单** (CancelAllOrders)
3. **同步仓位**（确认实际持仓状态）
4. **处理现有仓位**（保留/市价平仓/迁移挂单，由上层策略决定）
5. **按新参数重新挂单**
6. **恢复网格运行**

### 12.2 上层集成方式

网格机器人通过订阅 PositionEngine 的事件来协作：

| 事件 | 网格机器人响应 |
|------|----------------|
| OrderFilled | 判断是否触发对应网格层的反向挂单 |
| OrderPartiallyFilled | 更新网格层状态，等待完全成交 |
| PositionSynced | 检测外部操作导致的仓位变化，重新计算网格 |
| RiskAlert | 根据风控类型执行相应操作（减仓/暂停/平仓） |
| LiquidationWarning | 紧急减仓或全部平仓 |

---

## 13. 成交回报机制

### 13.1 方案选择：WebSocket 优先 + 轮询兜底

| 方式 | 延迟 | 可靠性 | 用途 |
|------|------|--------|------|
| **WebSocket 推送** | 毫秒级 | 高（需处理断连） | 主通道，实时接收成交回报 |
| **定时轮询** | 秒级 | 最高 | 兜底通道，补偿 WebSocket 消息丢失 |

### 13.2 处理流程

```
交易所 WebSocket 推送 OrderUpdate
  → ws_feed_rx 接收
  → 根据 exchange_order_id 查找本地 Order
  → 更新 Order 状态（filled / remaining / fill_price / fee）
  → 写入 Trade 记录
  → 更新 Position（size / entry_price / unrealized_pnl / realized_pnl）
  → 发出 OrderFilled / OrderPartiallyFilled 事件
  → 如果仓位归零 → 发出 PositionClosed 事件
```

### 13.3 WebSocket 断连与重连

| 场景 | 处理方式 |
|------|----------|
| 正常断连 | 自动重连，重连期间切换到轮询模式 |
| 重连成功 | 重新订阅，补查断连期间的订单状态 |
| 重连失败（超 30s） | 发出 RiskAlert 事件，建议上层暂停交易 |
| 消息乱序 | 按交易所返回的 timestamp 排序处理 |

**断连期间的补偿机制**：

```
WebSocket 断连检测
  → 立即对活跃订单执行一次 get_order 查询
  → 切换到轮询模式（每 2s 查询一次活跃订单）
  → WebSocket 重连成功后
      → 重新订阅
      → 对所有 Open/PartiallyFilled 状态的订单做一次全量查询
      → 对比本地状态，补齐缺失的成交记录
      → 切回 WebSocket 模式
```

### 13.4 轮询兜底

即使 WebSocket 正常，也运行一个低频轮询作为兜底：

```
轮询循环（每 10s）：
  → 获取所有 Open/PartiallyFilled 状态的订单
  → 对比本地状态
  → 如果发现差异（本地落后于交易所）
      → 以交易所状态为准，补齐成交记录
      → 发出相应事件
```

### 13.5 各交易所 WebSocket 支持

| 交易所 | WebSocket 地址 | 订阅方式 | 数据流 |
|--------|---------------|----------|--------|
| Binance | wss://fstream.binance.com/ws | 订阅 `orderId` 的 USER_DATA stream | ORDER_UPDATE 事件 |
| Bybit | wss://stream.bybit.com/v5/private | 订阅 `order` channel | `executionReport` 事件 |
| OKX | wss://ws.okx.com:8443/ws/v5/private | 订阅 `orders` channel | `orders` channel 推送 |

WebSocket 连接和消息解析由外部适配器（CcxtExchangeAdapter）实现，Position Engine 只消费 WsFeedEvent。

---

## 14. 引擎生命周期管理

### 14.1 状态定义

```rust
pub enum EngineState {
    /// 已创建，未启动
    Created,
    /// 正常运行中
    Running,
    /// 已暂停（不处理新命令，同步循环停止，但 WebSocket 保持连接）
    Paused,
    /// 正在关闭（等待进行中的操作完成）
    ShuttingDown,
    /// 已关闭
    Stopped,
}
```

### 14.2 生命周期

```
new(config, exchange, db)     → State::Created
  │
  ├─ run()                   → State::Running
  │    ├─ 启动命令循环 (Command Loop)
  │    ├─ 启动同步循环 (Sync Loop)
  │    ├─ 启动 WebSocket 消费循环 (WsFeed Loop)
  │    └─ 启动轮询兜底循环 (Polling Loop)
  │
  ├─ pause()                 → State::Paused
  │    ├─ 停止命令循环（排空当前命令后停止）
  │    ├─ 停止同步循环
  │    └─ 保持 WebSocket 连接（继续接收成交回报）
  │
  ├─ resume()                → State::Running
  │    ├─ 从数据库恢复状态到 DashMap
  │    ├─ 执行一次全量仓位同步
  │    └─ 重启所有循环
  │
  └─ shutdown()              → State::ShuttingDown → State::Stopped
       ├─ 停止接收新命令
       ├─ 等待进行中的操作完成（超时 30s）
       ├─ 持久化所有内存状态到数据库
       ├─ 关闭 WebSocket 连接
       └─ 释放资源
```

### 14.3 公开 API

```rust
impl PositionEngine {
    /// 创建引擎实例（不启动循环）
    pub fn new(config, exchange, db) -> Self;

    /// 启动引擎（阻塞，通常在 tokio::spawn 中运行）
    pub async fn run(&mut self) -> Result<()>;

    /// 暂停引擎
    pub async fn pause(&mut self) -> Result<()>;

    /// 恢复引擎
    pub async fn resume(&mut self) -> Result<()>;

    /// 优雅关闭
    pub async fn shutdown(&mut self) -> Result<()>;

    /// 获取命令发送端（供外部调用）
    pub fn command_sender(&self) -> mpsc::Sender<EngineCommand>;

    /// 订阅事件（供上层业务监听）
    pub fn subscribe_events(&self) -> broadcast::Receiver<EngineEvent>;

    /// 查询当前仓位（只读）
    pub fn get_position(&self, exchange: &str, symbol: &str, side: PositionSide) -> Option<Position>;

    /// 查询所有仓位（只读）
    pub fn get_all_positions(&self) -> Vec<Position>;
}
```

---

## 15. 错误恢复与重启

### 15.1 崩溃恢复流程

```
引擎启动 (run)
  → 从数据库加载所有 status = Open 的 Position
  → 从数据库加载所有 status = Open/PartiallyFilled 的 Order
  → 恢复到 DashMap
  → 执行一次全量同步（与交易所实际状态比对）
    → 如果本地有但交易所没有（外部平仓/强平）
        → 标记 Position 为 Closed，发出 PositionSynced 事件
    → 如果交易所有但本地没有（外部开仓）
        → 创建新 Position，发出 PositionSynced 事件
    → 如果订单状态不一致
        → 以交易所为准，补齐成交记录
  → 恢复 PnlTracker 状态（从 pe_pnl_snapshots 最新记录恢复 peak_equity）
  → 启动所有循环
```

### 15.2 数据一致性保证

| 场景 | 保证机制 |
|------|----------|
| 下单成功但写入 DB 失败 | 命令循环重试（最多 3 次），仍失败则发出 OrderFailed 事件 |
| 成交回报处理中途崩溃 | 下次启动时全量同步会检测到差异并修复 |
| 并发写入冲突 | 同一 position_id 串行处理（DashMap 的 key 级别原子操作） |
| 数据库连接断开 | 写入失败时缓存到内存队列，连接恢复后批量写入 |

### 15.3 审计日志

所有状态变更都写入 `pe_events` 表（风控事件 + 审计日志共用，通过 event_type 区分）：

| event_type | 触发时机 |
|------------|----------|
| engine_started | 引擎启动 |
| engine_paused | 引擎暂停 |
| engine_resumed | 引擎恢复 |
| engine_stopped | 引擎关闭 |
| state_recovered | 崩溃恢复完成 |
| sync_mismatch | 同步发现差异 |
| order_retry_failed | 订单重试失败 |

---

## 16. 双向持仓支持

### 16.1 模式说明

Binance 支持 Hedge Mode（双向持仓），允许同一交易对同时持有多头和空头仓位。

### 16.2 Key 设计

```
positions: DashMap<(String, String, PositionSide), Position>
//             exchange   symbol     side
```

- **单向持仓模式**（One-way）：只使用 `PositionSide::Long`，`PositionSide::Both` 不作为 key
- **双向持仓模式**（Hedge）：同时存在 `(exchange, symbol, Long)` 和 `(exchange, symbol, Short)` 两个条目

### 16.3 下单时的 position_side 传递

下单参数 `PlaceOrderParams` 已在第 4.2 节定义，其中 `position_side` 字段控制双向持仓行为：

---

## 17. 配置管理

### 17.1 EngineConfig

```rust
pub struct EngineConfig {
    pub engine_id: String,

    // 同步配置
    pub sync_interval_secs: u64,          // 仓位同步间隔，默认 10s
    pub poll_interval_secs: u64,          // 轮询兜底间隔，默认 10s
    pub ws_reconnect_timeout_secs: u64,   // WebSocket 重连超时，默认 30s

    // 风控配置（见第 9 章）
    pub risk: RiskConfig,

    // 盈亏快照
    pub pnl_snapshot_interval_secs: u64,  // 盈亏快照间隔，默认 60s
}

pub struct RiskConfig {
    pub max_position_per_symbol_pct: f64, // 默认 1.0 (100%)
    pub max_total_position_pct: f64,      // 默认 3.0 (300%)
    pub max_order_amount_pct: f64,        // 默认 0.3 (30%)
    pub max_drawdown_pct: f64,            // 默认 0.15 (15%)
    pub max_leverage: u32,                // 默认 20
    pub funding_rate_threshold: f64,      // 默认 0.001 (0.1%)
    pub liquidation_buffer_pct: f64,      // 默认 0.2 (20%)
    pub max_consecutive_losses: u32,      // 默认 5
}
```

### 17.2 配置来源

配置通过 `EngineConfig` 结构体在创建引擎时传入，由主程序从以下来源加载：

| 来源 | 用途 | 优先级 |
|------|------|--------|
| 数据库 | 持久化的引擎配置 | 最高 |
| 环境变量 | 全局默认值覆盖 | 中 |
| 代码默认值 | 兜底 | 最低 |

主程序在创建引擎前从数据库读取配置，未配置的字段使用环境变量或默认值。

---

## 18. 数据库建表 SQL

```sql
-- 仓位记录
CREATE TABLE IF NOT EXISTS pe_positions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    engine_id       VARCHAR(64) NOT NULL,
    strategy_id     UUID,
    exchange        VARCHAR(32) NOT NULL,
    symbol          VARCHAR(32) NOT NULL,
    side            VARCHAR(8) NOT NULL,  -- 'long' / 'short'
    status          VARCHAR(16) NOT NULL, -- 'empty'/'opening'/'open'/'closing'/'closed'
    size            DOUBLE PRECISION NOT NULL DEFAULT 0,
    entry_price     DOUBLE PRECISION NOT NULL DEFAULT 0,
    current_price   DOUBLE PRECISION NOT NULL DEFAULT 0,
    leverage        INTEGER NOT NULL DEFAULT 1,
    margin          DOUBLE PRECISION NOT NULL DEFAULT 0,
    unrealized_pnl  DOUBLE PRECISION NOT NULL DEFAULT 0,
    realized_pnl    DOUBLE PRECISION NOT NULL DEFAULT 0,
    stop_loss       DOUBLE PRECISION,
    take_profit     DOUBLE PRECISION,
    liquidation_price DOUBLE PRECISION,
    opened_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at       TIMESTAMPTZ,
    metadata        JSONB DEFAULT '{}',
    UNIQUE(engine_id, exchange, symbol, side)
);

-- 订单记录
CREATE TABLE IF NOT EXISTS pe_orders (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    position_id         UUID REFERENCES pe_positions(id),
    exchange_order_id   VARCHAR(64),
    client_order_id     VARCHAR(64),
    exchange            VARCHAR(32) NOT NULL,
    symbol              VARCHAR(32) NOT NULL,
    side                VARCHAR(8) NOT NULL,
    order_type          VARCHAR(16) NOT NULL,
    request_price       DOUBLE PRECISION,
    fill_price          DOUBLE PRECISION,
    amount              DOUBLE PRECISION NOT NULL,
    filled              DOUBLE PRECISION NOT NULL DEFAULT 0,
    remaining           DOUBLE PRECISION NOT NULL DEFAULT 0,
    status              VARCHAR(16) NOT NULL,
    reduce_only         BOOLEAN NOT NULL DEFAULT FALSE,
    fee                 DOUBLE PRECISION NOT NULL DEFAULT 0,
    fee_currency        VARCHAR(16) DEFAULT 'USDT',
    slippage            DOUBLE PRECISION,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 成交记录（不可变）
CREATE TABLE IF NOT EXISTS pe_trades (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    position_id     UUID REFERENCES pe_positions(id),
    order_id        UUID REFERENCES pe_orders(id),
    exchange        VARCHAR(32) NOT NULL,
    symbol          VARCHAR(32) NOT NULL,
    side            VARCHAR(8) NOT NULL,
    price           DOUBLE PRECISION NOT NULL,
    amount          DOUBLE PRECISION NOT NULL,
    fee             DOUBLE PRECISION NOT NULL DEFAULT 0,
    fee_currency    VARCHAR(16) DEFAULT 'USDT',
    pnl             DOUBLE PRECISION NOT NULL DEFAULT 0,
    trade_type      VARCHAR(16) NOT NULL, -- 'open'/'close'/'partial_close'
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 盈亏快照
CREATE TABLE IF NOT EXISTS pe_pnl_snapshots (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    engine_id           VARCHAR(64) NOT NULL,
    timestamp           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    equity              DOUBLE PRECISION NOT NULL,
    unrealized_pnl      DOUBLE PRECISION NOT NULL DEFAULT 0,
    realized_pnl        DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_pnl           DOUBLE PRECISION NOT NULL DEFAULT 0,
    max_drawdown        DOUBLE PRECISION NOT NULL DEFAULT 0,
    open_positions_count INTEGER NOT NULL DEFAULT 0
);

-- 风控与审计事件
CREATE TABLE IF NOT EXISTS pe_events (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    engine_id       VARCHAR(64) NOT NULL,
    event_type      VARCHAR(32) NOT NULL,
    symbol          VARCHAR(32),
    message         TEXT NOT NULL,
    severity        VARCHAR(8) NOT NULL DEFAULT 'info', -- 'info'/'warning'/'critical'
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 索引
CREATE INDEX idx_pe_positions_engine ON pe_positions(engine_id, status);
CREATE INDEX idx_pe_positions_lookup ON pe_positions(exchange, symbol, side);
CREATE INDEX idx_pe_orders_position ON pe_orders(position_id);
CREATE INDEX idx_pe_orders_exchange ON pe_orders(exchange_order_id) WHERE exchange_order_id IS NOT NULL;
CREATE INDEX idx_pe_orders_status ON pe_orders(status) WHERE status IN ('open', 'partially_filled');
CREATE INDEX idx_pe_trades_position ON pe_trades(position_id);
CREATE INDEX idx_pnl_snapshots_engine ON pe_pnl_snapshots(engine_id, timestamp DESC);
CREATE INDEX idx_events_engine ON pe_events(engine_id, created_at DESC);
```

---

## 19. 回撤保护策略细化

### 19.1 回撤计算

```
drawdown_pct = (peak_equity - current_equity) / peak_equity
```

- `peak_equity`：历史最高权益（初始资金 + 已实现盈亏 + 未实现盈亏）
- `current_equity`：当前权益
- 每次同步循环更新

### 19.2 触发后的执行策略

回撤保护触发后不是盲目全部平仓，而是分级响应：

| 回撤程度 | 级别 | 动作 |
|----------|------|------|
| < 10% | 正常 | 无动作 |
| 10% ~ 15% | 警告 | 发出 RiskAlert，记录日志 |
| 15% ~ 20% | 危险 | 发出 RiskAlert，暂停新开仓，允许现有仓位自然平仓 |
| > 20% | 严重 | 发出 RiskAlert，暂停所有交易，按优先级平仓（先平亏损最大的仓位） |

### 19.3 平仓优先级

```
平仓顺序 = 按未实现盈亏从低到高排序（亏损最大的先平）
  → 如果有多个同交易对的多空仓位，先平亏损方向
  → 使用 Market 单平仓，确保快速执行
```

---

## 20. 上层调用方式

### 20.1 进程内嵌入（推荐）

Position Engine 设计为库（crate），由主进程直接嵌入使用：

```rust
// 主程序中创建并启动引擎
let engine = PositionEngine::new(config, exchange_adapter, db_pool);
let cmd_tx = engine.command_sender();
let mut event_rx = engine.subscribe_events();

// 在 tokio::spawn 中运行引擎
tokio::spawn(async move {
    engine.run().await.unwrap();
});

// 上层业务通过 channel 发送命令
cmd_tx.send(EngineCommand::OpenPosition { ... }).await.unwrap();

// 上层业务通过 channel 接收事件
tokio::spawn(async move {
    while let Ok(event) = event_rx.recv().await {
        match event {
            EngineEvent::OrderFilled { .. } => { /* 网格机器人处理 */ }
            EngineEvent::RiskAlert { .. } => { /* 风控处理 */ }
            _ => {}
        }
    }
});
```

**优点**：零网络开销、类型安全、无需序列化。

### 20.2 HTTP API（可选）

如果需要从外部进程或前端直接调用，可以在主程序中暴露 HTTP API 层（位于 `src/position_engine/` 外部）：

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/position-engine/{engine_id}/positions` | 查询所有仓位 |
| GET | `/api/position-engine/{engine_id}/orders` | 查询活跃订单 |
| GET | `/api/position-engine/{engine_id}/trades` | 查询成交记录 |
| GET | `/api/position-engine/{engine_id}/pnl` | 查询盈亏统计 |
| POST | `/api/position-engine/{engine_id}/open` | 开仓 |
| POST | `/api/position-engine/{engine_id}/close` | 平仓 |
| POST | `/api/position-engine/{engine_id}/cancel` | 撤单 |
| POST | `/api/position-engine/{engine_id}/pause` | 暂停引擎 |
| POST | `/api/position-engine/{engine_id}/resume` | 恢复引擎 |

HTTP API 层不在 `src/position_engine/` 内，由主程序实现，调用引擎的公开方法。

---

## 21. 多实例部署

### 21.1 engine_id 唯一性

每个引擎实例通过 `engine_id` 标识，格式建议：`{user_id}:{exchange}:{market_type}`

```
示例：
"user_001:binance:perpetual"
"user_001:bybit:perpetual"
"user_002:binance:perpetual"
```

- 同一用户可以运行多个引擎实例（不同交易所）
- 同一交易所同一用户只允许一个引擎实例（通过数据库唯一约束保证）
- `engine_id` 作为所有数据库表的前缀，实现数据隔离

### 21.2 部署模式

| 模式 | 说明 | 适用场景 |
|------|------|----------|
| 单进程多引擎 | 一个 VIRS 进程内运行多个 PositionEngine 实例 | 单用户、多交易所 |
| 多进程 | 每个用户/交易所一个进程 | 多用户隔离 |
| 分布式 | 多台机器，共享 PostgreSQL | 高可用、负载均衡 |

所有模式共享同一个 PostgreSQL 数据库，通过 `engine_id` 隔离数据。

---

## 22. 实现计划

### 22.1 阶段规划

| 阶段 | 内容 | 依赖 |
|------|------|------|
| P1: 基础框架 | types.rs + error.rs + config.rs + exchange.rs (trait 定义) | 无 |
| P2: 引擎核心 | engine.rs（命令循环 + 下单/撤单 + 事件发送） | P1 |
| P3: 持久化 | persistence.rs（表创建 + CRUD） | P1 |
| P4: 仓位同步 | engine.rs（同步循环 + 差异检测） | P2 + P3 |
| P5: 风控 | risk.rs（规则检查 + 事件触发） | P2 |
| P6: 盈亏追踪 | tracker.rs（盈亏计算 + 快照） | P3 |
| P7: 主程序适配 | CcxtExchangeAdapter + 引擎注册 | P4 + P5 + P6 |
| P8: 网格集成 | 网格机器人接入 PositionEngine | P7 |

### 22.2 技术选型

| 技术 | 用途 | 理由 |
|------|------|------|
| sqlx + PostgreSQL | 持久化 | 与现有项目一致 |
| tokio | 异步运行时 | 已有依赖 |
| dashmap | 并发状态管理 | 已有依赖 |
| serde / serde_json | 序列化 | 已有依赖 |
| chrono / uuid | 时间和 ID | 已有依赖 |
| tracing | 日志 | 已有依赖 |
