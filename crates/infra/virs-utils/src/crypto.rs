use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;
use virs_error::{VirsError, VirsResult};

/*
 * AES-256-GCM 加密：生成随机 nonce 并与密文拼接后 Base64 编码。
 * nonce 拼接在前 12 字节，解密时需先分离 nonce 再解密。
 */
pub fn encrypt(plaintext: &str, key: &[u8; 32]) -> VirsResult<String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| VirsError::crypto(format!("Cipher init error: {:?}", e)))?;
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| VirsError::crypto(format!("Encryption error: {:?}", e)))?;

    /* 将 nonce（12字节）与密文拼接，统一编码为 Base64 */
    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);

    Ok(BASE64.encode(&combined))
}

/*
 * AES-256-GCM 解密：先 Base64 解码，再分离前 12 字节 nonce 和密文。
 * 密文长度不足 12 字节表示数据损坏。
 */
pub fn decrypt(encoded: &str, key: &[u8; 32]) -> VirsResult<String> {
    let combined = BASE64
        .decode(encoded)
        .map_err(|e| VirsError::crypto(format!("Base64 decode error: {}", e)))?;
    if combined.len() < 12 {
        return Err(VirsError::crypto("Invalid ciphertext length"));
    }

    /* 前 12 字节为 nonce，其余为密文 */
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| VirsError::crypto(format!("Cipher init error: {:?}", e)))?;
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| VirsError::crypto(format!("Decryption error: {:?}", e)))?;
    String::from_utf8(plaintext)
        .map_err(|e| VirsError::crypto(format!("UTF-8 decode error: {}", e)))
}

/* 从字符串派生 256 位密钥：使用 SHA-256 哈希，输出固定 32 字节 */
pub fn derive_key(secret: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/* 密码哈希：使用 bcrypt 默认成本因子 */
pub fn hash_password(password: &str) -> VirsResult<String> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|e| VirsError::crypto(format!("Hash error: {}", e)))
}

/* 密码验证：bcrypt 校验，失败时记录错误日志并返回 false（不 panic） */
pub fn verify_password(password: &str, hash: &str) -> bool {
    bcrypt::verify(password, hash).unwrap_or_else(|e| {
        tracing::error!(error = %e, "bcrypt verify error — hash may be corrupted or malformed");
        false
    })
}

/* 便捷加密：先从字符串派生密钥，再进行 AES-256-GCM 加密 */
pub fn encrypt_with_key(plaintext: &str, secret: &str) -> VirsResult<String> {
    let key = derive_key(secret);
    encrypt(plaintext, &key)
}

/* 便捷解密：先从字符串派生密钥，再进行 AES-256-GCM 解密 */
pub fn decrypt_with_key(encoded: &str, secret: &str) -> VirsResult<String> {
    let key = derive_key(secret);
    decrypt(encoded, &key)
}
