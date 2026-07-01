# virs-api 测试用例文档

> 生成日期: 2026-07-01
> Crate: `crates/services/virs-api`
> 状态: **36 个测试全部通过** (28 单元 + 8 集成)

---

## 1. 抽取的幂等函数清单

### 1.1 handlers/ai.rs — AI Provider 解析函数

| # | 函数 | 签名 | 说明 |
|---|------|------|------|
| AI1 | `resolve_provider_base_url` | `(provider: &str) -> Option<&'static str>` | provider → base URL 映射 |
| AI2 | `resolve_provider_model` | `(provider: &str) -> Option<&'static str>` | provider → model 映射 |

> 从 `call_llm_with_fallback`、`test_credential`、`fetch_models` 中提取，消除 ai.rs 和 ai_credentials.rs 间的重复代码。

### 1.2 handlers/ai_credentials.rs — API 响应解析函数

| # | 函数 | 签名 | 说明 |
|---|------|------|------|
| AC1 | `parse_models_response` | `(data: &Value) -> Vec<Value>` | 解析 LLM /models API 响应 |
| AC2 | `parse_balance_response` | `(data: &Value) -> Vec<Value>` | 解析 LLM balance API 响应 |

> 从 `fetch_models`、`fetch_balance` 中提取内联解析逻辑。

### 1.3 ws.rs — WebSocket JSON 转换函数

| # | 函数 | 签名 | 说明 |
|---|------|------|------|
| W1 | `position_to_ws_json` | `(&Position) -> Value` | Position → WS JSON（消除 2 处重复） |
| W2 | `kline_event_to_json` | `(&KlineEvent) -> Value` | KlineEvent → WS JSON |
| W3 | `orderbook_event_to_json` | `(&OrderBookEvent) -> Value` | OrderBookEvent → WS JSON |

> 从 `handle_position_ws`、`handle_kline_ws`、`handle_orderbook_ws` 中提取内联 JSON 构建。

### 1.4 handlers/response.rs — API 响应构造

| # | 函数 | 签名 | 说明 |
|---|------|------|------|
| R1 | `ApiResponse::ok` | `(data: Value) -> Self` | 成功响应 |
| R2 | `ApiResponse::err` | `(msg: impl Into<String>) -> Self` | 错误响应 |

---

## 2. 单元测试用例

### 2.1 ai_tests.rs — Provider 解析 (8)

| ID | 测试函数 | 描述 |
|----|---------|------|
| AI1.1 | `ai1_1_deepseek_url` | "deepseek" → "https://api.deepseek.com" |
| AI1.2 | `ai1_2_openai_url` | "openai" → "https://api.openai.com/v1" |
| AI1.3 | `ai1_3_openrouter_url` | "openrouter" → "https://openrouter.ai/api/v1" |
| AI1.4 | `ai1_4_unknown_url` | "unknown" → None |
| AI2.1 | `ai2_1_deepseek_model` | "deepseek" → "deepseek-chat" |
| AI2.2 | `ai2_2_openai_model` | "openai" → "gpt-4o" |
| AI2.3 | `ai2_3_openrouter_model` | "openrouter" → "deepseek/deepseek-chat" |
| AI2.4 | `ai2_4_unknown_model` | "unknown" → None |

### 2.2 ai_credentials_tests.rs — API 响应解析 (6)

| ID | 测试函数 | 描述 |
|----|---------|------|
| AC1.1 | `ac1_1_normal_models` | 标准 /models 响应 → 提取 id + owned_by |
| AC1.2 | `ac1_2_empty_data` | data=null → 空列表 |
| AC1.3 | `ac1_3_no_data_field` | 无 data 字段 → 空列表 |
| AC2.1 | `ac2_1_balance_infos` | balance_infos 数组 → 提取 total_balance + currency |
| AC2.2 | `ac2_2_data_fallback` | 无 balance_infos, 有 data → 从 data 提取 |
| AC2.3 | `ac2_3_no_balance_fields` | 无匹配字段 → 空列表 |

### 2.3 ws_tests.rs — WS JSON 转换 (9)

| ID | 测试函数 | 描述 |
|----|---------|------|
| W1.1 | `w1_1_position_all_fields` | 完整 Position → JSON 字段一致 |
| W1.2 | `w1_2_position_optional_fields_none` | stop_loss=None → JSON null |
| W1.3 | `w1_3_position_type_field` | JSON type="position_updated" |
| W2.1 | `w2_1_kline_normal` | 完整 KlineEvent → JSON 字段一致 |
| W2.2 | `w2_2_kline_event_types` | Update/Closed/Backfilled → 正确字符串 |
| W2.3 | `w2_3_kline_timeframe_format` | timeframe → Display 字符串 |
| W3.1 | `w3_1_orderbook_normal` | 完整 OrderBookEvent → JSON |
| W3.2 | `w3_2_orderbook_empty_levels` | bids/asks 空 → 空数组 |
| W3.3 | `w3_3_orderbook_level_format` | bids → [[price, amount], ...] |

### 2.4 response_tests.rs — ApiResponse (5)

| ID | 测试函数 | 描述 |
|----|---------|------|
| R1.1 | `r1_1_ok_success_true` | ok() → success=true |
| R1.2 | `r1_2_ok_data_preserved` | ok(data) → data 一致 |
| R1.3 | `r1_3_ok_message_none` | ok() → message=None |
| R2.1 | `r2_1_err_success_false` | err() → success=false |
| R2.2 | `r2_2_err_message_set` | err("msg") → message=Some("msg") |

---

## 3. 集成测试用例

### integration_tests.rs (8)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-1.1 | `int_1_1_provider_url_model_consistency` | 已知 provider → URL + model 均 Some |
| INT-1.2 | `int_1_2_known_provider_roundtrip` | deepseek → URL 以 https:// 开头 + model 非空 |
| INT-2.1 | `int_2_1_position_serde_then_ws_json` | Position → position_to_ws_json → 序列化验证 |
| INT-2.2 | `int_2_2_kline_event_to_json_chain` | KlineEvent (H1/Closed) → kline_event_to_json → 字段验证 |
| INT-3.1 | `int_3_1_api_response_ok_then_serialize` | ApiResponse::ok → serde_json → success=true |
| INT-3.2 | `int_3_2_api_response_err_then_serialize` | ApiResponse::err → serde_json → success=false |
| INT-4.1 | `int_4_1_models_response_parse_chain` | /models JSON → parse_models_response → ApiResponse 包装 |
| INT-4.2 | `int_4_2_balance_response_parse_chain` | balance JSON → parse_balance_response → ApiResponse 包装 |

---

## 4. 代码覆盖率

### 测试文件与模块映射

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/handlers/ai_tests.rs` | handlers/ai.rs (2 函数) | 8 |
| `src/handlers/ai_credentials_tests.rs` | handlers/ai_credentials.rs (2 函数) | 6 |
| `src/ws_tests.rs` | ws.rs (3 函数) | 9 |
| `src/handlers/response_tests.rs` | handlers/response.rs (2 方法) | 5 |
| `tests/integration_tests.rs` | 跨模块链路 | 8 |
| **合计** | | **36** |

### 业务使用验证

| 函数 | 业务调用位置 | 调用次数 |
|------|-------------|---------|
| `resolve_provider_base_url` | ai.rs (call_llm_with_fallback), ai_credentials.rs (test_credential, fetch_models) | 3 |
| `resolve_provider_model` | ai.rs (call_llm_with_fallback), ai_credentials.rs (test_credential) | 2 |
| `parse_models_response` | ai_credentials.rs (fetch_models) | 1 |
| `parse_balance_response` | ai_credentials.rs (fetch_balance) | 1 |
| `position_to_ws_json` | ws.rs (handle_position_ws: 事件推送 + 快照推送) | 2 |
| `kline_event_to_json` | ws.rs (handle_kline_ws) | 1 |
| `orderbook_event_to_json` | ws.rs (handle_orderbook_ws) | 1 |
| `ApiResponse::ok` | 全部 handler 模块 | 多处 |
| `ApiResponse::err` | 全部 handler 模块 | 多处 |

### 回归审查

- clippy 零警告
- 无孤儿代码（所有提取函数均在非测试业务代码中被调用）
- 无重复代码（ai.rs 与 ai_credentials.rs 间的 provider 解析重复已消除）
- 无重复代码（ws.rs 中的 position JSON 构建重复已消除）
