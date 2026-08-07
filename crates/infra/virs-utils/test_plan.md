# virs-utils - 测试计划

**测试总数:** 29（单元测试: 21，集成测试: 8）

---

## 单元测试

### `crates/infra/virs-utils/src/crypto_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `c1_1_deterministic` | 相同密钥派生确定性 | secret="my_secret"（两次调用 derive_key） | 两次派生结果相等 |
| 2 | `c1_2_different_inputs_different_outputs` | 不同密钥派生不同结果 | secret_a vs secret_b | 两个派生结果不相等 |
| 3 | `c1_3_output_length_32` | 派生密钥长度为 32 | secret="any_secret" | key.len()=32 |
| 4 | `c2_1_normal_roundtrip` | 正常加解密往返 | key=derive_key("test_secret"), plaintext="Hello, World!" | 解密后等于原文 |
| 5 | `c2_2_empty_string_roundtrip` | 空字符串加解密往返 | key=derive_key("test_secret"), plaintext="" | 解密后等于空字符串 |
| 6 | `c2_3_wrong_key_decrypt_fails` | 错误密钥解密失败 | key1=secret_a 加密, key2=secret_b 解密 | 解密返回 Err |
| 7 | `c2_4_invalid_base64_fails` | 非法 base64 解密失败 | 密文="!!!not_base64!!!" | 解密返回 Err |
| 8 | `c2_5_short_ciphertext_fails` | 过短密文解密失败 | 密文=base64 编码的 "hello" | 解密返回 Err |
| 9 | `c2_6_same_plaintext_different_ciphertext` | 相同明文生成不同密文 | 相同 plaintext="same text" 加密两次 | 两次密文不相等, 但均能解密为原文 |
| 10 | `c3_1_hash_then_verify_correct` | 密码哈希后正确验证 | password="my_secure_password" | hash_password 后 verify_password 返回 true |
| 11 | `c3_2_verify_wrong_password` | 错误密码验证失败 | 哈希来自 "correct_password", 验证 "wrong_password" | verify_password 返回 false |
| 12 | `c3_3_empty_password_hash_ok` | 空密码哈希验证 | password="" | hash_password 成功, verify_password 返回 true |
| 13 | `c3_4_same_password_different_hash` | 相同密码生成不同哈希 | password="same_password" 哈希两次 | 两次哈希不相等, 但均能验证通过 |
| 14 | `c4_1_normal_roundtrip` | encrypt_with_key 正常往返 | secret="my_encryption_secret", plaintext="sensitive API key data" | decrypt_with_key 后等于原文 |
| 15 | `c4_2_wrong_secret_decrypt_fails` | 错误密钥解密失败 | 用 "secret_a" 加密, 用 "secret_b" 解密 | 解密返回 Err |

### `crates/infra/virs-utils/src/auth_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `a1_1_normal_construction` | Claims 正常构造 | sub="user-123", username="admin", role="admin", expiry=3600 | sub/username/role 正确, iat>0, exp=iat+3600 |
| 2 | `a1_2_zero_expiry` | Claims 零过期时间 | sub="user-456", username="trader", role="user", expiry=0 | exp=iat |
| 3 | `a4_1_normal_roundtrip` | JWT 编解码往返 | claims: sub=user-789, secret="my_secret_key" | 解码后 sub/username/role/exp 与原 claims 相等, iat>=原 iat |
| 4 | `a4_2_wrong_secret_decode_fails` | 错误密钥解码失败 | 用 "secret_a" 编码, 用 "secret_b" 解码 | 解码返回 Err |
| 5 | `a4_3_invalid_token_decode_fails` | 非法 token 解码失败 | token="not.a.valid.token" | 解码返回 Err |
| 6 | `a4_4_empty_token_decode_fails` | 空 token 解码失败 | token="" | 解码返回 Err |

## 集成测试

### `crates/infra/virs-utils/tests/integration_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `int_1_1_derive_key_encrypt_decrypt_chain` | 密钥派生加解密链路 | secret="integration_test_secret", plaintext="sensitive API key: sk-1234567890abcdef" | 解密后等于原文 |
| 2 | `int_1_2_encrypt_with_key_decrypt_with_key_chain` | encrypt_with_key 加解密链路 | secret="another_secret", plaintext="api_secret_data" | decrypt_with_key 后等于原文 |
| 3 | `int_2_1_claims_new_encode_decode_chain` | Claims 构造编解码链路 | sub="user-int-1", username="trader", role="user", secret="jwt_secret" | 解码后 sub/username/role 与原 claims 相等 |
| 4 | `int_2_2_expired_claims_lifecycle` | 过期 Claims 生命周期 | claims.exp 设为当前时间减 100 秒 | 解码返回 Err |
| 5 | `int_3_1_hash_then_verify_correct_password` | 密码哈希正确验证链路 | password="my_super_secret_password_123" | verify_password 返回 true |
| 6 | `int_3_2_hash_then_verify_wrong_password` | 密码哈希错误验证链路 | 哈希来自 "correct_password", 验证 "wrong_password" | verify_password 返回 false |
| 7 | `int_4_1_tampered_ciphertext_decrypt_fails` | 篡改密文解密失败 | 加密后篡改第 6 个字符 | 解密返回 Err |
| 8 | `int_4_2_tampered_token_decode_fails` | 篡改 token 解码失败 | 编码后替换字符 'a' 为 'b'（或 'e' 为 'f'） | 解码返回 Err |
