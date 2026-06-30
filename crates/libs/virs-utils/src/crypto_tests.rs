//! Unit tests for crypto.rs functions.

use base64::Engine as _;

use crate::crypto::{
    decrypt, decrypt_with_key, derive_key, encrypt, encrypt_with_key, hash_password,
    verify_password,
};

// ============================================================
// TC-C1: derive_key
// ============================================================

#[test]
fn c1_1_deterministic() {
    let key1 = derive_key("my_secret");
    let key2 = derive_key("my_secret");
    assert_eq!(key1, key2);
}

#[test]
fn c1_2_different_inputs_different_outputs() {
    let key1 = derive_key("secret_a");
    let key2 = derive_key("secret_b");
    assert_ne!(key1, key2);
}

#[test]
fn c1_3_output_length_32() {
    let key = derive_key("any_secret");
    assert_eq!(key.len(), 32);
}

// ============================================================
// TC-C2: encrypt / decrypt round-trip
// ============================================================

#[test]
fn c2_1_normal_roundtrip() {
    let key = derive_key("test_secret");
    let plaintext = "Hello, World!";
    let encrypted = encrypt(plaintext, &key).unwrap();
    let decrypted = decrypt(&encrypted, &key).unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn c2_2_empty_string_roundtrip() {
    let key = derive_key("test_secret");
    let encrypted = encrypt("", &key).unwrap();
    let decrypted = decrypt(&encrypted, &key).unwrap();
    assert_eq!(decrypted, "");
}

#[test]
fn c2_3_wrong_key_decrypt_fails() {
    let key1 = derive_key("secret_a");
    let key2 = derive_key("secret_b");
    let encrypted = encrypt("secret data", &key1).unwrap();
    assert!(decrypt(&encrypted, &key2).is_err());
}

#[test]
fn c2_4_invalid_base64_fails() {
    let key = derive_key("test_secret");
    assert!(decrypt("!!!not_base64!!!", &key).is_err());
}

#[test]
fn c2_5_short_ciphertext_fails() {
    let key = derive_key("test_secret");
    // Base64 of 5 bytes — less than 12 byte nonce
    let short = base64::engine::general_purpose::STANDARD.encode(b"hello");
    assert!(decrypt(&short, &key).is_err());
}

#[test]
fn c2_6_same_plaintext_different_ciphertext() {
    let key = derive_key("test_secret");
    let plaintext = "same text";
    let enc1 = encrypt(plaintext, &key).unwrap();
    let enc2 = encrypt(plaintext, &key).unwrap();
    // Random nonce ensures different ciphertext
    assert_ne!(enc1, enc2);
    // But both decrypt to the same plaintext
    assert_eq!(decrypt(&enc1, &key).unwrap(), plaintext);
    assert_eq!(decrypt(&enc2, &key).unwrap(), plaintext);
}

// ============================================================
// TC-C3: hash_password / verify_password
// ============================================================

#[test]
fn c3_1_hash_then_verify_correct() {
    let password = "my_secure_password";
    let hash = hash_password(password).unwrap();
    assert!(verify_password(password, &hash));
}

#[test]
fn c3_2_verify_wrong_password() {
    let hash = hash_password("correct_password").unwrap();
    assert!(!verify_password("wrong_password", &hash));
}

#[test]
fn c3_3_empty_password_hash_ok() {
    let hash = hash_password("").unwrap();
    assert!(verify_password("", &hash));
}

#[test]
fn c3_4_same_password_different_hash() {
    let password = "same_password";
    let hash1 = hash_password(password).unwrap();
    let hash2 = hash_password(password).unwrap();
    // bcrypt salt ensures different hashes
    assert_ne!(hash1, hash2);
    // But both verify correctly
    assert!(verify_password(password, &hash1));
    assert!(verify_password(password, &hash2));
}

// ============================================================
// TC-C4: encrypt_with_key / decrypt_with_key
// ============================================================

#[test]
fn c4_1_normal_roundtrip() {
    let secret = "my_encryption_secret";
    let plaintext = "sensitive API key data";
    let encrypted = encrypt_with_key(plaintext, secret).unwrap();
    let decrypted = decrypt_with_key(&encrypted, secret).unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn c4_2_wrong_secret_decrypt_fails() {
    let plaintext = "secret data";
    let encrypted = encrypt_with_key(plaintext, "secret_a").unwrap();
    assert!(decrypt_with_key(&encrypted, "secret_b").is_err());
}
