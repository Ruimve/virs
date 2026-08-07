# virs-ccxt - 测试计划

**测试总数:** 133（单元测试: 119，集成测试: 14）

---

## 单元测试

### `crates/core/virs-ccxt/src/auth_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `a1_1_hmac_sha256_hex_known_vector` | HMAC-SHA256 已知向量验证 | key: `NhqPtmdSJYdKjVHjA7PZj4Mge3R5YNiP1e3UZjInClVN65XAbvqqM6A7H5fATj0j`，msg: 订单参数字符串 | 签名=`c8db56825ae71d6d79447849e617115f4a920fa2acdcab2b053c4b2838bd6b71` |
| 2 | `a1_2_hmac_sha256_hex_empty_message` | 空消息 HMAC 签名 | key: `secret`，msg: `""` | 签名长度=64，全为十六进制字符 |
| 3 | `a1_3_hmac_sha256_hex_empty_key` | 空密钥 HMAC 签名 | key: `""`，msg: `message` | 签名长度=64，全为十六进制字符 |
| 4 | `a1_4_hmac_sha256_hex_idempotent` | HMAC 签名幂等性 | key: `test_key`，msg: `test_message`，调用两次 | 两次签名结果完全相同 |
| 5 | `a1_5_hmac_sha256_hex_different_inputs` | 不同密钥产生不同签名 | key1: `key1`，key2: `key2`，msg: `msg` | sig1 != sig2 |
| 6 | `a3_1_make_header_valid_ascii` | 合法 ASCII 值构造请求头 | header 名: `x-mbx-apikey`，值: `abcdef123456` | 返回 Ok，值=`abcdef123456` |
| 7 | `a3_2_make_header_invalid_chars` | 含换行符的非法值构造请求头 | header 名: `x-custom`，值: `bad\nvalue` | 返回 Err |
| 8 | `a4_1_insert_header_success` | 成功插入请求头 | header 名: `x-mbx-apikey`，值: `test_key` | 返回 Ok，headers 中包含 `test_key` |
| 9 | `a4_2_insert_header_invalid_value` | 非法值插入请求头失败 | header 名: `x-custom`，值: `bad\r\nvalue` | 返回 Err，headers 中不包含该值 |

### `crates/core/virs-ccxt/src/types_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `t1_1_new_to_open` | CcxtOrderStatus::New 转换为应用层状态 | `CcxtOrderStatus::New` | `OrderStatus::Open` |
| 2 | `t1_2_partially_filled` | CcxtOrderStatus::PartiallyFilled 转换 | `CcxtOrderStatus::PartiallyFilled` | `OrderStatus::PartiallyFilled` |
| 3 | `t1_3_filled` | CcxtOrderStatus::Filled 转换 | `CcxtOrderStatus::Filled` | `OrderStatus::Filled` |
| 4 | `t1_4_canceled` | CcxtOrderStatus::Canceled 转换 | `CcxtOrderStatus::Canceled` | `OrderStatus::Canceled` |
| 5 | `t1_5_expired` | CcxtOrderStatus::Expired 转换 | `CcxtOrderStatus::Expired` | `OrderStatus::Expired` |
| 6 | `t1_6_expired_in_match` | ExpiredInMatch 映射为 Expired | `CcxtOrderStatus::ExpiredInMatch` | `OrderStatus::Expired` |
| 7 | `t7_1_funding_time_zero_is_epoch` | 时间戳 0 为有效 epoch 时间 | timestamp_millis: `0` | 返回 Some，等于 epoch 时间 |
| 8 | `t7_2_filter_zero_before_from_timestamp_millis` | 过滤 0 时间戳保留有效时间戳 | raw_ts: `0`，valid_ts: `1700000000000` | 0 被过滤为 None，1700000000000 保留为 Some |

### `crates/core/virs-ccxt/src/lib_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `l1_1_parse_f64_from_number` | 从 JSON 数字解析 f64 | `{"price": 12345.67}` | `Some(12345.67)` |
| 2 | `l1_2_parse_f64_from_string` | 从 JSON 字符串解析 f64 | `{"price": "12345.67"}` | `Some(12345.67)` |
| 3 | `l1_3_parse_f64_missing_field` | 字段缺失返回 None | `{"other": 1}` | `None` |
| 4 | `l1_4_parse_f64_null_field` | null 字段返回 None | `{"price": null}` | `None` |
| 5 | `l1_5_parse_f64_invalid_string` | 无效字符串返回 None | `{"price": "abc"}` | `None` |
| 6 | `l1_6_parse_f64_from_integer` | 从整数解析 f64 | `{"price": 42}` | `Some(42.0)` |
| 7 | `l2_1_parse_str_from_string` | 从 JSON 字符串解析 str | `{"symbol": "BTCUSDT"}` | `Some("BTCUSDT")` |
| 8 | `l2_2_parse_str_from_i64` | 从 i64 解析 str | `{"count": 12345}` | `Some("12345")` |
| 9 | `l2_3_parse_str_from_f64` | 从 f64 解析 str | `{"price": 2.5}` | `Some`，可解析回 2.5 |
| 10 | `l2_4_parse_str_missing_field` | 字段缺失返回 None | `{"other": "x"}` | `None` |
| 11 | `l4_1_parse_u32_from_u64` | 从 JSON 数字解析 u32 | `{"leverage": 10}` | `Some(10)` |
| 12 | `l4_2_parse_u32_from_string` | 从 JSON 字符串解析 u32 | `{"leverage": "20"}` | `Some(20)` |
| 13 | `l4_3_parse_u32_missing_field` | 字段缺失返回 None | `{"other": 1}` | `None` |
| 14 | `l5_1_build_display_url_no_params` | 无参数构建显示 URL | 路径: `/fapi/v1/ping`，空参数 | `/fapi/v1/ping` |
| 15 | `l5_2_build_display_url_with_params` | 带参数构建显示 URL | 路径: `/fapi/v1/depth`，参数: symbol=BTCUSDT, limit=100 | `/fapi/v1/depth?symbol=BTCUSDT&limit=100` |
| 16 | `l5_3_build_display_url_masks_signature` | URL 中签名字段被掩码 | 参数含 signature=abcdef123456 | URL 含 `***MASKED***`，不含 `abcdef123456` |
| 17 | `l5_4_build_display_url_empty_params` | 空参数数组构建 URL | 路径: `/fapi/v1/ping`，空数组 | `/fapi/v1/ping` |
| 18 | `l6_1_mask_signature_basic` | 基本签名掩码 | body: `symbol=BTCUSDT&signature=abcdef123456` | 含 `signature=***MASKED***`，不含 `abcdef123456` |
| 19 | `l6_2_mask_signature_with_trailing_params` | 带尾部参数的签名掩码 | body: `symbol=BTCUSDT&signature=abcdef&timestamp=123` | signature 被掩码，timestamp=123 保留 |
| 20 | `l6_3_mask_signature_no_signature` | 无签名的 body 原样返回 | body: `symbol=BTCUSDT&timestamp=123` | 原样返回 |
| 21 | `l6_4_mask_signature_multiple_signatures` | 多个签名字段掩码 | body: `signature=aaa&signature=bbb` | 含 `***MASKED***`，含 `bbb`（第二个未掩码） |
| 22 | `l7_1_extract_error_msg_with_code` | 提取含 code 的错误消息 | `{"code": -1121, "msg": "Invalid symbol."}` | `[-1121] Invalid symbol.` |
| 23 | `l7_2_extract_error_msg_only` | 仅含 msg 字段提取错误 | `{"msg": "Some error"}` | `Some error` |
| 24 | `l7_3_extract_error_bybit_format` | 提取 Bybit 格式错误 | `{"retCode": 10001, "retMsg": "error"}` | `[10001] error` |
| 25 | `l7_4_extract_error_error_field` | 提取 error 字段 | `{"error": "Not found"}` | `Not found` |
| 26 | `l7_5_extract_error_message_field` | 提取 message 字段 | `{"message": "Bad request"}` | `Bad request` |
| 27 | `l7_6_extract_error_detail_field` | 提取 detail 字段 | `{"detail": "Validation failed"}` | `Validation failed` |
| 28 | `l7_7_extract_error_no_matching_field` | 无匹配字段返回原始 JSON | `{"foo": "bar"}` | `{"foo":"bar"}` |
| 29 | `l8_1_parse_timestamp_ms_from_i64` | 从 i64 解析毫秒时间戳 | `{"time": 1713182400000}` | Some，timestamp_millis=1713182400000 |
| 30 | `l8_2_parse_timestamp_ms_from_string` | 从字符串解析毫秒时间戳 | `{"transactTime": "1713182400000"}` | Some，timestamp_millis=1713182400000 |
| 31 | `l8_3_parse_timestamp_ms_missing_field` | 字段缺失返回 None | `{"other": 1}` | `None` |
| 32 | `l8_4_parse_timestamp_ms_invalid_string` | 无效字符串返回 None | `{"time": "not_a_number"}` | `None` |

### `crates/core/virs-ccxt/src/errors_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `e2_1_no_data_construction` | NoData 错误构造 | msg: `No ticker found for BTCUSDT` | 匹配 `ExchangeError::NoData` 变体，msg 正确 |

### `crates/core/virs-ccxt/src/adapter/binance/mod_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `b3_1_status_new` | 解析 NEW 状态 | `NEW` | `CcxtOrderStatus::New` |
| 2 | `b3_2_status_partially_filled` | 解析 PARTIALLY_FILLED 状态 | `PARTIALLY_FILLED` | `CcxtOrderStatus::PartiallyFilled` |
| 3 | `b3_3_status_filled` | 解析 FILLED 状态 | `FILLED` | `CcxtOrderStatus::Filled` |
| 4 | `b3_4_status_canceled` | 解析 CANCELED 状态 | `CANCELED` | `CcxtOrderStatus::Canceled` |
| 5 | `b3_5_status_cancelled_variant` | 解析英式拼写 CANCELLED | `CANCELLED` | `CcxtOrderStatus::Canceled` |
| 6 | `b3_6_status_expired` | 解析 EXPIRED 状态 | `EXPIRED` | `CcxtOrderStatus::Expired` |
| 7 | `b3_7_status_expired_in_match` | 解析 EXPIRED_IN_MATCH 状态 | `EXPIRED_IN_MATCH` | `CcxtOrderStatus::ExpiredInMatch` |
| 8 | `b3_8_status_unknown_returns_unknown` | 未知状态返回 Unknown | `UNKNOWN` | `CcxtOrderStatus::Unknown("UNKNOWN")` |
| 9 | `b4_1_type_market` | 解析 MARKET 订单类型 | `MARKET` | `OrderType::Market` |
| 10 | `b4_2_type_limit` | 解析 LIMIT 订单类型 | `LIMIT` | `OrderType::Limit` |
| 11 | `b4_3_type_stop_market` | 解析 STOP_MARKET 订单类型 | `STOP_MARKET` | `OrderType::StopMarket` |
| 12 | `b4_4_type_stop` | 解析 STOP 订单类型 | `STOP` | `OrderType::Stop` |
| 13 | `b4_5_type_trailing_stop_market` | 解析 TRAILING_STOP_MARKET 订单类型 | `TRAILING_STOP_MARKET` | `OrderType::TrailingStopMarket` |
| 14 | `b4_6_type_liquidation` | 解析 LIQUIDATION 订单类型 | `LIQUIDATION` | `OrderType::Liquidation` |
| 15 | `b4_7_type_take_profit_market` | 解析 TAKE_PROFIT_MARKET 订单类型 | `TAKE_PROFIT_MARKET` | `OrderType::TakeProfitMarket` |
| 16 | `b4_7b_type_take_profit` | 解析 TAKE_PROFIT 订单类型 | `TAKE_PROFIT` | `OrderType::TakeProfit` |
| 17 | `b4_8_type_unknown_returns_unknown` | 未知订单类型返回 Unknown | `UNKNOWN` | `OrderType::Unknown("UNKNOWN")` |
| 18 | `b5_1_side_buy` | Buy 方向转字符串 | `Side::Buy` | `BUY` |
| 19 | `b5_2_side_sell` | Sell 方向转字符串 | `Side::Sell` | `SELL` |
| 20 | `b6_1_order_type_market` | Market 订单类型转字符串 | `OrderType::Market` | `MARKET` |
| 21 | `b6_2_order_type_limit` | Limit 订单类型转字符串 | `OrderType::Limit` | `LIMIT` |
| 22 | `b6_3_order_type_stop_market` | StopMarket 订单类型转字符串 | `OrderType::StopMarket` | `STOP_MARKET` |
| 23 | `b6_4_order_type_stop` | Stop 订单类型转字符串 | `OrderType::Stop` | `STOP` |
| 24 | `b6_5_order_type_take_profit_market` | TakeProfitMarket 订单类型转字符串 | `OrderType::TakeProfitMarket` | `TAKE_PROFIT_MARKET` |
| 25 | `b6_6_futures_stop_market_unchanged` | 合约 StopMarket 不变 | `OrderType::StopMarket` | `STOP_MARKET` |
| 26 | `b6_7_futures_take_profit_market_unchanged` | 合约 TakeProfitMarket 不变 | `OrderType::TakeProfitMarket` | `TAKE_PROFIT_MARKET` |
| 27 | `b6_8_order_type_take_profit` | TakeProfit 订单类型转字符串 | `OrderType::TakeProfit` | `TAKE_PROFIT` |
| 28 | `b6_9_order_type_trailing_stop_market` | TrailingStopMarket 订单类型转字符串 | `OrderType::TrailingStopMarket` | `TRAILING_STOP_MARKET` |
| 29 | `b6_10_order_type_liquidation` | Liquidation 订单类型转字符串 | `OrderType::Liquidation` | `LIQUIDATION` |
| 30 | `b7_1_try_build_ed25519_with_seed` | ed25519 种子 Base64 构造签名器 | api_key: `test_api_key`，seed_b64: 32 字节全零 Base64 | 返回 Ok |
| 31 | `b7_2_try_build_ed25519_pem` | ed25519 PEM 私钥构造签名器 | api_key: `test_key`，PEM 私钥 | 返回 Ok |
| 32 | `b7_3_try_build_ed25519_wrong_byte_count` | ed25519 错误字节长度 | 错误长度的 Base64 | 返回 Err |
| 33 | `b7_4_try_build_ed25519_not_base64` | ed25519 非 Base64/PEM 输入 | `this_is_not_base64_or_pem!` | 返回 Err |
| 34 | `t1_1_time_sync_interval_is_one_hour` | 时间同步间隔为 1 小时 | 常量检查 | `TIME_SYNC_INTERVAL_SECS=3600` |
| 35 | `t1_2_time_offset_warn_threshold_is_2000ms` | 时间偏移警告阈值为 2000ms | 常量检查 | `TIME_OFFSET_WARN_THRESHOLD_MS=2000` |
| 36 | `t1_3_time_sync_started_initialized_false` | 初始化后 time_sync_started 为 false | BinanceExchange::new(key, secret, ...) | `time_sync_started=false` |
| 37 | `t1_4_time_sync_started_swap_prevents_double_start` | CAS 防止重复启动时间同步 | 两次 swap(true) | 第一次返回 false，第二次返回 true |
| 38 | `t1_5_drop_cancels_time_sync` | Drop 取消时间同步任务 | 创建后立即 drop | 无 panic，正常释放 |
| 39 | `t1_6_no_tasks_on_init` | 初始化后无后台任务 | BinanceExchange::new(...) | time_sync_task=None，listenkey_task=None |

### `crates/core/virs-ccxt/src/adapter/binance/user_data_ws_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `test_parse_invalid_json` | 无效 JSON 解析失败 | `"not json"` | 返回 Err |
| 2 | `test_new_perpetual` | 创建永续合约 UserDataWs | listen_key: `test_listen_key`，api_key: `test_api_key` | 成功创建，无 panic |

### `crates/core/virs-ccxt/src/adapter/binance/orderbook_ws_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `w1_1_parse_levels_standard` | 标准格式解析深度层级 | `[["50000.0","1.5"],["49999.0","2.0"]]` | 返回 Some，含 2 个层级 |
| 2 | `w1_2_parse_levels_numeric_elements` | 数值类型元素解析层级 | `[[50000.0, 1.5]]` | 返回 Some，1 个层级，可解析为 50000.0 |
| 3 | `w1_3_parse_levels_empty_array` | 空数组解析层级 | `[]` | `Some(vec![])` |
| 4 | `w1_4_parse_levels_not_array` | 非数组解析返回 None | `{"key": "value"}` | `None` |
| 5 | `w1_5_parse_levels_short_element` | 元素长度不足返回 None | `[["50000.0"]]` | `None` |
| 6 | `w2_1_to_levels_normal` | 正常层级转换为 OrderBookLevel | `[["50000.0","1.5"],["49999.0","2.0"]]` | 2 个层级，price=50000.0/49999.0 |
| 7 | `w2_2_to_levels_filter_zero_amount` | 过滤零数量层级 | `[["50000.0","0.0"],["49999.0","2.0"]]` | 1 个层级，price=49999.0 |
| 8 | `w2_3_to_levels_filter_negative_amount` | 过滤负数量层级 | `[["50000.0","-1.0"],["49999.0","2.0"]]` | 1 个层级，price=49999.0 |
| 9 | `w2_4_to_levels_filter_invalid_number` | 过滤无效数字层级 | `[["abc","1.0"],["49999.0","2.0"]]` | 1 个层级，price=49999.0 |
| 10 | `w2_5_to_levels_empty` | 空数组转换 | `[]` | 空 vec |
| 11 | `w3_2_parse_payload_perpetual_format` | 永续合约深度格式解析 | 永续格式 depthUpdate JSON，bids=1, asks=1 | Some，symbol=Some("BTCUSDT")，timestamp_ms=1234567890，last_update_id=None |
| 12 | `w3_3_parse_payload_no_matching_format` | 无匹配格式返回 None | `{"foo": "bar"}` | `None` |
| 13 | `w4_2_into_depth_combined_stream_perpetual` | 永续合约组合流转深度 | 含 stream 和 data 的组合流 JSON | Some，stream_name=Some("btcusdt@depth20@500ms")，symbol=Some("BTCUSDT")，bids=1, asks=1 |
| 14 | `w4_4_into_depth_single_stream_perpetual` | 永续合约单流转深度 | 单流 depthUpdate JSON | Some，symbol=Some("BTCUSDT")，bids=1, asks=1，last_update_id=None |
| 15 | `w4_5_into_depth_invalid_message` | 无效消息转深度返回 None | `{"foo": "bar"}` | `None` |

### `crates/core/virs-ccxt/src/adapter/binance/kline_ws_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `test_parse_binance_kline_message` | 解析币安 K 线消息（组合流+扁平格式） | 组合流 K 线 JSON，open=65000.00，close=65050.00，volume=100.5，trades=500 | stream="btcusdt@kline_1m"，OHLCV 正确，closed=false；另测 closed=true 及扁平格式 |
| 2 | `test_parse_binance_kline_message_without_stream` | 无 stream 字段的 K 线消息解析 | 仅含 data 的 JSON | stream=None，data=Some |
| 3 | `test_parse_invalid_json` | 无效 JSON 解析失败 | `"not json"` | 返回 Err |
| 4 | `test_parse_non_kline_event` | 非 K 线事件解析失败 | trade 事件 JSON | 返回 Err |
| 5 | `test_to_candle_basic` | K 线数据转蜡烛图基本转换 | OHLCV: 65000/65100/64900/65050，volume=100.5 | 各字段正确，closed=false；另测 closed=true |
| 6 | `test_to_candle_invalid_numbers` | 无效 OHLCV 数字转蜡烛图失败 | open=`not_a_number`，high=`abc` | 返回 Err(NoData) |
| 7 | `test_ws_symbol` | 获取 WS 符号 | symbol: `BTCUSDT` | `BTCUSDT` |
| 8 | `test_binance_ws_symbol_basic` | WS 符号基础转换 | `BTCUSDT`、`btcusdt` | 均返回 `btcusdt` |
| 9 | `test_subscribe_without_start` | 未启动时订阅 | symbol: `BTCUSDT` | 订阅列表含 `btcusdt@kline_1m`，符号映射正确，未运行 |
| 10 | `t8_1_event_time_parsed_and_accessible` | 组合流事件时间解析 | E: 1713900000123 | data.event_time=1713900000123 |
| 11 | `t8_2_delay_threshold_is_5000ms` | 延迟阈值为 5000ms | 常量检查 | `KLINE_WS_DELAY_THRESHOLD_MS=5000`，6000ms 延迟超阈值，3000ms 未超 |
| 12 | `t8_3_single_stream_event_time_parsed` | 单流事件时间解析 | E: 1713900000456 | data.event_time=1713900000456 |
| 13 | `t8_4_single_stream_event_time_missing_returns_none` | 单流缺少 E 字段返回 None | 无 E 字段的 K 线 JSON | into_kline_data() 返回 None |

## 集成测试

### `crates/core/virs-ccxt/tests/integration_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `int_2_1_hmac_signature_deterministic` | HMAC 签名确定性验证 | key: `test_secret_key`，msg: `symbol=BTCUSDT&timestamp=1234567890` | 两次签名相同，长度=64 |
| 2 | `int_4_2_order_trade_update_dispatch` | 订单交易更新事件分发 | ORDER_TRADE_UPDATE 事件 JSON，symbol=BTCUSDT，side=BUY | dispatch_event 返回 Some |
| 3 | `int_4_3_non_order_event_returns_none` | 非订单事件分发返回 None | listenKeyExpired 事件 JSON | dispatch_event 返回 None |
| 4 | `int_5_1_create_exchange_binance_hmac` | 创建 Binance HMAC 交易所 | name=`binance`，HMAC 密钥 | 返回 Ok，name=`binance` |
| 5 | `int_5_2_create_exchange_binance_ed25519` | 创建 Binance ed25519 交易所 | name=`binance`，ed25519 种子 | 返回 Ok，name=`binance` |
| 6 | `int_5_3_create_exchange_bybit_not_supported` | 创建 Bybit 交易所不支持 | name=`bybit` | 返回 Err(NotSupported) |
| 7 | `int_5_4_create_exchange_okx_not_supported` | 创建 OKX 交易所不支持 | name=`okx` | 返回 Err(NotSupported) |
| 8 | `int_5_5_create_exchange_case_insensitive` | 交易所名称大小写不敏感 | name=`BINANCE` | 返回 Ok |
| 9 | `int_6_2_order_status_chain` | 订单状态完整转换链 | `PARTIALLY_FILLED` | CcxtOrderStatus::PartiallyFilled -> OrderStatus::PartiallyFilled |
| 10 | `int_6_3_order_status_expired_chain` | 过期状态完整转换链 | `EXPIRED` | CcxtOrderStatus::Expired -> OrderStatus::Expired |
| 11 | `int_7_1_order_type_roundtrip` | 订单类型往返转换一致性 | 7 种订单类型 | 每种 str -> parse 往返一致 |
| 12 | `int_7_2_side_roundtrip` | 方向往返转换 | Side::Buy, Side::Sell | `BUY`, `SELL` |
| 13 | `int_8_1_parse_f64_used_in_ticker_conversion` | Ticker 转换中使用 parse_f64 | `{"price": "0.00012345"}` | `Some(0.00012345)` |
| 14 | `int_8_3_parse_str_used_in_symbol` | 符号解析中使用 parse_str | `{"symbol": "BTCUSDT"}` | `Some("BTCUSDT")` |
