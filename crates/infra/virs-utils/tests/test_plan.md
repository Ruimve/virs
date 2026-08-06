# virs-utils 测试用例文档

> 生成日期: 2026-07-01
> Crate: `crates/libs/virs-utils`
> 状态: **29 个测试全部通过** (21 单元 + 8 集成)

---

## 单元测试用例

### auth_tests.rs — JWT 编解码 (6)

| ID | 测试函数 | 描述 |
|----|---------|------|
| A1.1 | `a1_1_normal_construction` | Claims::new → sub/username/role 一致, exp = iat + 3600 |
| A1.2 | `a1_2_zero_expiry` | exp_secs=0 → exp == iat |
| A4.1 | `a4_1_normal_roundtrip` | encode_jwt → decode_jwt → 字段一致 (iat 被覆盖) |
| A4.2 | `a4_2_wrong_secret_decode_fails` | 不同 secret 解码 → Err |
| A4.3 | `a4_3_invalid_token_decode_fails` | 无效 token → Err |
| A4.4 | `a4_4_empty_token_decode_fails` | 空字符串 → Err |

### crypto_tests.rs — 加解密 (15)

| ID | 测试函数 | 描述 |
|----|---------|------|
| C1.1 | `c1_1_deterministic` | derive_key 确定性 |
| C1.2 | `c1_2_different_inputs_different_outputs` | 不同输入 → 不同输出 |
| C1.3 | `c1_3_output_length_32` | 输出长度 = 32 |
| C2.1 | `c2_1_normal_roundtrip` | encrypt → decrypt → 原文一致 |
| C2.2 | `c2_2_empty_string_roundtrip` | 空字符串加解密 |
| C2.3 | `c2_3_wrong_key_decrypt_fails` | 不同 key → Err |
| C2.4 | `c2_4_invalid_base64_fails` | 无效 base64 → Err |
| C2.5 | `c2_5_short_ciphertext_fails` | 密文 < 12 字节 → Err |
| C2.6 | `c2_6_same_plaintext_different_ciphertext` | 相同明文 → 不同密文 (随机 nonce) |
| C3.1 | `c3_1_hash_then_verify_correct` | hash → verify(正确) → true |
| C3.2 | `c3_2_verify_wrong_password` | verify(错误) → false |
| C3.3 | `c3_3_empty_password_hash_ok` | 空密码 → hash Ok |
| C3.4 | `c3_4_same_password_different_hash` | 相同密码 → 不同 hash (bcrypt salt) |
| C4.1 | `c4_1_normal_roundtrip` | encrypt_with_key → decrypt_with_key → 一致 |
| C4.2 | `c4_2_wrong_secret_decrypt_fails` | 不同 secret → Err |

---

## 集成测试用例

### integration_tests.rs (8)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-1.1 | `int_1_1_derive_key_encrypt_decrypt_chain` | secret → derive_key → encrypt → decrypt 链路 |
| INT-1.2 | `int_1_2_encrypt_with_key_decrypt_with_key_chain` | encrypt_with_key → decrypt_with_key 链路 |
| INT-2.1 | `int_2_1_claims_new_encode_decode_chain` | Claims::new → encode_jwt → decode_jwt 链路 |
| INT-2.2 | `int_2_2_expired_claims_lifecycle` | 过期 Claims → encode → decode → Err |
| INT-3.1 | `int_3_1_hash_then_verify_correct_password` | hash_password → verify_password(正确) → true |
| INT-3.2 | `int_3_2_hash_then_verify_wrong_password` | hash_password → verify_password(错误) → false |
| INT-4.1 | `int_4_1_tampered_ciphertext_decrypt_fails` | 篡改密文 → decrypt → Err |
| INT-4.2 | `int_4_2_tampered_token_decode_fails` | 篡改 token → decode_jwt → Err |

---

## 代码覆盖率

### 测试文件与模块映射

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/auth_tests.rs` | auth.rs (Claims, encode_jwt, decode_jwt) | 6 |
| `src/crypto_tests.rs` | crypto.rs (全部 7 个函数) | 15 |
| `tests/integration_tests.rs` | 跨模块链路 | 8 |
| **合计** | | **29** |

### 死代码清理记录

无死代码。全部 10 个公开函数均有业务代码调用：

| 函数 | 业务调用位置 |
|------|-------------|
| `Claims::new` | `virs-api/src/handlers/auth.rs` |
| `encode_jwt` | `virs-api/src/handlers/auth.rs` |
| `decode_jwt` | `virs-api/src/handlers/response.rs` |
| `encrypt` | `virs-utils/src/crypto.rs` (被 `encrypt_with_key` 内部调用) |
| `decrypt` | `virs-app/src/adapters/credential_store.rs` |
| `derive_key` | `virs-app/src/engine_manager.rs` (2 处) |
| `hash_password` | `virs-app/src/main.rs`, `virs-api/src/handlers/user.rs` |
| `verify_password` | `virs-api/src/handlers/auth.rs` |
| `encrypt_with_key` | `virs-api/src/handlers/credentials.rs` (3 处), `virs-api/src/handlers/ai_credentials.rs` |
| `decrypt_with_key` | `virs-app/src/engine_manager.rs` (3 处), `virs-api/src/handlers/ai.rs`, `virs-api/src/handlers/ai_credentials.rs` (3 处) |

### 历史清理记录

| 已删除项 | 位置 | 删除原因 |
|---------|------|---------|
| `Claims::is_expired` | auth.rs | 孤儿方法，`decode_jwt` 使用 `Validation::default()` 已拒绝过期 token |
| `Claims::is_active` | auth.rs | 孤儿方法，同上 |
