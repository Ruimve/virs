# virs-ccxt 测试用例文档

> 生成日期: 2026-06-30
> Crate: `crates/libs/virs-ccxt`
> 状态: **158 个测试全部通过** (140 单元 + 18 集成)

---

## 单元测试用例

### lib_tests.rs — lib.rs 工具函数 (32)

| ID | 测试函数 | 描述 |
|----|---------|------|
| L1.1 | `l1_1_parse_f64_from_number` | 数字类型字段 → f64 |
| L1.2 | `l1_2_parse_f64_from_string` | 字符串字段 → f64 |
| L1.3 | `l1_3_parse_f64_missing_field` | 字段不存在 → None |
| L1.4 | `l1_4_parse_f64_null_field` | null 字段 → None |
| L1.5 | `l1_5_parse_f64_invalid_string` | 无效字符串 → None |
| L1.6 | `l1_6_parse_f64_from_integer` | 整数字段 → f64 |
| L2.1 | `l2_1_parse_str_from_string` | 字符串字段 → String |
| L2.2 | `l2_2_parse_str_from_i64` | i64 字段 → String |
| L2.3 | `l2_3_parse_str_from_f64` | f64 字段 → String |
| L2.4 | `l2_4_parse_str_missing_field` | 字段不存在 → None |
| L4.1 | `l4_1_parse_u32_from_u64` | u64 字段 → u32 |
| L4.2 | `l4_2_parse_u32_from_string` | 字符串数字 → u32 |
| L4.3 | `l4_3_parse_u32_missing_field` | 字段不存在 → None |
| L5.1 | `l5_1_build_display_url_no_params` | 无参数 → 仅 path |
| L5.2 | `l5_2_build_display_url_with_params` | 普通参数 → path?key=value |
| L5.3 | `l5_3_build_display_url_masks_signature` | signature 参数被脱敏 |
| L5.4 | `l5_4_build_display_url_empty_params` | 空参数迭代器 → 仅 path |
| L6.1 | `l6_1_mask_signature_basic` | signature= 被脱敏 |
| L6.2 | `l6_2_mask_signature_with_trailing_params` | 仅脱敏 signature，保留后续参数 |
| L6.3 | `l6_3_mask_signature_no_signature` | 无 signature= → 原样返回 |
| L6.4 | `l6_4_mask_signature_multiple_signatures` | 多个 signature= → 仅脱敏第一个 |
| L7.1 | `l7_1_extract_error_msg_with_code` | msg + code → "[code] msg" |
| L7.2 | `l7_2_extract_error_msg_only` | 仅 msg → msg |
| L7.3 | `l7_3_extract_error_bybit_format` | retMsg + retCode → Bybit 格式 |
| L7.4 | `l7_4_extract_error_error_field` | error 字段 |
| L7.5 | `l7_5_extract_error_message_field` | message 字段 |
| L7.6 | `l7_6_extract_error_detail_field` | detail 字段 |
| L7.7 | `l7_7_extract_error_no_matching_field` | 无匹配 → JSON 字符串 |
| L8.1 | `l8_1_parse_timestamp_ms_from_i64` | i64 字段 → DateTime |
| L8.2 | `l8_2_parse_timestamp_ms_from_string` | 字符串字段 → DateTime |
| L8.3 | `l8_3_parse_timestamp_ms_missing_field` | 字段不存在 → None |
| L8.4 | `l8_4_parse_timestamp_ms_invalid_string` | 无效字符串 → None |

### auth_tests.rs — 签名函数 (9)

| ID | 测试函数 | 描述 |
|----|---------|------|
| A1.1 | `a1_1_hmac_sha256_hex_known_vector` | 已知输入 → 预期 hex 签名 |
| A1.2 | `a1_2_hmac_sha256_hex_empty_message` | 空消息 → 有效 hex |
| A1.3 | `a1_3_hmac_sha256_hex_empty_key` | 空密钥 → 有效 hex |
| A1.4 | `a1_4_hmac_sha256_hex_idempotent` | 相同输入 → 相同输出 |
| A1.5 | `a1_5_hmac_sha256_hex_different_inputs` | 不同输入 → 不同输出 |
| A3.1 | `a3_1_make_header_valid_ascii` | 合法 ASCII → Ok(HeaderValue) |
| A3.2 | `a3_2_make_header_invalid_chars` | 非法字符 → Err |
| A4.1 | `a4_1_insert_header_success` | 插入合法 header |
| A4.2 | `a4_2_insert_header_invalid_value` | 非法值 → Err，HeaderMap 不变 |

### errors_tests.rs — 错误类型 (1)

| ID | 测试函数 | 描述 |
|----|---------|------|
| E2.1 | `e2_1_no_data_construction` | no_data 构造 → 消息正确 |

### types_tests.rs — 类型转换 (14)

| ID | 测试函数 | 描述 |
|----|---------|------|
| T1.1 | `t1_1_new_to_open` | New → Open |
| T1.2 | `t1_2_partially_filled` | PartiallyFilled → PartiallyFilled |
| T1.3 | `t1_3_filled` | Filled → Filled |
| T1.4 | `t1_4_canceled` | Canceled → Canceled |
| T1.5 | `t1_5_expired_maps_to_canceled` | Expired → Canceled |
| T1.6 | `t1_6_expired_in_match_maps_to_canceled` | ExpiredInMatch → Canceled |
| T2.1 | `t2_1_ticker_all_fields` | 所有字段有值 → 正确转换 |
| T2.2 | `t2_2_ticker_none_fields_return_error` | None 字段 → Err |
| T2.3 | `t2_3_ticker_timestamp_none_returns_error` | timestamp None → Err |
| T3.1 | `t3_1_order_book_normal` | 正常转换 → bids/asks 保留 |
| T3.2 | `t3_2_order_book_timestamp_none` | timestamp None → Err |
| T4.1 | `t4_1_funding_rate_normal` | 正常转换 |
| T7.1 | `t7_1_funding_time_zero_is_epoch` | timestamp 0 → 有效 epoch |
| T7.2 | `t7_2_filter_zero_before_from_timestamp_millis` | 0 → 过滤为 None |

### adapter/binance/mod_tests.rs — Binance 映射函数 (54)

| ID | 测试函数 | 描述 |
|----|---------|------|
| B1.1 | `b1_1_native_symbol_with_slash` | BTC/USDT → BTCUSDT |
| B1.2 | `b1_2_native_symbol_with_dash` | BTC-USDT → BTCUSDT |
| B1.3 | `b1_3_native_symbol_already_native` | BTCUSDT → BTCUSDT |
| B1.4 | `b1_4_native_symbol_eth_usdc` | ETH/USDC → ETHUSDC |
| B1.5 | `b1_5_native_symbol_empty` | 空字符串 → 空字符串 |
| B2.1 | `b2_1_unified_symbol_usdt` | BTCUSDT → BTC/USDT |
| B2.2 | `b2_2_unified_symbol_usdc` | ETHUSDC → ETH/USDC |
| B2.3 | `b2_3_unified_symbol_btc_pair` | BNBBTC → BNB/BTC |
| B2.4 | `b2_4_unified_symbol_busd` | BTCBUSD → BTC/BUSD |
| B2.5 | `b2_5_unified_symbol_unknown_quote` | 未知报价货币 → 原样返回 |
| B2.6 | `b2_6_unified_symbol_only_quote` | 仅报价货币 → 原样返回 |
| B3.1 | `b3_1_status_new` | NEW → New |
| B3.2 | `b3_2_status_partially_filled` | PARTIALLY_FILLED → PartiallyFilled |
| B3.3 | `b3_3_status_filled` | FILLED → Filled |
| B3.4 | `b3_4_status_canceled` | CANCELED → Canceled |
| B3.5 | `b3_5_status_cancelled_variant` | CANCELLED → Canceled |
| B3.6 | `b3_6_status_expired` | EXPIRED → Expired |
| B3.7 | `b3_7_status_expired_in_match` | EXPIRED_IN_MATCH → ExpiredInMatch |
| B3.8 | `b3_8_status_unknown_returns_unknown` | 未知状态 → Unknown |
| B4.1 | `b4_1_type_market` | MARKET → Market |
| B4.2 | `b4_2_type_limit` | LIMIT → Limit |
| B4.3 | `b4_3_type_stop_market` | STOP_MARKET → StopMarket |
| B4.4 | `b4_4_type_stop` | STOP → Stop |
| B4.5 | `b4_5_type_trailing_stop_market` | TRAILING_STOP_MARKET → TrailingStopMarket |
| B4.6 | `b4_6_type_liquidation` | LIQUIDATION → Liquidation |
| B4.7 | `b4_7_type_take_profit_market` | TAKE_PROFIT_MARKET → TakeProfitMarket |
| B4.7b | `b4_7b_type_take_profit` | TAKE_PROFIT → TakeProfit |
| B4.8 | `b4_8_type_unknown_returns_unknown` | 未知类型 → Unknown |
| B5.1 | `b5_1_side_buy` | Side::Buy → "BUY" |
| B5.2 | `b5_2_side_sell` | Side::Sell → "SELL" |
| B6.1 | `b6_1_order_type_market` | Market → "MARKET" |
| B6.2 | `b6_2_order_type_limit` | Limit → "LIMIT" |
| B6.3 | `b6_3_order_type_stop_market` | StopMarket → "STOP_MARKET" |
| B6.4 | `b6_4_order_type_stop` | Stop → "STOP" |
| B6.5 | `b6_5_order_type_take_profit_market` | TakeProfitMarket → "TAKE_PROFIT_MARKET" |
| B6.6 | `b6_6_futures_stop_market_unchanged` | StopMarket → "STOP_MARKET"（不变） |
| B6.7 | `b6_7_futures_take_profit_market_unchanged` | TakeProfitMarket → "TAKE_PROFIT_MARKET"（不变） |
| B6.8 | `b6_8_order_type_take_profit` | TakeProfit → "TAKE_PROFIT" |
| B6.9 | `b6_9_order_type_trailing_stop_market` | TrailingStopMarket → "TRAILING_STOP_MARKET" |
| B6.10 | `b6_10_order_type_liquidation` | Liquidation → "LIQUIDATION" |
| B7.1 | `b7_1_try_build_ed25519_with_seed` | 32 字节 base64 seed → Ok |
| B7.2 | `b7_2_try_build_ed25519_with_pem` | PEM 格式密钥 → Ok |
| B7.3 | `b7_3_try_build_ed25519_wrong_byte_count` | 非 32 字节 base64 → Err |
| B7.4 | `b7_4_try_build_ed25519_not_base64` | 非 base64/PEM → Err |
| F1.1 | `f1_1_parse_order_book_side_bids` | bids 数组解析 |
| F1.2 | `f1_2_parse_order_book_side_asks` | asks 数组解析 |
| F1.3 | `f1_3_parse_order_book_side_missing` | 字段缺失 → 空 |
| F1.4 | `f1_4_parse_order_book_side_empty` | 空数组 → 空 |
| T1.1 | `t1_1_time_sync_interval_is_one_hour` | TIME_SYNC_INTERVAL_SECS = 3600 |
| T1.2 | `t1_2_time_offset_warn_threshold_is_2000ms` | TIME_OFFSET_WARN_THRESHOLD_MS = 2000 |
| T1.3 | `t1_3_time_sync_started_initialized_false` | time_sync_started 初始为 false |
| T1.4 | `t1_4_time_sync_started_swap_prevents_double_start` | swap 防止重复启动 |
| T1.5 | `t1_5_drop_sets_time_sync_running_false` | drop 后 time_sync_running 重置 |
| T1.6 | `t1_6_time_sync_running_initialized_false` | time_sync_running 初始为 false |

### adapter/binance/orderbook_ws_tests.rs — OrderBook WS 解析 (15)

| ID | 测试函数 | 描述 |
|----|---------|------|
| W1.1 | `w1_1_parse_levels_standard` | 标准数组 → 字符串二维数组 |
| W1.2 | `w1_2_parse_levels_numeric_elements` | 数字元素 → 解析为字符串 |
| W1.3 | `w1_3_parse_levels_empty_array` | 空数组 → Some(vec![]) |
| W1.4 | `w1_4_parse_levels_not_array` | 非数组 → None |
| W1.5 | `w1_5_parse_levels_short_element` | 元素不足 → None |
| W2.1 | `w2_1_to_levels_normal` | 正常 → OrderBookLevel |
| W2.2 | `w2_2_to_levels_filter_zero_amount` | amount=0 → 过滤 |
| W2.3 | `w2_3_to_levels_filter_negative_amount` | amount<0 → 过滤 |
| W2.4 | `w2_4_to_levels_filter_invalid_number` | 无效数字 → 过滤 |
| W2.5 | `w2_5_to_levels_empty` | 空输入 → 空 |
| W3.2 | `w3_2_parse_payload_perpetual_format` | Perpetual 格式 → 解析成功 |
| W3.3 | `w3_3_parse_payload_no_matching_format` | 无匹配格式 → None |
| W4.2 | `w4_2_into_depth_combined_stream_perpetual` | 组合流 Perpetual → 解析成功 |
| W4.4 | `w4_4_into_depth_single_stream_perpetual` | 单流 Perpetual → 解析成功 |
| W4.5 | `w4_5_into_depth_invalid_message` | 无效消息 → None |

### adapter/binance/kline_ws_tests.rs — Kline WS 解析 (13)

| ID | 测试函数 | 描述 |
|----|---------|------|
| K1.1 | `test_parse_binance_kline_message` | 解析组合流 kline 消息 |
| K1.2 | `test_parse_binance_kline_message_without_stream` | 解析无 stream 字段 kline 消息 |
| K1.3 | `test_parse_invalid_json` | 无效 JSON → Err |
| K1.4 | `test_parse_non_kline_event` | 非 kline 事件 → Err |
| K2.1 | `test_to_candle_basic` | kline → candle 基础转换 |
| K2.2 | `test_to_candle_invalid_numbers` | 无效 OHLCV → Err(NoData) |
| K2.3 | `test_ws_symbol` | kline data → ws_symbol |
| K2.4 | `test_binance_ws_symbol_basic` | WS symbol 基础格式转换 |
| K3.1 | `test_subscribe_without_start` | 未启动时订阅 → 订阅集合正确 |
| T8.1 | `t8_1_event_time_parsed_and_accessible` | 组合流 E 字段解析 |
| T8.2 | `t8_2_delay_threshold_is_5000ms` | 延迟阈值 5000ms |
| T8.3 | `t8_3_single_stream_event_time_parsed` | 单流 E 字段解析 |
| T8.4 | `t8_4_single_stream_event_time_missing_returns_none` | 单流缺 E → None |

### adapter/binance/user_data_ws_tests.rs — User Data WS (2)

| ID | 测试函数 | 描述 |
|----|---------|------|
| U1.1 | `test_parse_invalid_json` | 无效 JSON → Err |
| U1.2 | `test_new_perpetual` | 合约 WS URL 构建 |

---

## 集成测试用例

### integration_tests.rs (18)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-1.1 | `int_1_1_symbol_roundtrip_usdt` | BTC/USDT → BTCUSDT → BTC/USDT |
| INT-1.2 | `int_1_2_symbol_roundtrip_usdc` | ETH-USDC → ETHUSDC → ETH/USDC |
| INT-1.3 | `int_1_3_symbol_roundtrip_btc_pair` | BNB/BTC → BNBBTC → BNB/BTC |
| INT-2.1 | `int_2_1_hmac_signature_deterministic` | HMAC 签名幂等性 |
| INT-4.2 | `int_4_2_order_trade_update_dispatch` | ORDER_TRADE_UPDATE → dispatch_event → WsFeedEvent |
| INT-4.3 | `int_4_3_non_order_event_returns_none` | 非订单事件 → dispatch_event → None |
| INT-5.1 | `int_5_1_create_exchange_binance_hmac` | Binance + HMAC 创建成功 |
| INT-5.2 | `int_5_2_create_exchange_binance_ed25519` | Binance + Ed25519 创建成功 |
| INT-5.3 | `int_5_3_create_exchange_bybit_not_supported` | Bybit → NotSupported |
| INT-5.4 | `int_5_4_create_exchange_okx_not_supported` | OKX → NotSupported |
| INT-5.5 | `int_5_5_create_exchange_case_insensitive` | 大写交易所名兼容 |
| INT-6.1 | `int_6_1_ticker_json_to_ticker_via_parse` | ticker JSON → parse_f64 → CcxtTicker → Ticker |
| INT-6.2 | `int_6_2_order_status_chain` | Binance 状态字符串 → CcxtOrderStatus → OrderStatus |
| INT-6.3 | `int_6_3_order_status_expired_to_canceled_chain` | EXPIRED → Canceled → Canceled |
| INT-7.1 | `int_7_1_order_type_roundtrip` | OrderType 往返转换 |
| INT-7.2 | `int_7_2_side_roundtrip` | Side 往返转换 |
| INT-8.1 | `int_8_1_parse_f64_used_in_ticker_conversion` | parse_f64 在 ticker 转换中使用 |
| INT-8.3 | `int_8_3_parse_str_used_in_symbol` | parse_str 在 symbol 提取中使用 |

---

## 代码覆盖率

### 测试文件与模块映射

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/lib_tests.rs` | lib.rs | 32 |
| `src/auth_tests.rs` | auth.rs | 9 |
| `src/errors_tests.rs` | errors.rs | 1 |
| `src/types_tests.rs` | types.rs | 14 |
| `src/adapter/binance/mod_tests.rs` | adapter/binance/mod.rs | 54 |
| `src/adapter/binance/orderbook_ws_tests.rs` | orderbook_ws.rs | 15 |
| `src/adapter/binance/kline_ws_tests.rs` | kline_ws.rs | 13 |
| `src/adapter/binance/user_data_ws_tests.rs` | user_data_ws.rs | 2 |
| `tests/integration_tests.rs` | 跨模块 | 18 |
| **合计** | | **158** |

### 死代码清理记录

| 已删除函数/方法 | 位置 | 删除原因 |
|----------------|------|---------|
| `parse_i64` | lib.rs | 零生产调用，零外部引用，仅被测试引用 |
| `hmac_sha256_base64` | auth.rs | 零生产调用，零外部引用，仅被测试引用 |
| `ExchangeError::exchange()` | errors.rs | 零生产调用，零外部引用，仅被测试引用 |
| `ExchangeError::is_retryable()` | errors.rs | 零生产调用，零外部引用，仅被测试引用 |

> 清理后测试数从 190 降至 158（删除 27 个仅测试死代码的用例，并移除已删除的 spot Ed25519 WS API 5 个用例），全部通过。
