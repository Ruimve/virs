# virs-config - 测试计划

**测试总数:** 28（单元测试: 15，集成测试: 13）

---

## 单元测试

### `crates/infra/virs-config/src/serde_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `s1_1_app_config_full_roundtrip` | AppConfig 完整 JSON 序列化往返 | 完整配置含 server/database/admin/time/proxy | 反序列化后各字段相等 |
| 2 | `s2_1_server_config_roundtrip` | ServerConfig JSON 序列化往返 | host=127.0.0.1, port=3000, jwt_hours=48 | 反序列化后各字段相等 |
| 3 | `s2_2_database_config_roundtrip` | DatabaseConfig JSON 序列化往返 | url=postgres://user:pass@localhost:5432/virs, pool_min=10, pool_max=100 | 反序列化后各字段相等 |
| 4 | `s2_4_admin_config_roundtrip_with_none_id` | AdminConfig (id=None) JSON 序列化往返 | username=admin, password=pass, id=None | 反序列化后 id==None |

### `crates/infra/virs-config/src/app_config_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `c3_1_parse_env_num_with_value` | 有值时解析环境变量数字 | Some("8080"), 默认"80" | 8080 |
| 2 | `c3_2_parse_env_num_none_uses_default` | None 时使用默认值 | None, 默认"80" | 80 |
| 3 | `c3_3_parse_env_num_invalid_returns_err` | 无效字符串返回错误 | Some("invalid"), 默认"80" | 返回错误 |
| 4 | `c3_4_parse_env_num_empty_string_returns_err` | 空字符串返回错误 | Some(""), 默认"80" | 返回错误 |
| 5 | `c3_5_parse_env_num_type_safety` | 多类型解析的类型安全 | "42"->u16/u32/u64, "-5"->i64 | 42, 42, 42, -5 |
| 6 | `s3_1_default_constants_values` | 默认常量值校验 | 无 | HOST="0.0.0.0", PORT="8080", JWT_HOURS="24", DB_POOL_MIN="5", DB_POOL_MAX="50", DB_ACQUIRE_TIMEOUT_SECS="10" 等 |
| 7 | `t12_1_time_config_default_values` | TimeConfig 默认值校验 | TimeConfig::default() | max_position=172800, pending=60, close=15, http=30, llm=120, retry=3/100, http=10/10, listenkey=1800 |
| 8 | `t12_2_time_config_default_constants` | TimeConfig 默认常量校验 | 无 | MAX_POSITION_DURATION="172800", PENDING_ORDER_TIMEOUT="60", CLOSE_ORDER_TIMEOUT="15", HTTP_TIMEOUT="30", LLM_TIMEOUT="120" |
| 9 | `t12_3_time_config_serde_roundtrip` | TimeConfig JSON 序列化往返 | 自定义各字段值 (max=3600, pending=30, close=20, http=60, llm=240 等) | 反序列化后相等 |
| 10 | `t12_4_time_config_clone_and_eq` | TimeConfig 克隆与相等性 | TimeConfig::default() | tc1 == tc2.clone() |
| 11 | `t12_5_time_config_max_position_duration_is_48h` | 最大持仓时长为 48 小时 | TimeConfig::default() | max_position_duration_secs / 3600 == 48 |

## 集成测试

### `crates/infra/virs-config/tests/integration_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `int_1_1_load_config_minimal_required` | 仅必需环境变量加载配置 | 设置 ENCRYPTION_KEY/LLM_KEY/DATABASE_URL/JWT_SECRET/ADMIN | 配置加载成功, 各字段匹配 |
| 2 | `int_1_2_load_config_missing_llm_key` | 缺少 LLM_KEY 返回错误 | 仅 ENCRYPTION_KEY/DATABASE_URL | 返回错误, 含 "LLM_KEY" |
| 3 | `int_1_3_load_config_missing_encryption_key` | 缺少 ENCRYPTION_KEY 返回错误 | 仅 LLM_KEY/DATABASE_URL | 返回错误, 含 "ENCRYPTION_KEY" |
| 4 | `int_1_4_load_config_missing_database_url` | 缺少 DATABASE_URL 返回错误 | ENCRYPTION_KEY/LLM_KEY/JWT_SECRET | 返回错误, 含 "DATABASE_URL" |
| 5 | `int_2_1_default_port` | 默认端口为 8080 | 仅必需环境变量 | server.port == 8080 |
| 6 | `int_2_2_default_host` | 默认主机为 0.0.0.0 | 仅必需环境变量 | server.host == "0.0.0.0" |
| 7 | `int_2_4_default_jwt_hours` | 默认 JWT 过期为 24 小时 | 仅必需环境变量 | jwt_expiration_hours == 24 |
| 8 | `int_2_5_default_db_pool` | 默认数据库连接池配置 | 仅必需环境变量 | pool_min==5, pool_max==50 |
| 9 | `int_5_1_custom_port` | 自定义端口生效 | PORT=3000 | server.port == 3000 |
| 10 | `int_5_2_custom_db_pool_max` | 自定义连接池上限生效 | DB_POOL_MAX=100 | pool_max == 100 |
| 11 | `int_5_4_custom_proxy` | 自定义代理生效 | PROXY_URL=http://proxy:8080 | proxy == Some("http://proxy:8080") |
| 12 | `int_6_1_same_encryption_and_llm_key_rejected` | 加密密钥与 LLM 密钥相同被拒绝 | ENCRYPTION_KEY == LLM_KEY == "same_key_value" | 返回错误, 含 "ENCRYPTION_KEY and LLM_KEY must be different" |
| 13 | `int_6_2_different_encryption_and_llm_key_accepted` | 加密密钥与 LLM 密钥不同被接受 | 两者不同 | 配置加载成功 |
