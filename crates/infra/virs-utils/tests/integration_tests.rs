use virs_utils::auth::{decode_jwt, encode_jwt, Claims};
use virs_utils::crypto::{
    decrypt, decrypt_with_key, derive_key, encrypt, encrypt_with_key, hash_password,
    verify_password,
};

#[test]
fn int_1_1_derive_key_encrypt_decrypt_chain() {
    let secret = "integration_test_secret";
    let plaintext = "sensitive API key: sk-1234567890abcdef";

    let key = derive_key(secret);
    let encrypted = encrypt(plaintext, &key).unwrap();
    let decrypted = decrypt(&encrypted, &key).unwrap();

    assert_eq!(decrypted, plaintext);
}

#[test]
fn int_1_2_encrypt_with_key_decrypt_with_key_chain() {
    let secret = "another_secret";
    let plaintext = "api_secret_data";

    let encrypted = encrypt_with_key(plaintext, secret).unwrap();
    let decrypted = decrypt_with_key(&encrypted, secret).unwrap();

    assert_eq!(decrypted, plaintext);
}

#[test]
fn int_2_1_claims_new_encode_decode_chain() {
    let claims = Claims::new("user-int-1", "trader", "user", 3600);
    let secret = "jwt_secret";

    let token = encode_jwt(&claims, secret).unwrap();
    let decoded = decode_jwt(&token, secret).unwrap();

    assert_eq!(decoded.sub, "user-int-1");
    assert_eq!(decoded.username, "trader");
    assert_eq!(decoded.role, "user");
}

#[test]
fn int_2_2_expired_claims_lifecycle() {
    let mut claims = Claims::new("user-int-2", "admin", "admin", 3600);

    claims.exp = chrono::Utc::now().timestamp() - 100;

    let token = encode_jwt(&claims, "secret").unwrap();
    assert!(decode_jwt(&token, "secret").is_err());
}

#[test]
fn int_3_1_hash_then_verify_correct_password() {
    let password = "my_super_secret_password_123";
    let hash = hash_password(password).unwrap();
    assert!(verify_password(password, &hash));
}

#[test]
fn int_3_2_hash_then_verify_wrong_password() {
    let hash = hash_password("correct_password").unwrap();
    assert!(!verify_password("wrong_password", &hash));
}

#[test]
fn int_4_1_tampered_ciphertext_decrypt_fails() {
    let key = derive_key("test_secret");
    let encrypted = encrypt("secret data", &key).unwrap();

    let mut tampered = encrypted.chars().collect::<Vec<_>>();
    if tampered.len() > 5 {
        tampered[5] = if tampered[5] == 'A' { 'B' } else { 'A' };
    }
    let tampered_str: String = tampered.into_iter().collect();

    assert!(decrypt(&tampered_str, &key).is_err());
}

#[test]
fn int_4_2_tampered_token_decode_fails() {
    let claims = Claims::new("user-1", "test", "user", 3600);
    let token = encode_jwt(&claims, "secret").unwrap();

    let tampered = token.replace('a', "b");

    let tampered = if tampered == token {
        token.replace('e', "f")
    } else {
        tampered
    };

    assert!(decode_jwt(&tampered, "secret").is_err());
}
