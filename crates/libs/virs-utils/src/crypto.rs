use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;

/// Encrypt a string using AES-256-GCM.
pub fn encrypt(plaintext: &str, key: &[u8; 32]) -> anyhow::Result<String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| anyhow::anyhow!("Cipher init error: {:?}", e))?;
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("Encryption error: {:?}", e))?;

    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);

    Ok(BASE64.encode(&combined))
}

/// Decrypt a string using AES-256-GCM.
pub fn decrypt(encoded: &str, key: &[u8; 32]) -> anyhow::Result<String> {
    let combined = BASE64
        .decode(encoded)
        .map_err(|e| anyhow::anyhow!("Base64 decode error: {}", e))?;
    if combined.len() < 12 {
        anyhow::bail!("Invalid ciphertext length");
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| anyhow::anyhow!("Cipher init error: {:?}", e))?;
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption error: {:?}", e))?;
    Ok(String::from_utf8(plaintext)?)
}

/// Derive a 32-byte key from a secret string.
pub fn derive_key(secret: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}
