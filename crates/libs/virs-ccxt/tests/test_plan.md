# virs-ccxt 测试用例文档

> 生成日期: 2026-06-29
> Crate: `crates/libs/virs-ccxt`
> 目标: 对 crate 内所有可提取的幂等/纯函数逻辑进行完整覆盖，并为关键业务路径编写集成测试。
> 状态: **全部 190 个测试通过** (165 单元 + 25 集成)

---

## 1. Crate 结构概览

```
virs-ccxt/
├── src/
│   ├── lib.rs                      # Exchange trait, ExchangeClient, parse_* 辅助函数, URL/签名脱敏
│   ├── auth.rs                     # Signer trait, HMAC-SHA256 签名辅助函数
│   ├── errors.rs                   # ExchangeError 枚举, is_retryable()
│   ├── types.rs                    # CCXT 内部类型定义 + From 转换实现
│   ├── ws_types.rs                 # WebSocket 事件类型定义
│   └── adapter/
│       ├── mod.rs                  # adapter 模块入口
│       └── binance/
│           ├── mod.rs              # BinanceExchange, BinanceSigner, BinanceEd25519Signer, 符号/状态/类型映射
│           ├── api.rs              # /api/v3 Spot REST (ticker, klines, orders...)
│           ├── fapi.rs             # /fapi/v1 永续 REST (ticker, klines, orders, positions, funding...)
│           ├── sapi.rs             # /sapi/v1 账户 API (apiRestrictions)
│           ├── kline_ws.rs         # Kline WebSocket 客户端 (已有内联测试)
│           ├── order_ws.rs         # 订单 User Data Stream WS 客户端 (已有内联测试)
│           ├── orderbook_ws.rs     # OrderBook WebSocket 客户端
│           └── ws_api.rs           # WebSocket API 客户端 (Ed25519 认证)
├── tests/
│   ├── test_plan.md                # 本文档
│   └── integration_tests.rs        # 集成测试
```

---

## 2. 幂等函数清单

以下函数为**纯函数或幂等函数**（相同输入永远产生相同输出，无副作用），是单元测试的核心目标。

### 2.1 lib.rs — 解析与工具函数

| # | 函数 | 签名 | 幂等 | 说明 |
|---|------|------|------|------|
| L1 | `parse_f64` | `(v: &Value, field: &str) -> Option<f64>` | 是 | 从 JSON Value 解析 f64，支持数字和字符串 |
| L2 | `parse_str` | `(v: &Value, field: &str) -> Option<String>` | 是 | 从 JSON Value 解析 String，支持 str/i64/f64 |
| L3 | `parse_i64` | `(v: &Value, field: &str) -> Option<i64>` | 是 | 从 JSON Value 解析 i64，支持数字和字符串 |
| L4 | `parse_u32` | `(v: &Value, field: &str) -> Option<u32>` | 是 | 从 JSON Value 解析 u32，支持数字和字符串 |
| L5 | `build_display_url` | `(path: &str, params: Iterator) -> String` | 是 | 构建 URL 字符串，脱敏 signature 参数 |
| L6 | `mask_signature` | `(s: &str) -> String` | 是 | 脱敏 URL 编码 body 中的 signature= 字段 |
| L7 | `extract_error_message` | `(json: &Value) -> String` | 是 | 从交易所错误 JSON 提取消息，支持 msg/retMsg/error/message/detail |

### 2.2 auth.rs — 签名辅助函数

| # | 函数 | 签名 | 幂等 | 说明 |
|---|------|------|------|------|
| A1 | `hmac_sha256_hex` | `(secret: &str, message: &str) -> String` | 是 | HMAC-SHA256 十六进制签名 |
| A2 | `hmac_sha256_base64` | `(secret: &str, message: &str) -> String` | 是 | HMAC-SHA256 Base64 签名 |
| A3 | `make_header` | `(name: &'static str, value: &str) -> Result<HeaderValue>` | 是 | 创建 HTTP HeaderValue |
| A4 | `insert_header` | `(headers: &mut HeaderMap, name, value) -> Result<()>` | 是 | 向 HeaderMap 插入 header |

### 2.3 errors.rs — 错误类型

| # | 函数 | 签名 | 幂等 | 说明 |
|---|------|------|------|------|
| E1 | `ExchangeError::exchange` | `(code, message) -> Self` | 是 | 构造 ExchangeError 变体 |
| E2 | `ExchangeError::no_data` | `(context: String) -> Self` | 是 | 构造 NoData 错误 |
| E3 | `ExchangeError::is_retryable` | `(&self) -> bool` | 是 | 判断错误是否可重试 |

### 2.4 types.rs — 类型转换 (From impls)

| # | 实现 | 幂等 | 说明 |
|---|------|------|------|
| T1 | `From<CcxtOrderStatus> for OrderStatus` | 是 | CCXT 订单状态 → 应用层订单状态 |
| T2 | `From<CcxtTicker> for Ticker` | 是 | CCXT Ticker → 应用层 Ticker (None → 0.0) |
| T3 | `From<CcxtOrderBook> for OrderBook` | 是 | CCXT OrderBook → 应用层 OrderBook |
| T4 | `From<CcxtFundingRate> for FundingRate` | 是 | CCXT 资金费率 → 应用层资金费率 |
| T5 | `From<CcxtFundingHistoryEntry> for FundingHistoryEntry` | 是 | CCXT 资金历史 → 应用层资金历史 |

### 2.5 adapter/binance/mod.rs — Binance 映射函数

| # | 函数 | 签名 | 幂等 | 说明 |
|---|------|------|------|------|
| B1 | `to_native_symbol` | `(&str) -> String` | 是 | 统一符号 → Binance 原生符号 (BTC/USDT → BTCUSDT) |
| B2 | `to_unified_symbol` | `(&str) -> String` | 是 | Binance 原生符号 → 统一符号 (BTCUSDT → BTC/USDT) |
| B3 | `parse_order_status` | `(&str) -> CcxtOrderStatus` | 是 | Binance 订单状态字符串 → CcxtOrderStatus |
| B4 | `parse_order_type` | `(&str) -> OrderType` | 是 | Binance 订单类型字符串 → OrderType |
| B5 | `side_str` | `(&Side) -> &'static str` | 是 | Side 枚举 → Binance 字符串 (BUY/SELL) |
| B6 | `order_type_str` | `(&OrderType) -> &'static str` | 是 | OrderType 枚举 → Binance 字符串 |
| B7 | `try_build_ed25519` | `(api_key, api_secret) -> Result<BinanceEd25519Signer>` | 是 | 根据 secret 格式判断签名器类型 |

### 2.6 adapter/binance/api.rs & fapi.rs — 共享解析函数

| # | 函数 | 签名 | 幂等 | 说明 |
|---|------|------|------|------|
| F1 | `parse_order_book_side` | `(&Value, side: &str) -> Vec<(f64, f64)>` | 是 | 解析订单簿 bids/asks 数组 |

> **重构说明**: `api.rs` 和 `fapi.rs` 各有一份完全相同的 `parse_order_book_side`，应提取到共享位置。

### 2.7 adapter/binance/orderbook_ws.rs — OrderBook WS 解析

| # | 函数 | 签名 | 幂等 | 说明 |
|---|------|------|------|------|
| W1 | `parse_levels` | `(&Value) -> Option<Vec<[String; 2]>>` | 是 | 解析价格层级数组为 [price, amount] 对 |
| W2 | `to_levels` | `(&[[String; 2]]) -> Vec<OrderBookLevel>` | 是 | 字符串对 → OrderBookLevel (过滤 amount<=0) |
| W3 | `parse_payload` | `(&Value) -> Option<(bids, asks, sym, ts)>` | 是 | 解析 depth payload (spot bids/asks 或 perpetual b/a) |
| W4 | `BinanceDepthMessage::into_depth` | `(self) -> Option<(bids, asks, stream, sym, ts)>` | 是 | 提取深度数据，兼容单流/组合流 + spot/perp |

### 2.8 adapter/binance/ws_api.rs — WebSocket API

| # | 函数 | 签名 | 幂等 | 说明 |
|---|------|------|------|------|
| WA1 | `build_session_logon_request` | `(&BinanceEd25519Signer, id) -> Result<Value, String>` | 半幂等 | 构造 session.logon 请求 (含时间戳，但签名逻辑可验证) |

---

## 3. 单元测试用例

### 3.1 lib_tests.rs — lib.rs 工具函数

#### TC-L1: parse_f64
- **L1.1** 数字类型字段 → 返回正确 f64
- **L1.2** 字符串类型字段 → 解析为 f64
- **L1.3** 字段不存在 → 返回 None
- **L1.4** 字段为 null → 返回 None
- **L1.5** 无效字符串 → 返回 None
- **L1.6** 字段为整数 → 返回 f64

#### TC-L2: parse_str
- **L2.1** 字符串字段 → 返回 String
- **L2.2** i64 字段 → 转为 String
- **L2.3** f64 字段 → 转为 String
- **L2.4** 字段不存在 → 返回 None

#### TC-L3: parse_i64
- **L3.1** i64 字段 → 返回正确值
- **L3.2** 字符串数字 → 解析为 i64
- **L3.3** 无效字符串 → 返回 None
- **L3.4** 字段不存在 → 返回 None

#### TC-L4: parse_u32
- **L4.1** u64 字段 → 返回正确 u32
- **L4.2** 字符串数字 → 解析为 u32
- **L4.3** 字段不存在 → 返回 None

#### TC-L5: build_display_url
- **L5.1** 无参数 → 仅返回 path
- **L5.2** 有普通参数 → path?key=value&key2=value2
- **L5.3** 含 signature 参数 → signature 被脱敏为 ***MASKED***
- **L5.4** 空参数迭代器 → 仅返回 path

#### TC-L6: mask_signature
- **L6.1** 含 signature= 的 body → signature 值被脱敏
- **L6.2** signature= 后还有 & 参数 → 仅脱敏 signature 值，保留后续参数
- **L6.3** 不含 signature= → 原样返回
- **L6.4** 多个 signature= → 仅脱敏第一个

#### TC-L7: extract_error_message
- **L7.1** 含 msg + code → "[code] msg"
- **L7.2** 仅含 msg → 返回 msg
- **L7.3** 含 retMsg + retCode → "[retCode] retMsg" (Bybit 格式)
- **L7.4** 含 error 字段 → 返回 error
- **L7.5** 含 message 字段 → 返回 message
- **L7.6** 含 detail 字段 → 返回 detail
- **L7.7** 无匹配字段 → 返回 JSON 字符串

---

### 3.2 auth_tests.rs — 签名函数

#### TC-A1: hmac_sha256_hex
- **A1.1** 已知输入 → 与预期签名匹配 (RFC 4231 测试向量)
- **A1.2** 空消息 → 返回有效 hex
- **A1.3** 空密钥 → 返回有效 hex
- **A1.4** 相同输入 → 相同输出 (幂等性)
- **A1.5** 不同输入 → 不同输出

#### TC-A2: hmac_sha256_base64
- **A2.1** 已知输入 → 与预期签名匹配
- **A2.2** 相同输入 → 相同输出 (幂等性)

#### TC-A3: make_header
- **A3.1** 合法 ASCII 值 → 返回 Ok(HeaderValue)
- **A3.2** 包含非法字符 → 返回 Err

#### TC-A4: insert_header
- **A4.1** 插入合法 header → HeaderMap 包含该 header
- **A4.2** 非法值 → 返回 Err，HeaderMap 不变

---

### 3.3 errors_tests.rs — 错误类型

#### TC-E1: ExchangeError::exchange
- **E1.1** 构造 ExchangeError 变体 → code 和 message 正确

#### TC-E2: ExchangeError::no_data
- **E2.1** 构造 NoData 错误 → 消息正确

#### TC-E3: is_retryable
- **E3.1** Network → true
- **E3.2** RateLimited → true
- **E3.3** Internal → true
- **E3.4** Http 429 → true
- **E3.5** Http 500 → true
- **E3.6** Http 502 → true
- **E3.7** Http 503 → true
- **E3.8** Http 504 → true
- **E3.9** Http 400 → false
- **E3.10** Http 401 → false
- **E3.11** Authentication → false
- **E3.12** InvalidRequest → false
- **E3.13** OrderNotFound → false
- **E3.14** NotSupported → false

---

### 3.4 types_tests.rs — 类型转换

#### TC-T1: CcxtOrderStatus → OrderStatus
- **T1.1** Open → Open
- **T1.2** PartiallyFilled → PartiallyFilled
- **T1.3** Filled → Filled
- **T1.4** Canceled → Canceled
- **T1.5** Expired → Canceled (合并到 Canceled)
- **T1.6** Failed → Failed
- **T1.7** Rejected → Failed (合并到 Failed)

#### TC-T2: CcxtTicker → Ticker
- **T2.1** 所有字段有值 → 正确转换
- **T2.2** Optional 字段为 None → 默认 0.0
- **T2.3** timestamp 为 None → 使用当前时间

#### TC-T3: CcxtOrderBook → OrderBook
- **T3.1** 正常转换 → bids/asks 保留
- **T3.2** timestamp 为 None → 使用当前时间

#### TC-T4: CcxtFundingRate → FundingRate
- **T4.1** 正常转换 → rate 和 next_funding_time 保留

#### TC-T5: CcxtFundingHistoryEntry → FundingHistoryEntry
- **T5.1** 正常转换 → funding_time 和 rate 保留

---

### 3.5 binance_mod_tests.rs — Binance 映射函数

#### TC-B1: to_native_symbol
- **B1.1** "BTC/USDT" → "BTCUSDT"
- **B1.2** "BTC-USDT" → "BTCUSDT"
- **B1.3** "BTCUSDT" → "BTCUSDT" (已是无分隔符)
- **B1.4** "ETH/USDC" → "ETHUSDC"
- **B1.5** 空字符串 → 空字符串

#### TC-B2: to_unified_symbol
- **B2.1** "BTCUSDT" → "BTC/USDT"
- **B2.2** "ETHUSDC" → "ETH/USDC"
- **B2.3** "BNBBTC" → "BNB/BTC"
- **B2.4** "BTCBUSD" → "BTC/BUSD"
- **B2.5** 未知报价货币 → 原样返回
- **B2.6** 仅报价货币 (如 "USDT") → 原样返回 (base 为空)

#### TC-B3: parse_order_status
- **B3.1** "NEW" → Open
- **B3.2** "PARTIALLY_FILLED" → PartiallyFilled
- **B3.3** "FILLED" → Filled
- **B3.4** "CANCELED" → Canceled
- **B3.5** "CANCELLED" → Canceled (拼写变体)
- **B3.6** "EXPIRED" → Canceled
- **B3.7** "REJECTED" → Rejected
- **B3.8** "PENDING_CANCEL" → Open
- **B3.9** 未知状态 → Open (默认)

#### TC-B4: parse_order_type
- **B4.1** "MARKET" → Market
- **B4.2** "LIMIT" → Limit
- **B4.3** "STOP_MARKET" → StopMarket
- **B4.4** "STOP_LOSS" → StopMarket
- **B4.5** "STOP_LOSS_LIMIT" → StopLimit
- **B4.6** "TAKE_PROFIT_LIMIT" → StopLimit
- **B4.7** "TAKE_PROFIT_MARKET" → TakeProfitMarket
- **B4.8** 未知类型 → Market (默认)

#### TC-B5: side_str
- **B5.1** Side::Buy → "BUY"
- **B5.2** Side::Sell → "SELL"

#### TC-B6: order_type_str
- **B6.1** Market → "MARKET"
- **B6.2** Limit → "LIMIT"
- **B6.3** StopMarket → "STOP_MARKET"
- **B6.4** StopLimit → "STOP_LIMIT"
- **B6.5** TakeProfitMarket → "TAKE_PROFIT_MARKET"

#### TC-B7: try_build_ed25519
- **B7.1** PEM 格式密钥 → 返回 Ok(BinanceEd25519Signer)
- **B7.2** 32 字节 base64 seed → 返回 Ok(BinanceEd25519Signer)
- **B7.3** 非 32 字节 base64 → 返回 Err
- **B7.4** 非 base64 字符串 → 返回 Err (HMAC fallback)

---

### 3.6 orderbook_ws_tests.rs — OrderBook WS 解析

#### TC-W1: parse_levels
- **W1.1** 标准字符串数组 → 正确解析
- **W1.2** 数字类型元素 → 转为字符串
- **W1.3** 空数组 → 返回空 Vec
- **W1.4** 非数组 Value → 返回 None
- **W1.5** 元素少于 2 个 → 返回 None

#### TC-W2: to_levels
- **W2.1** 正常价格层级 → 正确转换
- **W2.2** amount 为 0 → 过滤掉
- **W2.3** amount 为负 → 过滤掉
- **W2.4** 无效数字字符串 → 过滤掉
- **W2.5** 空输入 → 空 Vec

#### TC-W3: parse_payload
- **W3.1** Spot 格式 (bids/asks) → 正确解析
- **W3.2** Perpetual 格式 (b/a + s + E) → 正确解析
- **W3.3** 缺少 bids/asks 和 b/a → 返回 None

#### TC-W4: BinanceDepthMessage::into_depth
- **W4.1** 组合流 spot 格式 → 正确提取
- **W4.2** 组合流 perpetual 格式 → 正确提取
- **W4.3** 单流 spot 格式 → 正确提取
- **W4.4** 单流 perpetual 格式 → 正确提取
- **W4.5** 无效消息 → 返回 None

---

### 3.7 ws_api_tests.rs — WebSocket API

#### TC-WA1: build_session_logon_request
- **WA1.1** 返回的 JSON 包含 method="session.logon"
- **WA1.2** params 包含 apiKey, recvWindow, timestamp, signature
- **WA1.3** signature 非空
- **WA1.4** 相同输入 (相同时间戳) → 相同签名 (幂等性)
- **WA1.5** 不同 API key → 不同签名

---

## 4. 集成测试用例

集成测试位于 `tests/integration_tests.rs`，测试跨模块协作和端到端数据流。

### TC-INT-1: 符号转换往返
- **INT-1.1** `to_native_symbol("BTC/USDT")` → `to_unified_symbol(result)` == "BTC/USDT"
- **INT-1.2** `to_native_symbol("ETH-USDC")` → `to_unified_symbol(result)` == "ETH/USDC"

### TC-INT-2: 签名 → URL 构建 → 脱敏 全链路
- **INT-2.1** HMAC 签名生成 → build_display_url 包含 signature → signature 被脱敏
- **INT-2.2** HMAC 签名生成 → mask_signature 脱敏 body

### TC-INT-3: 错误响应 → 错误提取 → is_retryable 判定
- **INT-3.1** 429 HTTP 错误 JSON → extract_error_message → is_retryable == true
- **INT-3.2** 400 HTTP 错误 JSON → extract_error_message → is_retryable == false

### TC-INT-4: WS 消息解析 → WsFeedEvent 转换全链路
- **INT-4.1** executionReport JSON → BinanceOrderMessage → to_ws_feed_event → WsFeedEvent::OrderUpdate
- **INT-4.2** ORDER_TRADE_UPDATE JSON → BinanceOrderMessage → to_ws_feed_event → WsFeedEvent::OrderUpdate

### TC-INT-5: Kline WS 消息 → Candle 转换全链路
- **INT-5.1** 组合流 kline JSON → BinanceKlineMessage → into_kline_data → to_candle

### TC-INT-6: OrderBook WS 消息 → WsOrderBookUpdate 全链路
- **INT-6.1** 组合流 spot depth JSON → BinanceDepthMessage → into_depth → to_levels
- **INT-6.2** 组合流 perpetual depth JSON → BinanceDepthMessage → into_depth → to_levels

### TC-INT-7: create_exchange 工厂函数
- **INT-7.1** "binance" + HMAC secret → 创建成功，ed25519_signer 为 None
- **INT-7.2** "binance" + Ed25519 PEM → 创建成功，ed25519_signer 为 Some
- **INT-7.3** "bybit" → 返回 NotSupported 错误
- **INT-7.4** "okx" → 返回 NotSupported 错误

### TC-INT-8: 类型转换链 (REST 响应模拟)
- **INT-8.1** 模拟 ticker JSON → parse_f64 提取字段 → CcxtTicker 构造 → From<Ticker>
- **INT-8.2** 模拟 order JSON → parse_str/parse_f64 提取 → CcxtOrderStatus → From<OrderStatus>

---

## 5. 测试文件与模块映射

| 测试文件 | 被测模块 | 测试用例数 |
|----------|----------|-----------|
| `src/lib_tests.rs` | lib.rs | 23 |
| `src/auth_tests.rs` | auth.rs | 11 |
| `src/errors_tests.rs` | errors.rs | 14 |
| `src/types_tests.rs` | types.rs | 16 |
| `src/adapter/binance/mod_tests.rs` | adapter/binance/mod.rs | 28 |
| `src/adapter/binance/orderbook_ws_tests.rs` | orderbook_ws.rs | 18 |
| `src/adapter/binance/ws_api_tests.rs` | ws_api.rs | 5 |
| `tests/integration_tests.rs` | 跨模块 | 16 |
| **合计** | | **131** |

> 注: `kline_ws.rs` 和 `order_ws.rs` 已有内联测试 (共约 25 个)，不重复编写。

---

## 6. 重构需求

### 6.1 提取 `parse_order_book_side` 到共享位置

`api.rs` 和 `fapi.rs` 各有一份完全相同的 `parse_order_book_side` 函数。应提取到 `adapter/binance/mod.rs` 或单独的 `adapter/binance/shared.rs` 模块中，避免代码重复。

### 6.2 公开测试所需函数

部分函数当前为私有 (`fn` 而非 `pub fn`)，需要提升可见性以供 `_tests` 文件引用:
- `lib.rs`: `build_display_url`, `mask_signature`, `extract_error_message` → `pub(crate)`
- `orderbook_ws.rs`: `parse_levels`, `to_levels`, `parse_payload`, `BinanceDepthMessage` → `pub(crate)`
- `ws_api.rs`: `build_session_logon_request` → `pub(crate)`

### 6.3 `_tests` 文件模式

每个测试文件以 `_tests.rs` 后缀命名，通过 `#[cfg(test)] mod xxx_tests;` 在父模块中引入，保持源文件与测试文件分离。

---

## 7. 测试用例与文档对比审查报告

### 7.1 数量对比

| 测试文件 | 文档计划 | 实际实现 | 差异 | 状态 |
|----------|---------|---------|------|------|
| `lib_tests.rs` | 23 | 32 | +9 | ✅ 超额完成 |
| `auth_tests.rs` | 11 | 11 | 0 | ✅ 完全匹配 |
| `errors_tests.rs` | 14 | 16 | +2 | ✅ 超额完成 |
| `types_tests.rs` | 16 | 14 | -2 | ✅ 略少 (合并部分用例) |
| `mod_tests.rs` | 28 | 43 | +15 | ✅ 超额完成 |
| `orderbook_ws_tests.rs` | 18 | 18 | 0 | ✅ 完全匹配 |
| `ws_api_tests.rs` | 5 | 5 | 0 | ✅ 完全匹配 |
| `integration_tests.rs` | 16 | 25 | +9 | ✅ 超额完成 |
| 已有内联测试 (kline_ws + order_ws) | — | 25 | — | ✅ 已修复 (+1 ORDER_TRADE_UPDATE 单流测试) |
| **合计** | **131** | **190** | **+59** | ✅ 全部通过 |

### 7.2 文档中每个测试用例的实现状态

所有文档中列出的测试用例 ID 均已实现并通过：

- **TC-L1 ~ TC-L7** (lib_tests.rs): ✅ 全部实现
- **TC-A1 ~ TC-A4** (auth_tests.rs): ✅ 全部实现
- **TC-E1 ~ TC-E3** (errors_tests.rs): ✅ 全部实现
- **TC-T1 ~ TC-T5** (types_tests.rs): ✅ 全部实现
- **TC-B1 ~ TC-B7** (mod_tests.rs): ✅ 全部实现
- **TC-F1** (mod_tests.rs - parse_order_book_side): ✅ 全部实现
- **TC-W1 ~ TC-W4** (orderbook_ws_tests.rs): ✅ 全部实现
- **TC-WA1** (ws_api_tests.rs): ✅ 全部实现
- **TC-INT-1 ~ TC-INT-8** (integration_tests.rs): ✅ 全部实现

### 7.3 业务逻辑使用验证

每个被测函数均确认在生产代码中被正确使用：

| 函数 | 使用位置 | 用途 |
|------|---------|------|
| `parse_f64/str/i64/u32` | api.rs, fapi.rs | 解析 REST 响应字段 |
| `build_display_url` | lib.rs (signed_get, signed_post) | 构建日志 URL |
| `mask_signature` | lib.rs (signed_get, signed_post) | 脱敏签名参数 |
| `extract_error_message` | lib.rs (handle_response) | 提取错误消息 |
| `hmac_sha256_hex` | binance/mod.rs (BinanceSigner) | HMAC 签名 |
| `hmac_sha256_base64` | pub 工具函数 | 预留 (未来交易所如 Bybit) |
| `make_header/insert_header` | binance/mod.rs (BinanceSigner) | 添加 API key header |
| `to_native/unified_symbol` | api.rs, fapi.rs, ws clients | 符号格式转换 |
| `parse_order_status/type` | api.rs, fapi.rs, order_ws.rs | 订单状态/类型映射 |
| `side_str/order_type_str` | api.rs, fapi.rs | 创建订单参数 |
| `parse_order_book_side` | api.rs, fapi.rs | 解析订单簿 |
| `parse_levels/to_levels/parse_payload` | orderbook_ws.rs | WS 深度解析 |
| `build_session_logon_request` | ws_api.rs | WS API 认证 |
| `try_build_ed25519` | binance/mod.rs (new) | 签名器初始化 |

### 7.4 回归审查发现的问题及修复

| # | 问题 | 类型 | 修复 |
|---|------|------|------|
| 1 | `parse_order_type("STOP_LIMIT")` 无匹配项，回退到 Market | 生产 bug | 添加 `"STOP_LIMIT"` 到 match arms |
| 2 | `last_update_id` 字段缺少 `#[serde(rename = "lastUpdateId")]` | 生产 bug | 添加 serde rename 属性 |
| 3 | `test_new_perpetual` 期望 URL `/ws/` 但代码已改为 `/private/ws/` | 测试过时 | 更新期望值 |
| 4 | `test_update_listen_key` 同上 | 测试过时 | 更新期望值 |
| 5 | `api.rs` 和 `fapi.rs` 各有一份重复的 `parse_order_book_side` | 代码重复 | 提取到 `binance/mod.rs` 共享 |
| 6 | `OrderBookLevel` 缺少 `PartialEq` | 测试需求 | 添加 `#[derive(PartialEq)]` |
| 7 | `WsFeedEvent` 缺少 `PartialEq` | 测试需求 | 添加 `#[derive(PartialEq)]` |

### 7.5 孤儿代码检查

- **`hmac_sha256_base64`**: 定义为 `pub fn`，当前 crate 内未使用，保留供未来交易所实现（如 Bybit 使用 base64 编码的 HMAC-SHA256）。非孤儿代码。
- 其余所有函数均确认在业务代码中使用，无孤儿代码。
- Clippy 检查通过，无死代码警告。
