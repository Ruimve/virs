# virs-config 测试用例文档

> 生成日期: 2026-06-30
> Crate: `crates/libs/virs-config`
> 状态: **28 个测试全部通过** (11 单元 + 4 serde + 13 集成)

---

## 单元测试用例

### app_config_tests.rs — 纯函数、常量与 TimeConfig (11)

| ID | 测试函数 | 描述 |
|----|---------|------|
| C3.1 | `c3_1_parse_env_num_with_value` | Some("8080") → Ok(8080) |
| C3.2 | `c3_2_parse_env_num_none_uses_default` | None → Ok(默认值) |
| C3.3 | `c3_3_parse_env_num_invalid_returns_err` | Some("invalid") → Err |
| C3.4 | `c3_4_parse_env_num_empty_string_returns_err` | Some("") → Err |
| C3.5 | `c3_5_parse_env_num_type_safety` | u16/u32/u64/i64 类型安全 |
| S3.1 | `s3_1_default_constants_values` | 所有 DEFAULT_* 常量值验证 |
| T12.1 | `t12_1_time_config_default_values` | TimeConfig::default() 全字段默认值 |
| T12.2 | `t12_2_time_config_default_constants` | DEFAULT_*_SECS 常量值验证 |
| T12.3 | `t12_3_time_config_serde_roundtrip` | TimeConfig serde 往返 |
| T12.4 | `t12_4_time_config_clone_and_eq` | TimeConfig clone + PartialEq |
| T12.5 | `t12_5_time_config_max_position_duration_is_48h` | max_position_duration_secs = 48h (172800s) |

### serde_tests.rs — 序列化/反序列化 (4)

| ID | 测试函数 | 描述 |
|----|---------|------|
| S1.1 | `s1_1_app_config_full_roundtrip` | 完整 AppConfig serde 往返 |
| S2.1 | `s2_1_server_config_roundtrip` | ServerConfig serde 往返 |
| S2.2 | `s2_2_database_config_roundtrip` | DatabaseConfig serde 往返 |
| S2.4 | `s2_4_admin_config_roundtrip_with_none_id` | AdminConfig (id=None) 往返 |

---

## 集成测试用例

### integration_tests.rs (13)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-1.1 | `int_1_1_load_config_minimal_required` | 仅必需变量 → 加载成功 |
| INT-1.2 | `int_1_2_load_config_missing_llm_key` | 缺 LLM_KEY → Err |
| INT-1.3 | `int_1_3_load_config_missing_encryption_key` | 缺 ENCRYPTION_KEY → Err |
| INT-1.4 | `int_1_4_load_config_missing_database_url` | 缺 DATABASE_URL → Err |
| INT-2.1 | `int_2_1_default_port` | 默认 port=8080 |
| INT-2.2 | `int_2_2_default_host` | 默认 host="0.0.0.0" |
| INT-2.4 | `int_2_4_default_jwt_hours` | 默认 jwt_expiration_hours=24 |
| INT-2.5 | `int_2_5_default_db_pool` | 默认 pool_min=5, pool_max=50 |
| INT-5.1 | `int_5_1_custom_port` | PORT="3000" → port=3000 |
| INT-5.2 | `int_5_2_custom_db_pool_max` | DB_POOL_MAX="100" → pool_max=100 |
| INT-5.4 | `int_5_4_custom_proxy` | PROXY_URL → proxy=Some |
| INT-6.1 | `int_6_1_same_encryption_and_llm_key_rejected` | ENCRYPTION_KEY == LLM_KEY → Err |
| INT-6.2 | `int_6_2_different_encryption_and_llm_key_accepted` | ENCRYPTION_KEY != LLM_KEY → Ok |

---

## 代码覆盖率

### 测试文件与模块映射

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/app_config_tests.rs` | app_config.rs 纯函数 + 常量 + TimeConfig | 11 |
| `src/serde_tests.rs` | serde 序列化/反序列化 | 4 |
| `tests/integration_tests.rs` | load_config 端到端 | 13 |
| **合计** | | **28** |

### 死代码清理记录

| 已删除项 | 位置 | 删除原因 |
|---------|------|---------|
| `virs-types` 依赖 | Cargo.toml | 源码中零引用，未使用依赖 |
| `build_notification_config` 文档条目 | test_plan.md | 计划中列出但从未实现，phantom 条目 |
