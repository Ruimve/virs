# server - 测试计划

**测试总数:** 26（单元测试: 19，集成测试: 7）

---

## 单元测试

### `apps/server/src/adapters/chat_store_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `a1_1_bot_to_config_basic` | ChatBot 转配置基本字段 | bot: name=chat-test, symbol=ETHUSDT, exchange=okx, paper_mode=true, leverage=10 | config.name=chat-test, symbol=ETHUSDT, exchange=okx, paper_mode=true, leverage=10 |
| 2 | `a1_3_bot_to_config_optional_fields` | ChatBot 转配置可选字段为 None | bot 各可选字段均为 None | position_id_long/short、market_regime、ai_analysis、system_prompt、user_prompt、last_decided_at 均为 None |
| 3 | `a1_4_bot_to_config_stats` | ChatBot 转配置统计字段 | total_pnl=123.45, total_trades=10, win_trades=7, loss_trades=3 | config.total_pnl=123.45, total_trades=10, win_trades=7, loss_trades=3 |
| 4 | `a1_5_bot_to_config_id_preserved` | ChatBot 转配置保留 ID | bot.id 和 bot.user_id | config.id 与 bot.id 相等, config.user_id 与 bot.user_id 相等 |

### `apps/server/src/adapters/order_executor_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `o1_1_convert_order_placed` | 转换订单已下事件 | EngineEvent::OrderPlaced, side=Buy, symbol=BTCUSDT, avg_fill_price=101.0 | 返回 Some, OrderEvent::OrderPlaced, side=Buy, symbol=BTCUSDT, fill_price=101.0 |
| 2 | `o1_2_convert_order_filled` | 转换订单已成交事件 | EngineEvent::OrderFilled, side=Sell | 返回 Some, OrderEvent::OrderFilled, side=Sell |
| 3 | `o1_3_convert_order_canceled` | 转换订单已取消事件 | EngineEvent::OrderCanceled, order_id=123, client_order_id=CL456, symbol=BTCUSDT | 返回 Some, OrderEvent::OrderCanceled, order_id=123, client_order_id=Some, symbol=BTCUSDT |
| 4 | `o1_4_convert_order_failed` | 转换订单失败事件 | EngineEvent::OrderFailed, client_order_id=CL456, reason="Insufficient balance" | 返回 Some, OrderEvent::OrderFailed, reason="Insufficient balance", client_order_id=CL456 |
| 5 | `o1_5_convert_risk_alert` | 转换风险告警事件 | EngineEvent::RiskAlert, level=critical, message="Max drawdown exceeded" | 返回 Some, OrderEvent::RiskAlert, level=critical, message="Max drawdown exceeded" |
| 6 | `o1_6_convert_position_opened_none` | 转换仓位已开事件返回 None | EngineEvent::PositionOpened | 返回 None |

### `apps/server/src/adapters/market_data_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `m1_1_candle_to_kline_basic` | Candle 转 Kline 基本 OHLCV | open=100.0, high=105.0, low=95.0, close=102.0, volume=500.0, quote_volume=51000.0, trades=120 | k.open=100.0, high=105.0, low=95.0, close=102.0, volume=500.0, quote_volume=51000.0, trades=120 |
| 2 | `m1_2_candle_to_kline_timestamps` | Candle 转 Kline 时间戳 | open_time=1700000000000, close_time=1700000059999 | k.open_time=1700000000000, k.close_time=1700000059999 |
| 3 | `m1_3_candle_to_kline_zero_values` | Candle 零值转 Kline | 全零字段 Candle | k.open/high/low/close=0.0, trades=0 |
| 4 | `m1_4_candle_to_kline_metadata_empty` | Candle 转 Kline 元数据为空 | 正常 Candle | k.symbol/exchange/interval 均为空字符串 |

### `apps/server/src/adapters/llm_resolver_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `l1_1_resolve_deepseek_user_cred` | DeepSeek 用户凭证解析 | creds: [(deepseek, user-key-123, Some(deepseek-reasoner))] | key=user-key-123, url="https://api.deepseek.com", model="deepseek-reasoner", provider=deepseek |
| 2 | `l1_2_resolve_openai_user_cred` | OpenAI 用户凭证解析 | creds: [(openai, user-openai-key, Some(gpt-4o-mini))] | key=user-openai-key, url="https://api.openai.com/v1", model="gpt-4o-mini", provider=openai |
| 3 | `l1_3_resolve_openrouter_user_cred` | OpenRouter 用户凭证解析（默认模型） | creds: [(openrouter, user-or-key, None)] | key=user-or-key, url="https://openrouter.ai/api/v1", model="deepseek/deepseek-chat", provider=openrouter |
| 4 | `l1_4_resolve_no_key_error` | 无凭证返回错误 | 空凭证列表 | 返回 Err |
| 5 | `l1_5_resolve_deepseek_priority_over_openai` | DeepSeek 优先级高于 OpenAI | creds: [(openai, openai-key, None), (deepseek, deepseek-key, None)] | key=deepseek-key, provider=deepseek |

## 集成测试

### `apps/server/tests/integration_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `int_1_2_chat_bot_to_config_then_compare` | ChatBot 转配置后全字段比对 | bot: 含 position_id、market_regime、ai_analysis 等非空字段, total_pnl=250.0, total_trades=8 | config 各字段（id/name/symbol/exchange/leverage/position_id/market_regime/total_pnl 等）与原 bot 相等 |
| 2 | `int_2_1_candle_to_kline_preserves_ohlcv` | Candle 转 Kline 保留完整 OHLCV | open=42000.0, high=42500.0, low=41800.0, close=42300.0, volume=1000.0, trades=500 | k 各 OHLCV 字段及时间戳与原 Candle 相等 |
| 3 | `int_3_1_llm_resolve_priority_chain` | LLM 提供商优先级链路 | 三组凭证: [openai,deepseek,openrouter]、[openai,openrouter]、[openrouter] | 依次返回 deepseek、openai、openrouter |
| 4 | `int_3_2_llm_resolve_user_model_override` | LLM 用户自定义模型覆盖 | creds: [(deepseek, user-ds, Some(deepseek-reasoner)), (openai, user-oai, Some(gpt-4o-mini))] | key=user-ds, model="deepseek-reasoner", provider=deepseek |
| 5 | `int_4_1_convert_event_order_placed_filled` | 订单已下和已成交事件转换链路 | order_id=999, side=Buy, symbol=BTCUSDT | OrderPlaced 匹配 Some, OrderFilled 的 order.id=999, side=Buy |
| 6 | `int_4_2_convert_event_canceled_failed` | 订单已取消和失败事件转换链路 | OrderCanceled: order_id=999; OrderFailed: client_order_id=CL999, reason=timeout | OrderCanceled order_id=999, client_order_id=Some, symbol=BTCUSDT; OrderFailed reason=timeout, client_order_id=CL999 |
| 7 | `int_6_2_llm_resolve_default_models` | LLM 各提供商默认模型 | 单提供商凭证: deepseek/openai/openrouter（model=None） | 默认模型分别为 deepseek-chat、gpt-4o、deepseek/deepseek-chat |
