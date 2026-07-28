# virs-exchange 测试用例文档

> 生成日期: 2026-07-28（Exchange trait 合并后更新）
> Crate: `crates/libs/virs-exchange`
> 状态: **13 个测试全部通过** (4 单元 + 9 集成)

---

## 背景

三个并行 Exchange trait（`virs_ccxt::Exchange`、`virs_exchange::Exchange`、`virs_types::ExchangePe`）已合并为统一的 `ExchangePe`。原 `pe_adapter.rs`（268 行恒等转换中间层）和 `pe_adapter_tests.rs`（32 个测试）已删除。原 `adapter_tests.rs` 中 7 个引用已删除转换函数的测试已删除，保留 4 个 Kline 转换测试。

---

## 单元测试用例

### adapter_tests.rs — Kline 转换函数 (4)

| ID | 测试函数 | 描述 |
|----|---------|------|
| A4.1 | `a4_1_kline_normal_conversion` | 正常 kline 转换 → 字段一致 |
| A4.2 | `a4_2_kline_exchange_field` | exchange 字段 + None → 默认值 |
| A4.3 | `a4_3_kline_close_time_binance_format` | close_time 使用币安格式 |
| A4.4 | `a4_4_kline_close_time_from_exchange` | close_time 从 exchange 传入 |

---

## 集成测试用例

### integration_tests.rs (9)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-1.1 | `int_1_1_side_roundtrip` | Side 类型一致性验证 |
| INT-1.2 | `int_1_2_order_type_roundtrip` | OrderType 类型一致性验证 |
| INT-1.3 | `int_1_3_position_side_consistency` | PositionSide 类型一致性验证 |
| INT-1.4 | `int_1_4_market_type_consistency` | MarketType 类型一致性验证 |
| INT-2.1 | `int_2_1_paper_exchange_creation_and_balance` | Paper 创建 + 初始余额验证 |
| INT-2.2 | `int_2_2_paper_market_order_updates_balance` | Paper 市价单 → 余额减少 + 持仓创建 |
| INT-3.1 | `int_3_1_registry_register_and_get` | 注册表注册并获取 |
| INT-3.2 | `int_3_2_registry_get_nonexistent` | 不存在的 key → None |
| INT-3.3 | `int_3_3_registry_list_names` | 注册表列出名称 |

---

## 代码覆盖率

### 测试文件与模块映射

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/adapter_tests.rs` | adapter.rs (`to_models_kline` 转换函数) | 4 |
| `tests/integration_tests.rs` | 跨模块集成 | 9 |
| **合计** | | **13** |

### 已删除测试记录

| 已删除文件/模块 | 原测试数 | 删除原因 |
|----------------|---------|---------|
| `src/pe_adapter_tests.rs` | 32 | `pe_adapter.rs` 已删除（恒等转换中间层消除） |
| `src/adapter_tests.rs` 中 A1-A3/A5-A6 | 9 | 引用已删除的恒等转换函数（`to_ccxt_side` 等） |
| `src/paper_tests.rs` | 8 | `compute_paper_liquidation_price` 函数已移除 |
