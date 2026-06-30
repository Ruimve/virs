# virs-config 测试用例文档

> 生成日期: 2026-06-30
> Crate: `crates/libs/virs-config`
> 状态: **67 个测试全部通过** (42 单元 + 25 集成)

---

## 单元测试用例

### app_config_tests.rs — 纯函数与配置构造 (33)

| ID | 测试函数 | 描述 |
|----|---------|------|
| C1.1 | `c1_1_parse_paper_value_true` | Some("true") → Some(true) |
| C1.2 | `c1_2_parse_paper_value_one` | Some("1") → Some(true) |
| C1.3 | `c1_3_parse_paper_value_false` | Some("false") → Some(false) |
| C1.4 | `c1_4_parse_paper_value_zero` | Some("0") → Some(false) |
| C1.5 | `c1_5_parse_paper_value_other` | Some("anything_else") → Some(false) |
| C1.6 | `c1_6_parse_paper_value_none_defaults_to_true` | None → Some(true) |
| C2.1 | `c2_1_parse_bool_str_true` | "true" → true |
| C2.2 | `c2_2_parse_bool_str_one` | "1" → true |
| C2.3 | `c2_3_parse_bool_str_false` | "false" → false |
| C2.4 | `c2_4_parse_bool_str_zero` | "0" → false |
| C2.5 | `c2_5_parse_bool_str_yes_is_false` | "yes" → false |
| C2.6 | `c2_6_parse_bool_str_empty` | "" → false |
| C2.7 | `c2_7_parse_bool_str_case_sensitive` | "TRUE" → false (大小写敏感) |
| C3.1 | `c3_1_parse_env_num_with_value` | Some("8080") → Ok(8080) |
| C3.2 | `c3_2_parse_env_num_none_uses_default` | None → Ok(默认值) |
| C3.3 | `c3_3_parse_env_num_invalid_returns_err` | Some("invalid") → Err |
| C3.4 | `c3_4_parse_env_num_empty_string_returns_err` | Some("") → Err |
| C3.5 | `c3_5_parse_env_num_type_safety` | u16/u32/u64/i64 类型安全 |
| C4.1 | `c4_1_build_redis_config_with_password` | url+password → Some(RedisConfig) |
| C4.2 | `c4_2_build_redis_config_without_password` | url only → Some(RedisConfig), password=None |
| C4.3 | `c4_3_build_redis_config_no_url` | url=None → None |
| C4.4 | `c4_4_build_redis_config_both_none` | 全 None → None |
| C5.1 | `c5_1_build_telegram_config_both_present` | token+chat_id → Some |
| C5.2 | `c5_2_build_telegram_config_missing_chat_id` | 缺 chat_id → None |
| C5.3 | `c5_3_build_telegram_config_missing_token` | 缺 token → None |
| C5.4 | `c5_4_build_telegram_config_both_none` | 全 None → None |
| C6.1 | `c6_1_build_email_config_all_fields` | 全字段 → Some, port=465 |
| C6.2 | `c6_2_build_email_config_defaults_for_port_and_from` | port/from 缺失 → 默认 587/noreply@virs.com |
| C6.3 | `c6_3_build_email_config_no_host` | 缺 host → None |
| C6.4 | `c6_4_build_email_config_no_username` | 缺 username → None |
| C6.5 | `c6_5_build_email_config_no_password` | 缺 password → None |
| C6.6 | `c6_6_build_email_config_invalid_port_uses_default` | port 非数字 → 默认 587 |
| S3.1 | `s3_1_default_constants_values` | 所有 DEFAULT_* 常量值验证 |

### serde_tests.rs — 序列化/反序列化 (9)

| ID | 测试函数 | 描述 |
|----|---------|------|
| S1.1 | `s1_1_app_config_full_roundtrip` | 完整 AppConfig serde 往返 |
| S1.2 | `s1_2_app_config_paper_missing_uses_default` | paper 字段缺失 → 默认 Some(true) |
| S2.1 | `s2_1_server_config_roundtrip` | ServerConfig serde 往返 |
| S2.2 | `s2_2_database_config_roundtrip` | DatabaseConfig serde 往返 |
| S2.3 | `s2_3_redis_config_roundtrip_with_none_password` | RedisConfig (password=None) 往返 |
| S2.4 | `s2_4_admin_config_roundtrip_with_none_id` | AdminConfig (id=None) 往返 |
| S2.5 | `s2_5_cache_config_roundtrip` | CacheConfig serde 往返 |
| S2.6 | `s2_6_telegram_config_roundtrip` | TelegramConfig serde 往返 |
| S2.7 | `s2_7_email_config_roundtrip` | EmailConfig serde 往返 |

---

## 集成测试用例

### integration_tests.rs (25)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-1.1 | `int_1_1_load_config_minimal_required` | 仅必需变量 → 加载成功 |
| INT-1.2 | `int_1_2_load_config_missing_secret_key` | 缺 SECRET_KEY → Err |
| INT-1.3 | `int_1_3_load_config_missing_encryption_key` | 缺 ENCRYPTION_KEY → Err |
| INT-1.4 | `int_1_4_load_config_missing_database_url` | 缺 DATABASE_URL → Err |
| INT-2.1 | `int_2_1_default_port` | 默认 port=8080 |
| INT-2.2 | `int_2_2_default_host` | 默认 host="0.0.0.0" |
| INT-2.3 | `int_2_3_default_log_level` | 默认 log_level="info" |
| INT-2.4 | `int_2_4_default_jwt_hours` | 默认 jwt_expiration_hours=24 |
| INT-2.5 | `int_2_5_default_db_pool` | 默认 pool_min=5, pool_max=50 |
| INT-3.1 | `int_3_1_redis_with_password` | Redis URL+password → Some |
| INT-3.2 | `int_3_2_redis_not_set` | 无 Redis URL → None |
| INT-3.3 | `int_3_3_telegram_both_set` | Telegram token+chat_id → Some |
| INT-3.4 | `int_3_4_telegram_only_token` | 仅 token → None |
| INT-3.5 | `int_3_5_email_all_required_set` | Email 全字段 → Some, 默认 port/from |
| INT-3.6 | `int_3_6_email_custom_port` | Email 自定义 port=465 |
| INT-4.1 | `int_4_1_paper_trading_true` | PAPER_TRADING="true" → Some(true) |
| INT-4.2 | `int_4_2_paper_trading_one` | PAPER_TRADING="1" → Some(true) |
| INT-4.3 | `int_4_3_paper_trading_false` | PAPER_TRADING="false" → Some(false) |
| INT-4.4 | `int_4_4_paper_trading_not_set_defaults_true` | 未设置 → Some(true) |
| INT-5.1 | `int_5_1_custom_port` | PORT="3000" → port=3000 |
| INT-5.2 | `int_5_2_custom_db_pool_max` | DB_POOL_MAX="100" → pool_max=100 |
| INT-5.3 | `int_5_3_custom_cache_ttl` | CACHE_TTL_TICKER="30" → ttl_ticker=30 |
| INT-5.4 | `int_5_4_custom_proxy` | PROXY_URL → proxy=Some |
| INT-6.1 | `int_6_1_redis_config_in_appconfig_roundtrip` | Redis → AppConfig serde 往返 |
| INT-6.2 | `int_6_2_notification_config_in_appconfig_roundtrip` | Notification → AppConfig serde 往返 |

---

## 代码覆盖率

### 测试文件与模块映射

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/app_config_tests.rs` | app_config.rs 纯函数+构造函数+常量 | 33 |
| `src/serde_tests.rs` | serde 序列化/反序列化 | 9 |
| `tests/integration_tests.rs` | load_config 端到端 | 25 |
| **合计** | | **67** |

### 死代码清理记录

| 已删除项 | 位置 | 删除原因 |
|---------|------|---------|
| `virs-types` 依赖 | Cargo.toml | 源码中零引用，未使用依赖 |
| `build_notification_config` 文档条目 | test_plan.md | 计划中列出但从未实现，phantom 条目 |
