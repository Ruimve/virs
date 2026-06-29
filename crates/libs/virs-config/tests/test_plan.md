# virs-config 测试用例文档

> 生成日期: 2026-06-29
> Crate: `crates/libs/virs-config`
> 目标: 抽取配置解析中的幂等函数逻辑，进行完整单元测试和集成测试覆盖。
> 状态: **全部 67 个测试通过** (42 单元 + 25 集成)

---

## 1. Crate 结构概览

```
virs-config/
├── src/
│   ├── lib.rs            # 模块入口，re-export app_config
│   └── app_config.rs     # AppConfig 及所有子配置结构体 + load_config() 函数
├── tests/
│   ├── test_plan.md      # 本文档
│   └── integration_tests.rs  # 集成测试
```

### 核心逻辑分析

`app_config.rs` 包含：
- **10 个配置结构体**: `AppConfig`, `ServerConfig`, `DatabaseConfig`, `RedisConfig`, `AdminConfig`, `AiConfig`, `NotificationConfig`, `TelegramConfig`, `EmailConfig`, `CacheConfig`
- **1 个加载函数**: `load_config()` — 从环境变量读取并构造 `AppConfig`
- **1 个辅助函数**: `default_paper()` — 返回 `Some(true)`

### 问题识别

`load_config()` 是一个 100+ 行的单一函数，将"环境变量读取"和"配置构造"逻辑混在一起，导致：
1. 无法对配置构造逻辑进行单元测试（依赖全局 env 状态）
2. 解析逻辑（如 paper trading 的 "true"/"1" 判断）内联在函数中
3. 可选配置的条件构造逻辑（如 telegram 需要 token+chat_id 同时存在）不可复用

---

## 2. 重构方案：抽取幂等函数

将 `load_config()` 中的纯逻辑抽取为以下幂等函数：

### 2.1 纯解析函数

| # | 函数 | 签名 | 说明 |
|---|------|------|------|
| C1 | `parse_paper_value` | `(v: Option<String>) -> Option<bool>` | 解析 paper trading 环境变量值 |
| C2 | `parse_bool_str` | `(v: &str) -> bool` | 将字符串解析为 bool ("true"/"1" → true) |
| C3 | `parse_env_num<T>` | `(value: Option<String>, default: &str) -> Result<T>` | 泛型数值解析，带默认值 |

### 2.2 配置构造函数

| # | 函数 | 签名 | 说明 |
|---|------|------|------|
| C4 | `build_redis_config` | `(url: Option<String>, password: Option<String>) -> Option<RedisConfig>` | 条件构造 Redis 配置 |
| C5 | `build_telegram_config` | `(bot_token: Option<String>, chat_id: Option<String>) -> Option<TelegramConfig>` | 条件构造 Telegram 配置 (需要两者同时存在) |
| C6 | `build_email_config` | `(host, username, password: Option<String>, port: Option<String>, from: Option<String>) -> Option<EmailConfig>` | 条件构造 Email 配置 (需要 host+username+password 同时存在) |
| C7 | `build_notification_config` | `(telegram, email) -> NotificationConfig` | 组装通知配置 |

### 2.3 默认值常量

| # | 常量 | 值 | 说明 |
|---|------|----|------|
| D1 | `DEFAULT_HOST` | `"0.0.0.0"` | 服务器默认监听地址 |
| D2 | `DEFAULT_PORT` | `"8080"` | 服务器默认端口 |
| D3 | `DEFAULT_LOG_LEVEL` | `"info"` | 默认日志级别 |
| D4 | `DEFAULT_JWT_HOURS` | `"24"` | JWT 默认过期时间 |
| D5 | `DEFAULT_DB_POOL_MIN` | `"5"` | 数据库连接池最小值 |
| D6 | `DEFAULT_DB_POOL_MAX` | `"50"` | 数据库连接池最大值 |
| D7 | `DEFAULT_SMTP_PORT` | `"587"` | SMTP 默认端口 |
| D8 | `DEFAULT_SMTP_FROM` | `"noreply@virs.com"` | 默认发件人 |
| D9 | `DEFAULT_CACHE_TTL_TICKER` | `"10"` | Ticker 缓存默认 TTL |
| D10 | `DEFAULT_CACHE_TTL_KLINE_1M` | `"60"` | 1m K线缓存默认 TTL |
| D11 | `DEFAULT_CACHE_TTL_KLINE_5M` | `"120"` | 5m K线缓存默认 TTL |
| D12 | `DEFAULT_CACHE_TTL_KLINE_1H` | `"300"` | 1h K线缓存默认 TTL |
| D13 | `DEFAULT_CACHE_TTL_KLINE_1D` | `"3600"` | 1d K线缓存默认 TTL |
| D14 | `DEFAULT_ADMIN_USERNAME` | `"admin"` | 默认管理员用户名 |
| D15 | `DEFAULT_ADMIN_PASSWORD` | `"admin123"` | 默认管理员密码 |

---

## 3. 单元测试用例

### 3.1 app_config_tests.rs — 纯函数测试

#### TC-C1: parse_paper_value
- **C1.1** `Some("true")` → `Some(true)`
- **C1.2** `Some("1")` → `Some(true)`
- **C1.3** `Some("false")` → `Some(false)`
- **C1.4** `Some("0")` → `Some(false)`
- **C1.5** `Some("anything_else")` → `Some(false)`
- **C1.6** `None` → `Some(true)` (默认开启 paper trading)

#### TC-C2: parse_bool_str
- **C2.1** `"true"` → `true`
- **C2.2** `"1"` → `true`
- **C2.3** `"false"` → `false`
- **C2.4** `"0"` → `false`
- **C2.5** `"yes"` → `false`
- **C2.6** `""` → `false`
- **C2.7** `"TRUE"` → `false` (大小写敏感)

#### TC-C3: parse_env_num
- **C3.1** `Some("8080"), "80"` → `Ok(8080)`
- **C3.2** `None, "80"` → `Ok(80)`
- **C3.3** `Some("invalid"), "80"` → `Err`
- **C3.4** `Some(""), "80"` → `Err`
- **C3.5** 类型安全: u16, u32, u64, i64 各类型正确解析

#### TC-C4: build_redis_config
- **C4.1** url=Some, password=Some → `Some(RedisConfig)`
- **C4.2** url=Some, password=None → `Some(RedisConfig)` (password 为 None)
- **C4.3** url=None, password=Some → `None` (无 url 不创建)
- **C4.4** url=None, password=None → `None`

#### TC-C5: build_telegram_config
- **C5.1** token=Some, chat_id=Some → `Some(TelegramConfig)`
- **C5.2** token=Some, chat_id=None → `None`
- **C5.3** token=None, chat_id=Some → `None`
- **C5.4** token=None, chat_id=None → `None`

#### TC-C6: build_email_config
- **C6.1** host=Some, user=Some, pass=Some, port=Some("465"), from=Some → `Some(EmailConfig)` with port=465
- **C6.2** host=Some, user=Some, pass=Some, port=None, from=None → `Some(EmailConfig)` with default port=587, default from
- **C6.3** host=None → `None`
- **C6.4** host=Some, user=None → `None`
- **C6.5** host=Some, user=Some, pass=None → `None`
- **C6.6** port=Some("invalid") → 使用默认端口 587

### 3.2 serde_tests.rs — 序列化/反序列化测试

#### TC-S1: AppConfig serde round-trip
- **S1.1** 完整 AppConfig 序列化 → 反序列化 → 字段一致
- **S1.2** paper 字段缺失时使用默认值 `Some(true)`

#### TC-S2: 各子配置 serde round-trip
- **S2.1** ServerConfig round-trip
- **S2.2** DatabaseConfig round-trip
- **S2.3** RedisConfig round-trip (含 password=None)
- **S2.4** AdminConfig round-trip (含 id=None)
- **S2.5** CacheConfig round-trip
- **S2.6** TelegramConfig round-trip
- **S2.7** EmailConfig round-trip

#### TC-S3: 默认值常量验证
- **S3.1** 所有 DEFAULT_* 常量值与文档一致

---

## 4. 集成测试用例

### TC-INT-1: load_config 最小必需环境变量
- **INT-1.1** 仅设置必需变量 (SECRET_KEY, ENCRYPTION_KEY, DATABASE_URL) → 加载成功，其余使用默认值
- **INT-1.2** 缺少 SECRET_KEY → 返回错误
- **INT-1.3** 缺少 ENCRYPTION_KEY → 返回错误
- **INT-1.4** 缺少 DATABASE_URL → 返回错误

### TC-INT-2: load_config 默认值验证
- **INT-2.1** 不设置 PORT → port == 8080
- **INT-2.2** 不设置 HOST → host == "0.0.0.0"
- **INT-2.3** 不设置 LOG_LEVEL → log_level == "info"
- **INT-2.4** 不设置 JWT_EXPIRATION_HOURS → jwt_expiration_hours == 24
- **INT-2.5** 不设置 DB_POOL_MIN/MAX → 5/50

### TC-INT-3: load_config 可选配置
- **INT-3.1** 设置 REDIS_URL → redis == Some, 设置 REDIS_PASSWORD → password == Some
- **INT-3.2** 不设置 REDIS_URL → redis == None
- **INT-3.3** 设置 TELEGRAM_BOT_TOKEN + TELEGRAM_CHAT_ID → telegram == Some
- **INT-3.4** 仅设置 TELEGRAM_BOT_TOKEN → telegram == None
- **INT-3.5** 设置 SMTP_HOST + SMTP_USERNAME + SMTP_PASSWORD → email == Some
- **INT-3.6** 设置 SMTP_PORT → email.port == 自定义值

### TC-INT-4: load_config paper trading
- **INT-4.1** PAPER_TRADING="true" → paper == Some(true)
- **INT-4.2** PAPER_TRADING="1" → paper == Some(true)
- **INT-4.3** PAPER_TRADING="false" → paper == Some(false)
- **INT-4.4** 不设置 PAPER_TRADING → paper == Some(true) (默认安全模式)

### TC-INT-5: load_config 自定义值
- **INT-5.1** 设置 PORT="3000" → port == 3000
- **INT-5.2** 设置 DB_POOL_MAX="100" → pool_max == 100
- **INT-5.3** 设置 CACHE_TTL_TICKER="30" → ttl_ticker == 30
- **INT-5.4** 设置 PROXY_URL → proxy == Some

### TC-INT-6: 配置链路 — 构造函数 → AppConfig
- **INT-6.1** build_redis_config → 嵌入 AppConfig → serde round-trip
- **INT-6.2** build_telegram_config + build_email_config → NotificationConfig → AppConfig round-trip

---

## 5. 测试文件与模块映射

| 测试文件 | 被测模块 | 文档计划 | 实际实现 | 状态 |
|----------|----------|---------|---------|------|
| `src/app_config_tests.rs` | app_config.rs 纯函数 | 25 | 33 | ✅ 超额完成 |
| `src/serde_tests.rs` | serde 序列化/反序列化 | 10 | 9 | ✅ 基本匹配 |
| `tests/integration_tests.rs` | load_config 端到端 | 22 | 25 | ✅ 超额完成 |
| **合计** | | **57** | **67** | ✅ 全部通过 |

### 6.1 抽取纯函数

将 `load_config()` 中的逻辑拆分为：
1. 纯解析函数（不依赖 env）：`parse_paper_value`, `parse_bool_str`, `parse_env_num`
2. 纯构造函数（不依赖 env）：`build_redis_config`, `build_telegram_config`, `build_email_config`
3. 默认值常量：`DEFAULT_*` 常量
4. `load_config_from_env()` — 纯环境变量读取，不含 dotenvy 加载

`load_config()` 改为调用 `load_config_from_env()`，保持行为不变。

### 6.2 公开测试所需函数

- 纯函数和构造函数设为 `pub(crate)` 以供 `_tests` 文件引用
- 默认值常量设为 `pub(crate)`
- `load_config_from_env` 设为 `pub` 以供集成测试引用

### 6.3 `_tests` 文件模式

- `src/app_config_tests.rs` — 纯函数和构造函数测试
- `src/serde_tests.rs` — serde round-trip 测试
- 通过 `#[cfg(test)] mod xxx_tests;` 在 `lib.rs` 中引入

---

## 7. 测试用例与文档对比审查报告

### 7.1 数量对比

| 测试文件 | 文档计划 | 实际实现 | 差异 | 状态 |
|----------|---------|---------|------|------|
| `app_config_tests.rs` | 25 | 33 | +8 | ✅ 超额完成 |
| `serde_tests.rs` | 10 | 9 | -1 | ✅ 基本匹配 (合并 S3) |
| `integration_tests.rs` | 22 | 25 | +3 | ✅ 超额完成 |
| **合计** | **57** | **67** | **+10** | ✅ 全部通过 |

### 7.2 文档中每个测试用例的实现状态

所有文档中列出的测试用例 ID 均已实现并通过：

- **TC-C1** (parse_paper_value): ✅ 6/6 实现
- **TC-C2** (parse_bool_str): ✅ 7/7 实现
- **TC-C3** (parse_env_num): ✅ 5/5 实现
- **TC-C4** (build_redis_config): ✅ 4/4 实现
- **TC-C5** (build_telegram_config): ✅ 4/4 实现
- **TC-C6** (build_email_config): ✅ 6/6 实现
- **TC-S1** (AppConfig serde): ✅ 2/2 实现
- **TC-S2** (子配置 serde): ✅ 7/7 实现
- **TC-S3** (默认值常量): ✅ 合并到 app_config_tests.rs
- **TC-INT-1** (必需变量): ✅ 4/4 实现
- **TC-INT-2** (默认值): ✅ 5/5 实现
- **TC-INT-3** (可选配置): ✅ 6/6 实现
- **TC-INT-4** (paper trading): ✅ 4/4 实现
- **TC-INT-5** (自定义值): ✅ 4/4 实现
- **TC-INT-6** (配置链路): ✅ 2/2 实现

### 7.3 业务逻辑使用验证

每个被测函数均确认在 `load_config_from_env()` 中被正确使用：

| 函数 | 使用位置 | 用途 |
|------|---------|------|
| `parse_paper_value` | `load_config_from_env` paper 字段 | 解析 PAPER_TRADING 环境变量 |
| `parse_bool_str` | 被 `parse_paper_value` 调用 | 底层 bool 字符串解析 |
| `parse_env_num` | `load_config_from_env` port/pool/cache TTL 等 | 泛型数值解析 |
| `build_redis_config` | `load_config_from_env` redis 字段 | 条件构造 Redis 配置 |
| `build_telegram_config` | `load_config_from_env` notification.telegram | 条件构造 Telegram 配置 |
| `build_email_config` | `load_config_from_env` notification.email | 条件构造 Email 配置 |
| 所有 `DEFAULT_*` 常量 | `load_config_from_env` 各字段默认值 | 提供回退默认值 |
| `load_config_from_env` | 被 `load_config` 调用 | 纯环境变量读取 |
| `load_config` | 被 `virs-app/main.rs` 调用 | 应用启动入口 |

### 7.4 回归审查发现的问题及修复

| # | 问题 | 类型 | 修复 |
|---|------|------|------|
| 1 | `load_config()` 将 dotenvy 加载和 env 读取混在一起，无法测试缺失变量场景 | 可测试性 | 拆分为 `load_config()` (dotenvy + 委托) 和 `load_config_from_env()` (纯 env 读取) |
| 2 | paper trading 解析逻辑内联在 load_config 中 | 可测试性 | 提取为 `parse_paper_value` 纯函数 |
| 3 | Redis/Telegram/Email 条件构造逻辑内联 | 可测试性 | 提取为 `build_*_config` 纯函数 |
| 4 | 所有配置结构体缺少 `PartialEq` | 测试需求 | 添加 `#[derive(PartialEq)]` |
| 5 | 默认值硬编码在 `unwrap_or_else` 闭包中 | 可维护性 | 提取为 `DEFAULT_*` 常量 |

### 7.5 孤儿代码检查

- 所有 `pub(crate)` 函数均在 `load_config_from_env()` 中被调用
- 所有 `DEFAULT_*` 常量均在 `load_config_from_env()` 中被引用
- `load_config_from_env` 被 `load_config()` 调用
- `load_config` 被应用主程序调用
- `default_paper` 被 serde `#[serde(default = "default_paper")]` 引用
- Clippy 检查通过，零警告，无死代码
