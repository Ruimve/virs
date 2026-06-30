# virs-models 测试用例文档

> 生成日期: 2026-06-30
> 最后更新: 2026-06-30
> Crate: `crates/libs/virs-models`
> 目标: 抽取数据模型中的幂等计算逻辑为方法，进行完整单元测试和集成测试覆盖。
> 状态: **全部 37 个测试通过** (31 单元 + 6 集成)；孤儿代码问题已解决。

---

## 1. Crate 结构概览

```
virs-models/
├── src/
│   ├── lib.rs        # 模块入口，re-export virs-types 的枚举和 market 类型
│   ├── trading.rs    # Order 结构体 (API 层订单表示，纯数据)
│   ├── user.rs       # User, UserResponse, LoginRequest, LoginResponse, CreateUserRequest
│   ├── grid.rs       # GridBot (含方法), GridTrade (纯数据)
│   └── auto.rs       # AutoBot (含方法), AutoTrade (纯数据)
├── tests/
│   ├── test_plan.md  # 本文档
│   └── integration_tests.rs
```

### 设计原则

`virs-models` 只保留**被业务代码实际调用**的派生方法。对于状态判断类方法（如 `Order::is_filled`、`GridTrade::is_open`），若业务代码已直接通过 `status` 字段或 `virs-types::PositionOrder` 处理，则不在此 crate 重复封装，避免孤儿代码。

---

## 2. 保留的幂等方法

### 2.1 grid.rs — GridBot 方法

| # | 方法 | 签名 | 说明 | 业务调用位置 |
|---|------|------|------|-------------|
| G1 | `grid_spacing` | `(&self) -> f64` | 网格间距 = (upper - lower) / grid_count | `virs-api/src/handlers/grid.rs` `get_bot` |
| G3 | `is_running` | `(&self) -> bool` | 是否运行中 | `virs-api/src/handlers/grid.rs` `get_bot`；`virs-bot/src/grid/worker.rs` |
| G4 | `is_stopped` | `(&self) -> bool` | 是否已停止 | `virs-api/src/handlers/grid.rs` `get_bot`；`virs-bot/src/grid/worker.rs` |
| G5 | `total_return_pct` | `(&self) -> f64` | 总收益率 = total_pnl / initial_capital * 100 | `virs-api/src/handlers/grid.rs` `get_bot` |

### 2.2 auto.rs — AutoBot 方法

| # | 方法 | 签名 | 说明 | 业务调用位置 |
|---|------|------|------|-------------|
| A1 | `win_rate` | `(&self) -> f64` | 胜率 = win_trades / total_trades * 100 | `virs-api/src/handlers/auto_trade.rs` `get_stats` |
| A2 | `loss_rate` | `(&self) -> f64` | 亏损率 = loss_trades / total_trades * 100 | `virs-api/src/handlers/auto_trade.rs` `get_stats` |
| A3 | `total_return_pct` | `(&self) -> f64` | 总收益率 = total_pnl / initial_capital * 100 | `virs-api/src/handlers/auto_trade.rs` `get_bot` |
| A4 | `is_running` | `(&self) -> bool` | 是否运行中 (status == "running") | `virs-api/src/handlers/auto_trade.rs` `get_bot` |
| A5 | `is_stopped` | `(&self) -> bool` | 是否已停止 (status == "stopped") | `virs-api/src/handlers/auto_trade.rs` `get_bot` |

### 2.3 已删除的孤儿方法

以下方法此前仅被测试引用、未被业务代码调用，已删除以保持 crate 精简：

| 原方法 | 原因 |
|-------|------|
| `Order::is_filled` / `is_open` / `is_canceled` / `fill_rate` / `is_buy` / `is_sell` | `Order` 为 API 层瞬时类型；状态判断由 `virs-types::PositionOrder` 在引擎层处理 |
| `User::to_response` | `virs-api` 的 auth handler 直接内联构造响应，无需封装 |
| `GridTrade::is_open` / `is_closed` | 业务代码直接通过 `status` 字段判断 |
| `AutoTrade::is_open` / `is_closed` | 业务代码直接通过 `status` 字段判断 |
| `GridBot::is_valid_config` | 业务代码（`virs-bot/src/grid/utils/mod.rs`）已直接判断 `width <= 0.0 \|\| grid_count <= 0`，无需替换 |

---

## 3. 单元测试用例

### 3.1 grid_tests.rs — GridBot 方法

#### TC-G1: grid_spacing
- G1.1 upper=50000, lower=40000, grid_count=10 → 1000.0
- G1.2 upper=50000, lower=50000, grid_count=10 → 0.0
- G1.3 grid_count=0 → 0.0 (除零保护)

#### TC-G3: is_running
- G3.1 status=Running → true
- G3.2 status=Stopped → false

#### TC-G4: is_stopped
- G4.1 status=Stopped → true
- G4.2 status=Running → false

#### TC-G5: total_return_pct
- G5.1 total_pnl=500, initial_capital=10000 → 5.0
- G5.2 total_pnl=0, initial_capital=10000 → 0.0
- G5.3 total_pnl=500, initial_capital=0 → 0.0 (除零保护)

### 3.2 auto_tests.rs — AutoBot 方法

#### TC-A1: win_rate
- A1.1 win=7, total=10 → 70.0
- A1.2 win=0, total=10 → 0.0
- A1.3 win=10, total=0 → 0.0 (除零保护)

#### TC-A2: loss_rate
- A2.1 loss=3, total=10 → 30.0
- A2.2 loss=0, total=0 → 0.0 (除零保护)

#### TC-A3: total_return_pct
- A3.1 total_pnl=1000, initial_capital=10000 → 10.0
- A3.2 total_pnl=-500, initial_capital=10000 → -5.0
- A3.3 initial_capital=0 → 0.0 (除零保护)

#### TC-A4: is_running
- A4.1 status="running" → true
- A4.2 status="stopped" → false

#### TC-A5: is_stopped
- A5.1 status="stopped" → true
- A5.2 status="running" → false

### 3.3 serde_tests.rs — 序列化/反序列化

#### TC-S1: Order serde round-trip
- S1.1 完整 Order → JSON → Order → 字段一致

#### TC-S2: User/UserResponse serde
- S2.1 User round-trip
- S2.2 UserResponse round-trip
- S2.3 LoginRequest 反序列化
- S2.4 CreateUserRequest 反序列化 (role=None)

#### TC-S3: GridBot/GridTrade serde
- S3.1 GridBot round-trip
- S3.2 GridTrade round-trip

#### TC-S4: AutoBot/AutoTrade serde
- S4.1 AutoBot round-trip
- S4.2 AutoTrade round-trip

---

## 4. 集成测试用例

### TC-INT-1: User 结构体
- 已移除 `to_response()` 方法的集成测试。User 结构体由 `virs-api` 的 auth handler 通过 `sqlx::FromRow` 直接使用，无需转换方法。

### TC-INT-2: GridBot 计算链路
- INT-2.1 构造 GridBot → grid_spacing() / is_running() / is_stopped() / total_return_pct() 一致
- INT-2.2 无效配置 (upper<lower) → grid_spacing() 为负，is_stopped() == true

### TC-INT-3: AutoBot 统计链路
- INT-3.1 win_rate + loss_rate = 100 (完整覆盖)
- INT-3.2 total_return_pct 正负值，is_stopped()/is_running() 状态判断

### TC-INT-4: Order 结构体
- 已移除状态判断方法的集成测试。Order 为 API 层瞬时类型，状态判断由 `virs-types::PositionOrder` 在引擎层处理。

### TC-INT-5: serde 完整往返
- INT-5.1 GridBot → JSON → GridBot → grid_spacing() 一致
- INT-5.2 AutoBot → JSON → AutoBot → win_rate() 一致

---

## 5. 测试文件与模块映射

| 测试文件 | 被测模块 | 测试数 | 状态 |
|----------|----------|-------|------|
| `src/grid_tests.rs` | grid.rs (GridBot) | 10 | ✅ 通过 |
| `src/auto_tests.rs` | auto.rs (AutoBot) | 12 | ✅ 通过 |
| `src/serde_tests.rs` | 全部 serde | 9 | ✅ 通过 |
| `tests/integration_tests.rs` | 跨模块 | 6 | ✅ 通过 |
| **合计** | | **37** | ✅ 全部通过 |

> 已删除的测试文件：`src/trading_tests.rs`、`src/user_tests.rs`（对应方法已删除）。

---

## 6. 业务代码重构记录

### 6.1 `virs-api/src/handlers/auto_trade.rs`

**`get_bot` 函数**：原本使用两次独立 SQL 查询 + 内联计算，重构为单次查询获取完整 `AutoBot`，并使用模型方法：

```rust
// 重构前
let win_rate = if total_trades > 0 {
    (win_trades as f64 / total_trades as f64) * 100.0
} else { 0.0 };

// 重构后
let bot = sqlx::query_as::<_, virs_models::AutoBot>("SELECT * FROM qd_auto_bots WHERE id = $1 AND user_id = $2")
    .bind(id).bind(user_id).fetch_optional(&state.db_pool).await?;
"total_return_pct": bot.total_return_pct(),
"is_running": bot.is_running(),
"is_stopped": bot.is_stopped(),
```

**`get_stats` 函数**：使用 `AutoBot::win_rate()` 和 `loss_rate()` 替换内联计算，并在响应 JSON 中新增 `loss_rate` 字段。

### 6.2 `virs-api/src/handlers/grid.rs`

**`get_bot` 函数**：原本使用两次独立 SQL 查询 + 内联计算，重构为单次查询获取完整 `GridBot`，并使用模型方法：

```rust
// 重构后
"grid_spacing": bot.grid_spacing(),
"total_return_pct": bot.total_return_pct(),
"is_running": bot.is_running(),
"is_stopped": bot.is_stopped(),
```

### 6.3 `virs-bot/src/grid/worker.rs`

状态判断逻辑已使用 `GridBot::is_running` / `is_stopped`（在前次提交中完成）。

---

## 7. 验证结果

### 7.1 测试通过

```
$ cargo test -p virs-models
running 31 tests ... test result: ok. 31 passed; 0 failed
running 6 tests  ... test result: ok. 6 passed; 0 failed
```

### 7.2 编译验证

```
$ cargo check -p virs-api -p virs-bot
Finished `dev` profile in 6.74s
```

### 7.3 Clippy 检查

```
$ cargo clippy -p virs-models -p virs-api -p virs-bot --all-targets
Finished `dev` profile in 7.66s
```

`virs-models` 零警告。`virs-api` / `virs-bot` 中仅有与本次重构无关的预存警告（`ai.rs`、`state.rs`、`virs-market`）。

### 7.4 孤儿代码检查结论

**已解决**。所有保留的 `pub fn` 方法均被业务代码调用：

| 方法 | 业务调用位置 |
|------|-------------|
| `GridBot::grid_spacing` | `virs-api/src/handlers/grid.rs` |
| `GridBot::is_running` / `is_stopped` | `virs-api/src/handlers/grid.rs`；`virs-bot/src/grid/worker.rs` |
| `GridBot::total_return_pct` | `virs-api/src/handlers/grid.rs` |
| `AutoBot::win_rate` / `loss_rate` | `virs-api/src/handlers/auto_trade.rs` |
| `AutoBot::total_return_pct` | `virs-api/src/handlers/auto_trade.rs` |
| `AutoBot::is_running` / `is_stopped` | `virs-api/src/handlers/auto_trade.rs` |

所有未被业务代码调用的孤儿方法及其测试已删除。
