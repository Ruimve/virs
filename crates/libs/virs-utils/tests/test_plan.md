# virs-utils 测试用例文档

> 生成日期: 2026-06-30
> 最后修订: 2026-06-30 (孤儿代码清理)
> Crate: `crates/libs/virs-utils`
> 目标: 抽取密码哈希等幂等逻辑为函数，进行完整单元测试和集成测试覆盖。
> 状态: **29 个测试通过** (21 单元 + 8 集成)；孤儿方法已删除，业务代码已重构调用 utils 函数

---

## 1. Crate 结构概览

```
virs-utils/
├── src/
│   ├── lib.rs       # 模块入口
│   ├── auth.rs      # JWT 编解码: Claims, encode_jwt, decode_jwt
│   └── crypto.rs    # AES-256-GCM 加解密: encrypt, decrypt, derive_key
├── tests/
│   ├── test_plan.md # 本文档
│   └── integration_tests.rs
```

### 业务使用情况

| 函数 | 业务调用数 | 调用位置 |
|------|-----------|---------|
| `crypto::encrypt` | 0 | (业务代码已改用 `encrypt_with_key`) |
| `crypto::decrypt` | 1 | credential_store.rs(1) (使用预 derived key) |
| `crypto::derive_key` | 2 | engine_manager.rs(2) (为 `PgCredentialStore::new` 派生 key) |
| `crypto::encrypt_with_key` | 4 | credentials.rs(3), ai_credentials.rs(1) |
| `crypto::decrypt_with_key` | 7 | ai_credentials.rs(3), ai.rs(1), engine_manager.rs(3) |
| `crypto::hash_password` | 2 | user.rs(1), main.rs(1) |
| `crypto::verify_password` | 1 | auth.rs(1) |
| `auth::encode_jwt` | 1 | auth.rs(1) (登录生成 token) |
| `auth::decode_jwt` | 1 | response.rs(1) (`extract_user_id`) |
| `auth::Claims::new` | 1 | auth.rs(1) |
| `auth::Claims::is_expired` | 0 | **已删除** (孤儿方法，`decode_jwt` 已拒绝过期 token) |
| `auth::Claims::is_active` | 0 | **已删除** (孤儿方法) |

### 问题识别

1. ~~`auth` 模块在业务代码中零调用~~ → **已修复**: `auth.rs` 登录 handler 改用 `Claims::new` + `encode_jwt`；`response.rs` 改用 `decode_jwt`
2. ~~`bcrypt` 在 `virs-utils` 中未封装~~ → **已修复**: `hash_password`/`verify_password` 已封装，业务代码已重构调用
3. `Claims` 已添加 `PartialEq`
4. ~~`Claims` 缺少过期判断方法~~ → **已删除** `is_expired`/`is_active` (孤儿方法，`decode_jwt` 使用 `Validation::default()` 已拒绝过期 token)
5. `virs-api`/`virs-app` 的 `bcrypt`/`jsonwebtoken` 直接依赖已移除 (业务代码通过 `virs-utils` 间接使用)

---

## 2. 抽取的幂等函数

### 2.1 auth.rs — 方法

| # | 方法/函数 | 签名 | 说明 |
|---|---------|------|------|
| A1 | `Claims::new` | `(sub, username, role, exp_secs) -> Self` | 构造 Claims，iat 自动设为当前时间 (业务调用: `auth.rs`) |
| ~~A2~~ | ~~`Claims::is_expired`~~ | — | **已删除**: 孤儿方法 (`decode_jwt` 已拒绝过期 token) |
| ~~A3~~ | ~~`Claims::is_active`~~ | — | **已删除**: 孤儿方法 |

### 2.2 crypto.rs — 函数

| # | 函数 | 签名 | 说明 |
|---|------|------|------|
| C1 | `hash_password` | `(password: &str) -> anyhow::Result<String>` | bcrypt 哈希封装 (业务调用: `user.rs`, `main.rs`) |
| C2 | `verify_password` | `(password: &str, hash: &str) -> bool` | bcrypt 验证封装 (业务调用: `auth.rs`) |
| C3 | `encrypt_with_key` | `(plaintext, secret: &str) -> Result<String>` | derive_key + encrypt 组合 (业务调用: `credentials.rs`, `ai_credentials.rs`) |
| C4 | `decrypt_with_key` | `(encoded, secret: &str) -> Result<String>` | derive_key + decrypt 组合 (业务调用: `ai_credentials.rs`, `ai.rs`, `engine_manager.rs`) |

---

## 3. 单元测试用例

### 3.1 auth_tests.rs — JWT 编解码

#### TC-A1: Claims::new
- A1.1 正常构造 → sub/username/role 一致，iat > 0，exp = iat + exp_secs
- A1.2 exp_secs=0 → exp == iat

#### TC-A2/A3: Claims::is_expired / is_active
- **已删除**: 方法无业务消费者 (孤儿代码)，测试一并删除。

#### TC-A4: encode_jwt / decode_jwt 往返
- A4.1 正常往返 → Claims 字段一致 (iat 被覆盖)
- A4.2 不同 secret 解码 → Err
- A4.3 无效 token 解码 → Err
- A4.4 空字符串 token → Err

### 3.2 crypto_tests.rs — 加解密

#### TC-C1: derive_key
- C1.1 相同输入 → 相同输出 (确定性)
- C1.2 不同输入 → 不同输出
- C1.3 输出长度 = 32

#### TC-C2: encrypt / decrypt 往返
- C2.1 正常往返 → 原文一致
- C2.2 空字符串 → 正常加解密
- C2.3 不同 key 解密 → Err
- C2.4 无效 base64 → Err
- C2.5 过短密文 (< 12 字节) → Err
- C2.6 相同明文加密两次 → 密文不同 (随机 nonce)

#### TC-C3: hash_password / verify_password
- C3.1 哈希后验证 → true
- C3.2 错误密码验证 → false
- C33 空密码哈希 → Ok
- C3.4 相同密码哈希两次 → 哈希值不同 (bcrypt salt)

#### TC-C4: encrypt_with_key / decrypt_with_key
- C4.1 正常往返 → 原文一致
- C4.2 不同 secret 解密 → Err

---

## 4. 集成测试用例

### TC-INT-1: 完整加密链路
- INT-1.1 secret → derive_key → encrypt → decrypt → 原文一致
- INT-1.2 encrypt_with_key → decrypt_with_key → 原文一致

### TC-INT-2: JWT 生命周期
- INT-2.1 Claims::new → encode_jwt → decode_jwt → 字段一致
- INT-2.2 过期 Claims → encode → decode → Err (jsonwebtoken 默认拒绝过期 token)

### TC-INT-3: 密码哈希链路
- INT-3.1 hash_password → verify_password(正确) → true
- INT-3.2 hash_password → verify_password(错误) → false

### TC-INT-4: 错误处理
- INT-4.1 篡改密文 → decrypt → Err
- INT-4.2 篡改 token → decode_jwt → Err

---

## 5. 测试文件与模块映射

| 测试文件 | 被测模块 | 文档计划 | 实际实现 | 状态 |
|----------|----------|---------|---------|------|
| `src/auth_tests.rs` | auth.rs | 11 | 6 | ✅ (A2/A3 已删除) |
| `src/crypto_tests.rs` | crypto.rs | 14 | 15 | ✅ 超额完成 |
| `tests/integration_tests.rs` | 跨模块 | 8 | 8 | ✅ 完全匹配 (INT-2.x 断言已精简) |
| **合计** | | **33** | **29** | ✅ 全部通过 |

---

## 6. 重构需求

### 6.1 添加方法/函数
- `auth.rs`: `Claims` 添加 `new` 方法 + `PartialEq` (已删除 `is_expired`/`is_active` 孤儿方法)
- `crypto.rs`: 添加 `hash_password`, `verify_password`, `encrypt_with_key`, `decrypt_with_key`

### 6.2 业务代码重构 (消除孤儿代码)
- `virs-api/handlers/user.rs`: `bcrypt::hash` → `virs_utils::crypto::hash_password`
- `virs-app/main.rs`: `bcrypt::hash` → `virs_utils::crypto::hash_password`
- `virs-api/handlers/auth.rs`: `bcrypt::verify` → `virs_utils::crypto::verify_password`; `serde_json::json!` + `jsonwebtoken::encode` → `Claims::new` + `encode_jwt`
- `virs-api/handlers/response.rs`: `jsonwebtoken::decode::<serde_json::Value>` → `virs_utils::auth::decode_jwt`
- `virs-api/handlers/credentials.rs`: `derive_key` + `encrypt` → `encrypt_with_key` (3 处)
- `virs-api/handlers/ai_credentials.rs`: `derive_key` + `encrypt` → `encrypt_with_key` (1 处); `derive_key` + `decrypt` → `decrypt_with_key` (3 处)
- `virs-api/handlers/ai.rs`: `derive_key` + `decrypt` → `decrypt_with_key` (1 处)
- `virs-app/engine_manager.rs`: `derive_key` + `decrypt` → `decrypt_with_key` (3 处); 保留 `derive_key` (2 处，为 `PgCredentialStore::new` 派生 key)
- `virs-api/Cargo.toml` / `virs-app/Cargo.toml`: 移除直接 `bcrypt`/`jsonwebtoken` 依赖

### 6.3 `_tests` 文件模式
- 通过 `#[cfg(test)] mod xxx_tests;` 在 `lib.rs` 中引入

---

## 7. 测试用例与文档对比审查报告

### 7.1 数量对比

| 测试文件 | 文档计划 | 实际实现 | 差异 | 状态 |
|----------|---------|---------|------|------|
| `auth_tests.rs` | 11 | 6 | -5 | ✅ (A2/A3 孤儿方法已删除) |
| `crypto_tests.rs` | 14 | 15 | +1 | ✅ 超额完成 |
| `integration_tests.rs` | 8 | 8 | 0 | ✅ 完全匹配 (INT-2.x 断言精简) |
| **合计** | **33** | **29** | **-4** | ✅ 全部通过 |

### 7.2 文档中每个测试用例的实现状态

- **TC-A1** (auth_tests.rs): ✅ 2/2 实现 (Claims::new)
- **TC-A2/A3** (auth_tests.rs): ❌ 已删除 (孤儿方法 `is_expired`/`is_active`)
- **TC-A4** (auth_tests.rs): ✅ 4/4 实现 (encode_jwt/decode_jwt 往返)
- **TC-C1 ~ C4** (crypto_tests.rs): ✅ 15/14 实现 (C3 额外增加了相同密码不同哈希测试)
- **TC-INT-1 ~ INT-4** (integration_tests.rs): ✅ 8/8 实现 (INT-2.x 移除了 `is_expired`/`is_active` 断言)

### 7.3 业务逻辑使用验证

| 方法/函数 | 业务调用情况 | 说明 |
|---------|------------|------|
| `crypto::encrypt` | ✅ 0 处 (仅测试) | 业务代码已改用 `encrypt_with_key` |
| `crypto::decrypt` | ✅ 1 处调用 | credential_store.rs (使用预 derived key) |
| `crypto::derive_key` | ✅ 2 处调用 | engine_manager.rs (为 `PgCredentialStore::new` 派生 key) |
| `crypto::hash_password` | ✅ 2 处调用 | user.rs, main.rs (已重构) |
| `crypto::verify_password` | ✅ 1 处调用 | auth.rs (已重构) |
| `crypto::encrypt_with_key` | ✅ 4 处调用 | credentials.rs(3), ai_credentials.rs(1) (已重构) |
| `crypto::decrypt_with_key` | ✅ 7 处调用 | ai_credentials.rs(3), ai.rs(1), engine_manager.rs(3) (已重构) |
| `auth::encode_jwt` | ✅ 1 处调用 | auth.rs 登录 handler (已重构) |
| `auth::decode_jwt` | ✅ 1 处调用 | response.rs `extract_user_id` (已重构) |
| `auth::Claims` | ✅ 1 处调用 | auth.rs (已重构) |
| `auth::Claims::new` | ✅ 1 处调用 | auth.rs (已重构) |
| `Claims::is_expired` | ❌ 已删除 | 孤儿方法 (`decode_jwt` 已拒绝过期 token) |
| `Claims::is_active` | ❌ 已删除 | 孤儿方法 |

### 7.4 回归审查发现的问题

| # | 问题 | 类型 | 修复 |
|---|------|------|------|
| 1 | ~~`bcrypt` 在 `virs-utils` 中未封装~~ | 可维护性 | **已修复**: `hash_password`/`verify_password` 已封装，业务代码已重构调用 |
| 2 | ~~密码哈希逻辑散落在 3 处业务代码中~~ | 可维护性 | **已修复**: user.rs/main.rs/auth.rs 已改用 `virs_utils::crypto::hash_password`/`verify_password` |
| 3 | ~~`derive_key` + `encrypt`/`decrypt` 组合在 17 处重复~~ | 可维护性 | **已修复**: 业务代码已重构调用 `encrypt_with_key`/`decrypt_with_key` |
| 4 | `Claims` 缺少 `PartialEq` | 测试需求 | 添加 `#[derive(PartialEq)]` |
| 5 | ~~`Claims` 缺少构造方法和过期判断~~ | 可用性 | 新增 `new`; **已删除** `is_expired`/`is_active` (孤儿方法，`decode_jwt` 已拒绝过期 token) |
| 6 | ~~`auth` 模块在业务代码中零调用~~ | 死代码 | **已修复**: auth.rs 登录 handler 改用 `Claims::new`+`encode_jwt`; response.rs 改用 `decode_jwt` |
| 7 | JWT 过期 token 的 `decode_jwt` 返回 `Err` | 预期行为 | `jsonwebtoken` 默认 `Validation` 拒绝过期 token，测试已验证 |
| 8 | `virs-api`/`virs-app` 直接依赖 `bcrypt`/`jsonwebtoken` 但仅用于散落调用 | 可维护性 | **已修复**: 移除直接依赖，业务代码通过 `virs-utils` 间接使用 |
| 9 | 所有数值方法均含除零保护 | 健壮性 | `decrypt` 检查 `combined.len() < 12` |

### 7.5 孤儿代码检查

- 所有保留的 `pub fn` 方法/函数均有业务代码调用 (非仅测试引用)
- 已删除的孤儿方法: `Claims::is_expired`, `Claims::is_active` (因 `decode_jwt` 使用 `Validation::default()` 已拒绝过期 token，业务代码无法从 `decode_jwt` 获得过期 `Claims`)
- 业务代码已重构的方法调用位置:
  - `virs-api/handlers/user.rs`: `bcrypt::hash` → `hash_password`
  - `virs-app/main.rs`: `bcrypt::hash` → `hash_password`
  - `virs-api/handlers/auth.rs`: `bcrypt::verify` → `verify_password`; `serde_json::json!`+`jsonwebtoken::encode` → `Claims::new`+`encode_jwt`
  - `virs-api/handlers/response.rs`: `jsonwebtoken::decode::<serde_json::Value>` → `decode_jwt`
  - `virs-api/handlers/credentials.rs`: `derive_key`+`encrypt` → `encrypt_with_key` (3处)
  - `virs-api/handlers/ai_credentials.rs`: `derive_key`+`encrypt` → `encrypt_with_key` (1处); `derive_key`+`decrypt` → `decrypt_with_key` (3处)
  - `virs-api/handlers/ai.rs`: `derive_key`+`decrypt` → `decrypt_with_key` (1处)
  - `virs-app/engine_manager.rs`: `derive_key`+`decrypt` → `decrypt_with_key` (3处); 保留 `derive_key` (2处，为 `PgCredentialStore::new` 派生 key)
- `virs-api/Cargo.toml`/`virs-app/Cargo.toml` 已移除直接 `bcrypt`/`jsonwebtoken` 依赖
- Clippy 检查通过 (无新增警告)
- 整个工作区编译通过，29 个测试全部通过
