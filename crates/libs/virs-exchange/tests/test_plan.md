# virs-exchange 测试用例文档

> 生成日期: 2026-07-01
> Crate: `crates/libs/virs-exchange`
> 状态: **68 个测试全部通过** (53 单元 + 15 集成)

---

## 单元测试用例

### adapter_tests.rs — CCXT 适配器转换函数 (13)

| ID | 测试函数 | 描述 |
|----|---------|------|
| A1.2 | `a1_2_perpetual_to_ccxt` | Perpetual → Perpetual |
| A2.1 | `a2_1_buy_to_ccxt` | Buy → Buy |
| A2.2 | `a2_2_sell_to_ccxt` | Sell → Sell |
| A3.1 | `a3_1_market_to_ccxt` | Market → Market |
| A3.2 | `a3_2_limit_to_ccxt` | Limit → Limit |
| A3.3 | `a3_3_stop_market_to_ccxt` | StopMarket → StopMarket |
| A3.4 | `a3_4_stop_limit_to_ccxt` | StopLimit → StopLimit |
| A3.5 | `a3_5_take_profit_market_to_ccxt` | TakeProfitMarket → TakeProfitMarket |
| A4.1 | `a4_1_kline_normal_conversion` | 正常 kline 转换 → 字段一致 |
| A4.2 | `a4_2_kline_exchange_field` | exchange 字段 + None → 默认值 |
| A5.1 | `a5_1_balance_normal` | 正常 balance 转换 → 字段一致 |
| A6.1 | `a6_1_order_normal` | 正常 order 转换 → 字段一致 |
| A6.2 | `a6_2_order_optional_fields_none` | Optional 字段为 None → 默认值 |

### paper_tests.rs — 模拟交易计算 (8)

| ID | 测试函数 | 描述 |
|----|---------|------|
| P1.1 | `p1_1_long_normal` | Long, entry=50000, lev=10 → 45000.0 |
| P1.2 | `p1_2_short_normal` | Short, entry=50000, lev=10 → 55000.0 |
| P1.3 | `p1_3_leverage_zero` | leverage=0 → None |
| P1.4 | `p1_4_entry_price_zero` | entry_price=0 → None |
| P1.5 | `p1_5_entry_price_negative` | entry_price<0 → None |
| P1.7 | `p1_7_leverage_one_long` | lev=1, Long → entry * 0 = 0.0 |
| P1.8 | `p1_8_leverage_one_short` | lev=1, Short → entry * 2 = 100000.0 |

### pe_adapter_tests.rs — PE 适配器转换函数 (32)

| ID | 测试函数 | 描述 |
|----|---------|------|
| PE1.1 | `pe1_1_buy` | models::Side::Buy → Side::Buy |
| PE1.2 | `pe1_2_sell` | models::Side::Sell → Side::Sell |
| PE2.1 | `pe2_1_buy` | Side::Buy → models::Side::Buy |
| PE2.2 | `pe2_2_sell` | Side::Sell → models::Side::Sell |
| PE3.1 | `pe3_1_long` | Some(Long) → Some(Long) |
| PE3.2 | `pe3_2_short` | Some(Short) → Some(Short) |
| PE3.4 | `pe3_4_none` | None → None |
| PE4.1 | `pe4_1_market` | Market → Market |
| PE4.2 | `pe4_2_limit` | Limit → Limit |
| PE4.3 | `pe4_3_stop_market` | StopMarket → StopMarket |
| PE4.4 | `pe4_4_stop_limit` | StopLimit → StopLimit |
| PE4.5 | `pe4_5_take_profit_market` | TakeProfitMarket → TakeProfitMarket |
| PE5.1 | `pe5_1_open` | Open → Open |
| PE5.2 | `pe5_2_partially_filled` | PartiallyFilled → PartiallyFilled |
| PE5.3 | `pe5_3_filled` | Filled → Filled |
| PE5.4 | `pe5_4_canceled` | Canceled → Canceled |
| PE5.5 | `pe5_5_failed` | Failed → Failed |
| PE5.6 | `pe5_6_pending` | Pending → Pending |
| PE6.1 | `pe6_1_long` | models Long → PE Long |
| PE6.2 | `pe6_2_short` | models Short → PE Short |
| PE7.2 | `pe7_2_perpetual` | models Perpetual → PE Perpetual |
| PE8.1 | `pe8_1_order_normal` | 正常 order → PositionOrder 字段一致 |
| PE8.2 | `pe8_2_order_no_fill_no_price` | filled=0 + price=None → fill_price=None |
| PE9.1 | `pe9_1_position_normal` | 正常 position → ExchangePosition 字段一致 |
| PE9.2 | `pe9_2_position_no_liquidation` | liquidation_price=None → 正确处理 |
| PE10.1 | `pe10_1_order_update` | OrderUpdate 事件 → 正确转换 |
| PE10.2 | `pe10_2_connection_changed` | ConnectionChanged 事件 → 正确转换 |
| PE11.1 | `pe11_1_network_error` | anyhow 错误 → Exchange 变体 |
| PE11.2 | `pe11_2_auth_error` | anyhow 错误 → Exchange 变体 |
| PE12.1 | `pe12_1_no_exchange_error` | 返回 "No perpetual exchange" 错误消息 |

---

## 集成测试用例

### integration_tests.rs (15)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-1.1 | `int_1_1_side_roundtrip` | Side 类型一致性验证 |
| INT-1.2 | `int_1_2_order_type_roundtrip` | OrderType 类型一致性验证 |
| INT-1.3 | `int_1_3_position_side_consistency` | PositionSide 类型一致性验证 |
| INT-1.4 | `int_1_4_market_type_consistency` | MarketType 类型一致性验证 |
| INT-2.1 | `int_2_1_paper_exchange_creation_and_balance` | Paper 创建 + 初始余额验证 |
| INT-2.2 | `int_2_2_paper_market_order_updates_balance` | Paper 市价单 → 余额减少 + 持仓创建 |
| INT-3.1 | `int_3_1_registry_register_and_get` | 空注册表 get → None |
| INT-3.2 | `int_3_2_registry_get_nonexistent` | 不存在的 key → None |
| INT-3.3 | `int_3_3_registry_list_names` | 空注册表 → 空列表 |
| INT-4.1 | `int_4_1_exchange_error_to_pe_error` | anyhow 错误 → PE Exchange 错误 |
| INT-4.2 | `int_4_2_no_exchange_error` | no_exchange_error → 包含 "No perpetual exchange" |
| INT-5.1 | `int_5_1_ws_order_update_conversion` | WsFeedEvent::OrderUpdate 跨层转换 |
| INT-5.2 | `int_5_2_ws_connection_changed_conversion` | WsFeedEvent::ConnectionChanged 跨层转换 |
| INT-6.1 | `int_6_1_order_conversion_full_chain` | models::Order → PositionOrder 完整链路 |
| INT-6.2 | `int_6_2_position_conversion_full_chain` | models::ExchangePosition → ExchangePosition 完整链路 |

---

## 代码覆盖率

### 测试文件与模块映射

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/adapter_tests.rs` | adapter.rs (6 个转换函数) | 13 |
| `src/paper_tests.rs` | paper.rs (compute_paper_liquidation_price) | 8 |
| `src/pe_adapter_tests.rs` | pe_adapter.rs (12 个转换函数) | 32 |
| `tests/integration_tests.rs` | 跨模块集成 | 15 |
| **合计** | | **68** |

### 死代码清理记录

无死代码。全部 19 个公开函数均在 virs-exchange 生产代码（`CcxtAdapter`/`CcxtExchangeAdapter`/`PaperExchangeAdapter` 的 trait 实现）中被调用，非仅测试引用。

### test_plan.md 修正记录

| 修正项 | 原文档 | 实际代码 | 修正方向 |
|--------|--------|---------|---------|
| PE5.6 | "Rejected → Failed" | "Pending → Pending" | 文档错误，`models::OrderStatus` 无 `Rejected` 变体 |
| PE3 | 3 个用例 | 4 个用例 (含 PE3.4 None) | 文档遗漏 |
| PE11 | 3 个用例 (含 OrderNotFound) | 2 个用例 | 文档多余，`to_pe_error` 接收 `anyhow::Error` 非分类型 |
| PE12 签名 | `no_exchange_error(name: &str)` | `no_exchange_error()` | 文档签名错误，函数无参数 |
| INT 编号 | INT-4 为 CcxtAdapter 一致性 | INT-4 为错误处理 | 文档编号与实际不匹配 |
| INT-2 | 4 个用例 | 2 个用例 | 文档多计 |
