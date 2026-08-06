use crate::auth::{decode_jwt, encode_jwt, Claims};

#[test]
fn a1_1_normal_construction() {
    let claims = Claims::new("user-123", "admin", "admin", 3600);
    assert_eq!(claims.sub, "user-123");
    assert_eq!(claims.username, "admin");
    assert_eq!(claims.role, "admin");
    assert!(claims.iat > 0);
    assert_eq!(claims.exp, claims.iat + 3600);
}

#[test]
fn a1_2_zero_expiry() {
    let claims = Claims::new("user-456", "trader", "user", 0);
    assert_eq!(claims.exp, claims.iat);
}

#[test]
fn a4_1_normal_roundtrip() {
    let claims = Claims::new("user-789", "trader", "user", 3600);
    let secret = "my_secret_key";
    let token = encode_jwt(&claims, secret).unwrap();
    let decoded = decode_jwt(&token, secret).unwrap();

    assert_eq!(decoded.sub, claims.sub);
    assert_eq!(decoded.username, claims.username);
    assert_eq!(decoded.role, claims.role);
    assert_eq!(decoded.exp, claims.exp);

    assert!(decoded.iat >= claims.iat);
}

#[test]
fn a4_2_wrong_secret_decode_fails() {
    let claims = Claims::new("user-1", "test", "user", 3600);
    let token = encode_jwt(&claims, "secret_a").unwrap();
    assert!(decode_jwt(&token, "secret_b").is_err());
}

#[test]
fn a4_3_invalid_token_decode_fails() {
    assert!(decode_jwt("not.a.valid.token", "secret").is_err());
}

#[test]
fn a4_4_empty_token_decode_fails() {
    assert!(decode_jwt("", "secret").is_err());
}
