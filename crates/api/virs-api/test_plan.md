# virs-api - 测试计划

**测试总数:** 40（单元测试: 32，集成测试: 8）

---

## 单元测试

### `crates/api/virs-api/src/ws_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `w1_1_position_all_fields` | 仓位转 WS JSON 全字段 | side=Long, status=Open, entry_price=50000.0, stop_loss=45000.0, take_profit=55000.0 | type=position_updated, symbol=BTCUSDT, side=long, status=open, quantity=1.0, entry_price=50000.0, stop_loss=45000.0, take_profit=55000.0 |
| 2 | `w1_2_position_optional_fields_none` | 仓位可选字段为 null | side=Short, status=Closed, stop_loss=None, take_profit=None | stop_loss=null, take_profit=null |
| 3 | `w1_3_position_type_field` | 仓位 JSON 的 type 字段 | side=Long, status=Open | json["type"]="position_updated" |
| 4 | `w2_1_kline_normal` | K 线事件转 WS JSON 正常字段 | KlineEvent: exchange=binance, symbol=BTCUSDT, timeframe=1m, candle.open=50000.0, close=50100.0, closed=false, event_type=Update | exchange=binance, symbol=BTCUSDT, timeframe=1m, candle.open=50000.0, candle.close=50100.0, candle.closed=false, event_type=Update |
| 5 | `w2_2_kline_event_types` | K 线事件类型映射 | Update, Closed, Backfilled 三种事件类型 | event_type 分别为 "Update", "Closed", "Backfilled" |
| 6 | `w2_3_kline_timeframe_format` | K 线事件时间周期格式 | timeframe=Timeframe::M1 | json["timeframe"]="1m" |
| 7 | `w3_1_orderbook_normal` | 订单簿事件转 WS JSON 正常 | 2 档买盘和 2 档卖盘, 首档 price=50000.0, amount=1.5 | exchange=binance, symbol=BTCUSDT, bids 长度=2, asks 长度=2, bids[0]=[50000.0, 1.5] |
| 8 | `w3_2_orderbook_empty_levels` | 订单簿空档位 | bids=[], asks=[] | bids 为空数组, asks 为空数组 |
| 9 | `w3_3_orderbook_level_format` | 订单簿档位格式 | 1 档买盘 price=50000.0, amount=1.5 | first_bid 为数组, [0]=50000.0, [1]=1.5 |

### `crates/api/virs-api/src/handlers/ai_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `ai1_1_deepseek_url` | DeepSeek 提供商基础 URL | provider="deepseek" | Some("https://api.deepseek.com") |
| 2 | `ai1_2_openai_url` | OpenAI 提供商基础 URL | provider="openai" | Some("https://api.openai.com/v1") |
| 3 | `ai1_3_openrouter_url` | OpenRouter 提供商基础 URL | provider="openrouter" | Some("https://openrouter.ai/api/v1") |
| 4 | `ai1_4_unknown_url` | 未知提供商 URL | provider="unknown" | None |
| 5 | `ai2_1_deepseek_model` | DeepSeek 默认模型 | provider="deepseek" | Some("deepseek-chat") |
| 6 | `ai2_2_openai_model` | OpenAI 默认模型 | provider="openai" | Some("gpt-4o") |
| 7 | `ai2_3_openrouter_model` | OpenRouter 默认模型 | provider="openrouter" | Some("deepseek/deepseek-chat") |
| 8 | `ai2_4_unknown_model` | 未知提供商模型 | provider="unknown" | None |

### `crates/api/virs-api/src/handlers/utils_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `t13_1_format_duration_zero_or_negative` | 零或负时长格式化 | 毫秒: 0, -1, -1000 | 均返回 "-" |
| 2 | `t13_2_format_duration_seconds` | 秒级时长格式化 | 毫秒: 1000, 5000, 59999 | "1s", "5s", "59s" |
| 3 | `t13_3_format_duration_minutes` | 分钟级时长格式化 | 毫秒: 60000, 65000, 3599999 | "1m0s", "1m5s", "59m59s" |
| 4 | `t13_4_format_duration_hours` | 小时级时长格式化 | 毫秒: 3600000, 3661000, 7320000 | "1h0m", "1h1m", "2h2m" |

### `crates/api/virs-api/src/handlers/ai_credentials_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `ac1_1_normal_models` | 正常模型列表解析 | data 数组含 2 个模型: deepseek-chat, deepseek-reasoner | 长度=2, models[0].id="deepseek-chat", owned_by="deepseek", models[1].id="deepseek-reasoner" |
| 2 | `ac1_2_empty_data` | null 数据解析 | json!(null) | 返回空列表 |
| 3 | `ac1_3_no_data_field` | 无 data 字段解析 | json!({"other": "field"}) | 返回空列表 |
| 4 | `ac2_1_balance_infos` | 余额信息解析（balance_infos 字段） | balance_infos 数组含 1 项: total_balance="100.50", currency="USD" | 长度=1, total_balance="100.50", currency="USD" |
| 5 | `ac2_2_data_fallback` | 余额解析回退到 data 字段 | data 数组含 1 项: total_balance="200.00", currency="CNY" | 长度=1, total_balance="200.00", currency="CNY" |
| 6 | `ac2_3_no_balance_fields` | 无余额字段解析 | json!({"other": "field"}) | 返回空列表 |

### `crates/api/virs-api/src/handlers/response_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `r1_1_ok_success_true` | 成功响应 success 为 true | ApiResponse::ok(json!({"key": "value"})) | resp.success=true |
| 2 | `r1_2_ok_data_preserved` | 成功响应保留 data | data=json!({"key": "value"}) | resp.data 与原 data 相等 |
| 3 | `r1_3_ok_message_none` | 成功响应 message 为 None | ApiResponse::ok(json!({})) | resp.message=None |
| 4 | `r2_1_err_success_false` | 错误响应 success 为 false | VirsError::bad_request("something went wrong") | err.success=false |
| 5 | `r2_2_err_message_set` | 错误响应 message 设置 | VirsError::bad_request("error message") | err.message="error message" |

## 集成测试

### `crates/api/virs-api/tests/integration_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `int_1_1_provider_url_model_consistency` | 提供商 URL 与模型一致性 | deepseek, openai, openrouter 三种提供商 | 三者 URL 和 model 均为 Some |
| 2 | `int_1_2_known_provider_roundtrip` | 已知提供商往返校验 | provider="deepseek" | url 以 "https://" 开头, model 非空 |
| 3 | `int_2_1_position_serde_then_ws_json` | 仓位序列化后转 WS JSON 链路 | side=Long 仓位, stop_loss/take_profit=None | json type=position_updated, symbol=BTCUSDT, side=long, 序列化字符串含 "position_updated" |
| 4 | `int_2_2_kline_event_to_json_chain` | K 线事件转 JSON 链路 | symbol=ETH/USDT, timeframe=H1, candle.close=3020.0, closed=true, event_type=Closed | exchange=binance, symbol=ETH/USDT, timeframe=1h, candle.close=3020.0, closed=true, event_type=Closed |
| 5 | `int_3_1_api_response_ok_then_serialize` | 成功响应序列化链路 | ApiResponse::ok(json!({"result": "success"})) | json success=true, data.result="success", message=null |
| 6 | `int_3_2_api_error_from_virs_error_then_serialize` | 错误响应序列化链路 | VirsError::bad_request("operation failed") | json success=false, status=400, message="operation failed" |
| 7 | `int_4_1_models_response_parse_chain` | 模型列表解析并包装响应链路 | data 数组含 2 个模型 | 解析长度=2, 每个模型包装为 ApiResponse 后 success=true |
| 8 | `int_4_2_balance_response_parse_chain` | 余额解析并包装响应链路 | balance_infos 含 1 项: total_balance="10.50", currency="USD" | 解析长度=1, 包装后 success=true, data.balances[0].total_balance="10.50" |
