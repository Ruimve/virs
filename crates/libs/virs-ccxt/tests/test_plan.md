# virs-ccxt 测试用例文档

> 生成日期: 2026-06-30
> Crate: `crates/libs/virs-ccxt`
> 状态: **163 个测试全部通过** (144 单元 + 19 集成)

---

## 单元测试用例

### lib_tests.rs — lib.rs 工具函数 (28)

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
| T1.1 | `t1_1_open_to_open` | Open → Open |
| T1.2 | `t1_2_partially_filled` | PartiallyFilled → PartiallyFilled |
| T1.3 | `t1_3_filled` | Filled → Filled |
| T1.4 | `t1_4_canceled` | Canceled → Canceled |
| T1.5 | `t1_5_expired_maps_to_canceled` | Expired → Canceled |
| T1.6 | `t1_6_failed` | Failed → Failed |
| T1.7 | `t1_7_rejected_maps_to_failed` | Rejected → Failed |
| T2.1 | `t2_1_ticker_all_fields` | 所有字段有值 → 正确转换 |
| T2.2 | `t2_2_ticker_none_fields_default_to_zero` | None 字段 → 0.0 |
| T2.3 | `t2_3_ticker_timestamp_none_uses_now` | timestamp None → 当前时间 |
| T3.1 | `t3_1_order_book_normal` | 正常转换 → bids/asks 保留 |
| T3.2 | `t3_2_order_book_timestamp_none` | timestamp None → 当前时间 |
| T4.1 | `t4_1_funding_rate_normal` | 正常转换 |
| T5.1 | `t5_1_funding_history_normal` | 正常转换 |

### adapter/binance/mod_tests.rs — Binance 映射函数 (43)

| ID | 测试函数 | 描述 |
|----|---------|------|
| B1.1 | `b1_1_native_symbol_with_slash` | BTC/USDT → BTCUSDT |
| B1.2 | `b1_2_native_symbol_with_dash` | BTC-USDT → BTCUSDT |
| B1.3 | `b1_3_native_symbol_already_native` | BTCUSDT → BTCUSDT |
| B1.4 | `b1_4_native_symbol_eth_usdc` | ETH/USDC → ETHUSDC |
| B1.5 | `b1_5_native_symbol_empty` | 空字符串 → 空字符串 |
| B2.1 | `b2_1_unified_symbol_usdt` | BTCUSDT → BTC/USDT |
| B2.2 | `b2_2_unified_symbol_usdc` | ETHUSDC → ETH/USDC |
| B2.3 | `b2_3_unified_symbol_btc` | BNBBTC → BNB/BTC |
| B2.4 | `b2_4_unified_symbol_busd` | BTCBUSD → BTC/BUSD |
| B2.5 | `b2_5_unified_symbol_unknown_quote` | 未知报价货币 → 原样返回 |
| B2.6 | `b2_6_unified_symbol_only_quote` | 仅报价货币 → 原样返回 |
| B3.1–B3.9 | `b3_*` | parse_order_status: NEW/PARTIALLY_FILLED/FILLED/CANCELED/CANCELLED/EXPIRED/REJECTED/PENDING_CANCEL/未知 |
| B4.1–B4.8 | `b4_*` | parse_order_type: MARKET/LIMIT/STOP_MARKET/STOP_LOSS/STOP_LOSS_LIMIT/TAKE_PROFIT_LIMIT/TAKE_PROFIT_MARKET/未知 |
| B5.1 | `b5_1_side_buy` | Side::Buy → "BUY" |
| B5.2 | `b5_2_side_sell` | Side::Sell → "SELL" |
| B6.1–B6.5 | `b6_*` | order_type_str: Market/Limit/StopMarket/StopLimit/TakeProfitMarket |
| B7.1 | `b7_1_ed25519_pem` | PEM 格式密钥 → Ok |
| B7.2 | `b7_2_ed25519_seed` | 32 字节 base64 seed → Ok |
| B7.3 | `b7_3_ed25519_wrong_length` | 非 32 字节 base64 → Err |
| B7.4 | `b7_4_ed25519_not_base64` | 非 base64 → HMAC fallback |
| F1.* | `f1_*` | parse_order_book_side 解析测试 |

### adapter/binance/orderbook_ws_tests.rs — OrderBook WS 解析 (18)

| ID | 测试函数 | 描述 |
|----|---------|------|
| W1.1–W1.5 | `w1_*` | parse_levels: 标准数组/数字/空数组/非数组/元素不足 |
| W2.1–W2.5 | `w2_*` | to_levels: 正常/amount=0/amount<0/无效数字/空输入 |
| W3.1–W3.3 | `w3_*` | parse_payload: Perpetual/缺少字段 |
| W4.1–W4.5 | `w4_*` | into_depth: 组合流perp/单流perp/无效消息 |

### adapter/binance/ws_api_tests.rs — (deleted, spot Ed25519 WS API removed)

### adapter/binance/kline_ws.rs — 内联测试

| 测试函数 | 描述 |
|---------|------|
| `test_binance_ws_symbol_basic` | WS symbol 基础格式 |
| `test_ws_symbol` | WS symbol 转换 |
| `test_new_perpetual` | 合约 WS URL 构建 |
| `test_parse_binance_kline_message` | 解析组合流 kline 消息 |
| `test_parse_binance_kline_message_without_stream` | 解析单流 kline 消息 |
| `test_parse_non_kline_event` | 非 kline 事件解析 |
| `test_parse_invalid_json` | 无效 JSON 处理 |
| `test_to_candle_basic` | kline → candle 基础转换 |
| `test_to_candle_invalid_numbers` | 无效数字处理 |

### adapter/binance/user_data_ws.rs — 内联测试 (2)

| 测试函数 | 描述 |
|---------|------|
| `test_parse_invalid_json` | 无效 JSON 处理 |
| `test_new_perpetual` | 合约 WS URL |

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
| `src/lib_tests.rs` | lib.rs | 28 |
| `src/auth_tests.rs` | auth.rs | 9 |
| `src/errors_tests.rs` | errors.rs | 1 |
| `src/types_tests.rs` | types.rs | 14 |
| `src/adapter/binance/mod_tests.rs` | adapter/binance/mod.rs | 43 |
| `src/adapter/binance/orderbook_ws_tests.rs` | orderbook_ws.rs | 18 |
| `src/adapter/binance/ws_api_tests.rs` | ws_api.rs | 5 |
| `src/adapter/binance/kline_ws.rs` (内联) | kline_ws.rs | 11 |
| `src/adapter/binance/order_ws.rs` (内联) | order_ws.rs | 15 |
| `tests/integration_tests.rs` | 跨模块 | 19 |
| **合计** | | **163** |

### 死代码清理记录

| 已删除函数/方法 | 位置 | 删除原因 |
|----------------|------|---------|
| `parse_i64` | lib.rs | 零生产调用，零外部引用，仅被测试引用 |
| `hmac_sha256_base64` | auth.rs | 零生产调用，零外部引用，仅被测试引用 |
| `ExchangeError::exchange()` | errors.rs | 零生产调用，零外部引用，仅被测试引用 |
| `ExchangeError::is_retryable()` | errors.rs | 零生产调用，零外部引用，仅被测试引用 |

> 清理后测试数从 190 降至 163（删除 27 个仅测试死代码的用例），全部通过。
