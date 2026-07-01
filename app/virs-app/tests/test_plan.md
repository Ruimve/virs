# virs-app 测试用例文档

> Crate: `app/virs-app`
> 状态: **43 个测试全部通过** (31 单元 + 12 集成)

---

## 1. 单元测试用例

### 1.1 utils_tests.rs — 共享工具函数 (6)

| ID | 测试函数 | 描述 |
|----|---------|------|
| U1.1 | `u1_1_sanitize_pnl_pct_normal` | 正常值 → 原值 |
| U1.2 | `u1_2_sanitize_pnl_pct_nan` | NaN → 0.0 |
| U1.3 | `u1_3_sanitize_pnl_pct_zero` | 0.0 → 0.0 |
| U1.4 | `u1_4_sanitize_pnl_pct_negative` | 负值 → 原值 |
| U2.1 | `u2_1_derive_open_side_buy` | "buy" → "sell" |
| U2.2 | `u2_2_derive_open_side_sell` | "sell" → "buy" |

### 1.2 grid_store_tests.rs — GridBot 转换 (5)

| ID | 测试函数 | 描述 |
|----|---------|------|
| G1.1 | `g1_1_bot_to_config_basic` | 基础字段映射正确 |
| G1.2 | `g1_2_bot_to_config_optional_fields` | None 字段保持 None |
| G1.3 | `g1_3_bot_to_config_some_fields` | Some 字段正确映射 |
| G1.4 | `g1_4_bot_to_config_numeric` | 数值字段精确映射 |
| G1.5 | `g1_5_bot_to_config_id_preserved` | UUID 保持一致 |

### 1.3 auto_store_tests.rs — AutoBot 转换 (5)

| ID | 测试函数 | 描述 |
|----|---------|------|
| A1.1 | `a1_1_bot_to_config_basic` | 基础字段映射正确 |
| A1.2 | `a1_2_bot_to_config_market_type` | market_type 字符串 → AutoMarketType |
| A1.3 | `a1_3_bot_to_config_optional_fields` | None 字段保持 None |
| A1.4 | `a1_4_bot_to_config_stats` | 统计字段 (pnl/trades) 精确映射 |
| A1.5 | `a1_5_bot_to_config_id_preserved` | UUID 保持一致 |

### 1.4 market_data_tests.rs — Candle 转换 (4)

| ID | 测试函数 | 描述 |
|----|---------|------|
| M1.1 | `m1_1_candle_to_kline_basic` | OHLCV 字段映射正确 |
| M1.2 | `m1_2_candle_to_kline_timestamps` | open_time/close_time 保持一致 |
| M1.3 | `m1_3_candle_to_kline_zero_values` | 零值字段正确处理 |
| M1.4 | `m1_4_candle_to_kline_metadata_empty` | symbol/exchange/interval 为空字符串 |

### 1.5 order_executor_tests.rs — 事件转换 (6)

| ID | 测试函数 | 描述 |
|----|---------|------|
| O1.1 | `o1_1_convert_order_placed` | OrderPlaced → OrderEvent::OrderPlaced |
| O1.2 | `o1_2_convert_order_filled` | OrderFilled → OrderEvent::OrderFilled |
| O1.3 | `o1_3_convert_order_canceled` | OrderCanceled → OrderEvent::OrderCanceled |
| O1.4 | `o1_4_convert_order_failed` | OrderFailed → OrderEvent::OrderFailed |
| O1.5 | `o1_5_convert_risk_alert` | RiskAlert → OrderEvent::RiskAlert |
| O1.6 | `o1_6_convert_position_opened_none` | PositionOpened → None (不映射) |

### 1.6 llm_resolver_tests.rs — LLM 解析 (5)

| ID | 测试函数 | 描述 |
|----|---------|------|
| L1.1 | `l1_1_resolve_deepseek_user_cred` | 用户 deepseek 凭证优先 |
| L1.2 | `l1_2_resolve_openai_fallback` | 无 deepseek → openai |
| L1.3 | `l1_3_resolve_openrouter_fallback` | 无 deepseek/openai → openrouter |
| L1.4 | `l1_4_resolve_no_key_error` | 无任何 key → Err |
| L1.5 | `l1_5_resolve_user_cred_overrides_config` | 用户凭证覆盖配置 |

---

## 2. 集成测试用例

### integration_tests.rs (12)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-1.1 | `int_1_1_grid_bot_to_config_then_compare` | GridBot 转换后字段一致性 |
| INT-1.2 | `int_1_2_auto_bot_to_config_then_compare` | AutoBot 转换后字段一致性 |
| INT-2.1 | `int_2_1_candle_to_kline_preserves_ohlcv` | Candle→Kline OHLCV 保持一致 |
| INT-2.2 | `int_2_2_sanitize_then_derive_chain` | sanitize_pnl_pct → derive_open_side 链路 |
| INT-3.1 | `int_3_1_llm_resolve_priority_chain` | deepseek > openai > openrouter 优先级链 |
| INT-3.2 | `int_3_2_llm_resolve_user_overrides_config` | 用户凭证覆盖系统配置 |
| INT-4.1 | `int_4_1_convert_event_order_placed_filled` | OrderPlaced + OrderFilled 事件链 |
| INT-4.2 | `int_4_2_convert_event_canceled_failed` | OrderCanceled + OrderFailed 事件链 |
| INT-5.1 | `int_5_1_sanitize_all_pnl_cases` | NaN/0/正/负 全覆盖 |
| INT-5.2 | `int_5_2_derive_open_side_all_cases` | buy/sell/unknown 全覆盖 |
| INT-6.1 | `int_6_1_grid_auto_bot_to_config_consistency` | 两种 bot_to_config 不会互相干扰 |
| INT-6.2 | `int_6_2_llm_resolve_default_models` | 各 provider 默认 model 正确 |

---

## 3. 代码覆盖率

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/adapters/utils_tests.rs` | adapters/utils.rs | 6 |
| `src/adapters/grid_store_tests.rs` | adapters/grid_store.rs | 5 |
| `src/adapters/auto_store_tests.rs` | adapters/auto_store.rs | 5 |
| `src/adapters/market_data_tests.rs` | adapters/market_data.rs | 4 |
| `src/adapters/order_executor_tests.rs` | adapters/order_executor.rs | 6 |
| `src/adapters/llm_resolver_tests.rs` | adapters/llm_resolver.rs | 5 |
| `tests/integration_tests.rs` | 跨模块链路 | 12 |
| **合计** | | **43** |
