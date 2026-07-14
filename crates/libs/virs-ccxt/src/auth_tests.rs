use reqwest::header::{HeaderMap, HeaderValue};

use crate::auth::{hmac_sha256_hex, insert_header, make_header};


#[test]
fn a1_1_hmac_sha256_hex_known_vector() {


    let key = "NhqPtmdSJYdKjVHjA7PZj4Mge3R5YNiP1e3UZjInClVN65XAbvqqM6A7H5fATj0j";
    let msg = "symbol=LTCBTC&side=BUY&type=LIMIT&timeInForce=GTC&quantity=1&price=0.1&recvWindow=5000&timestamp=1499827319559";
    let sig = hmac_sha256_hex(key, msg);
    assert_eq!(
        sig,
        "c8db56825ae71d6d79447849e617115f4a920fa2acdcab2b053c4b2838bd6b71"
    );
}

#[test]
fn a1_2_hmac_sha256_hex_empty_message() {
    let sig = hmac_sha256_hex("secret", "");

    assert_eq!(sig.len(), 64);
    assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn a1_3_hmac_sha256_hex_empty_key() {
    let sig = hmac_sha256_hex("", "message");
    assert_eq!(sig.len(), 64);
    assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn a1_4_hmac_sha256_hex_idempotent() {
    let key = "test_key";
    let msg = "test_message";
    let sig1 = hmac_sha256_hex(key, msg);
    let sig2 = hmac_sha256_hex(key, msg);
    assert_eq!(sig1, sig2);
}

#[test]
fn a1_5_hmac_sha256_hex_different_inputs() {
    let sig1 = hmac_sha256_hex("key1", "msg");
    let sig2 = hmac_sha256_hex("key2", "msg");
    assert_ne!(sig1, sig2);
}


#[test]
fn a3_1_make_header_valid_ascii() {
    let result = make_header("x-mbx-apikey", "abcdef123456");
    assert!(result.is_ok());
    let val = result.unwrap();
    assert_eq!(val, HeaderValue::from_static("abcdef123456"));
}

#[test]
fn a3_2_make_header_invalid_chars() {

    let result = make_header("x-custom", "bad\nvalue");
    assert!(result.is_err());
}


#[test]
fn a4_1_insert_header_success() {
    let mut headers = HeaderMap::new();
    let result = insert_header(&mut headers, "x-mbx-apikey", "test_key");
    assert!(result.is_ok());
    assert_eq!(headers.get("x-mbx-apikey").unwrap(), "test_key");
}

#[test]
fn a4_2_insert_header_invalid_value() {
    let mut headers = HeaderMap::new();
    let result = insert_header(&mut headers, "x-custom", "bad\r\nvalue");
    assert!(result.is_err());

    assert!(headers.get("x-custom").is_none());
}
