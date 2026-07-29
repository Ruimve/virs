# virs-app 测试用例文档

> Crate: `app/virs-app`
> 状态: **26 个测试全部通过** (19 单元 + 7 集成)

---

## 1. 单元测试用例

### 1.1 auto_store_tests.rs — AutoBot 转换 (4)

| ID | 测试函数 | 描述 |
|----|---------|------|
| A1.1 | `a1_1_bot_to_config_basic` | 基础字段映射正确 |
| A1.3 | `a1_3_bot_to_config_optional_fields` | None 字段保持 None |
| A1.4 | `a1_4_bot_to_config_stats` | 统计字段 (pnl/trades) 精确映射 |
| A1.5 | `a1_5_bot_to_config_id_preserved` | UUID 保持一致 |

### 1.2 market_data_tests.rs — Candle 转换 (4)

| ID | 测试函数 | 描述 |
|----|---------|------|
| M1.1 | `m1_1_candle_to_kline_basic` | OHLCV 字段映射正确 |
| M1.2 | `m1_2_candle_to_kline_timestamps` | open_time/close_time 保持一致 |
| M1.3 | `m1_3_candle_to_kline_zero_values` | 零值字段正确处理 |
| M1.4 | `m1_4_candle_to_kline_metadata_empty` | symbol/exchange/interval 为空字符串 |

### 1.3 order_executor_tests.rs — 事件转换 (6)

| ID | 测试函数 | 描述 |
|----|---------|------|
| O1.1 | `o1_1_convert_order_placed` | OrderPlaced → OrderEvent::OrderPlaced |
| O1.2 | `o1_2_convert_order_filled` | OrderFilled → OrderEvent::OrderFilled |
| O1.3 | `o1_3_convert_order_canceled` | OrderCanceled → OrderEvent::OrderCanceled |
| O1.4 | `o1_4_convert_order_failed` | OrderFailed → OrderEvent::OrderFailed |
| O1.5 | `o1_5_convert_risk_alert` | RiskAlert → OrderEvent::RiskAlert |
| O1.6 | `o1_6_convert_position_opened_none` | PositionOpened → None (不映射) |

### 1.4 llm_resolver_tests.rs — LLM 解析 (5)

| ID | 测试函数 | 描述 |
|----|---------|------|
| L1.1 | `l1_1_resolve_deepseek_user_cred` | deepseek 凭证 → key/url/model/provider 正确 |
| L1.2 | `l1_2_resolve_openai_user_cred` | openai 凭证 → key/url/model/provider 正确 |
| L1.3 | `l1_3_resolve_openrouter_user_cred` | openrouter 凭证 → 默认 model |
| L1.4 | `l1_4_resolve_no_key_error` | 空凭证列表 → Err |
| L1.5 | `l1_5_resolve_deepseek_priority_over_openai` | deepseek 优先于 openai |

---

## 2. 集成测试用例

### integration_tests.rs (7)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-1.2 | `int_1_2_auto_bot_to_config_then_compare` | AutoBot 转换后字段一致性 |
| INT-2.1 | `int_2_1_candle_to_kline_preserves_ohlcv` | Candle→Kline OHLCV 保持一致 |
| INT-3.1 | `int_3_1_llm_resolve_priority_chain` | deepseek > openai > openrouter 优先级链 |
| INT-3.2 | `int_3_2_llm_resolve_user_model_override` | 用户 model 覆盖默认 model |
| INT-4.1 | `int_4_1_convert_event_order_placed_filled` | OrderPlaced + OrderFilled 事件链 |
| INT-4.2 | `int_4_2_convert_event_canceled_failed` | OrderCanceled + OrderFailed 事件链 |
| INT-6.2 | `int_6_2_llm_resolve_default_models` | 各 provider 默认 model 正确 |

---

## 3. 代码覆盖率

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/adapters/auto_store_tests.rs` | adapters/auto_store.rs | 4 |
| `src/adapters/market_data_tests.rs` | adapters/market_data.rs | 4 |
| `src/adapters/order_executor_tests.rs` | adapters/order_executor.rs | 6 |
| `src/adapters/llm_resolver_tests.rs` | adapters/llm_resolver.rs | 5 |
| `tests/integration_tests.rs` | 跨模块链路 | 7 |
| **合计** | | **26** |
