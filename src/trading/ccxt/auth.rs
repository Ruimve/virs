//! Exchange authentication and request signing.
//!
//! Following CCXT's design, each exchange has its own signing algorithm.
//! This module provides:
//! - `Signer` trait for exchange-specific signing
//! - Binance HMAC-SHA256 signer
//! - OKX HMAC-SHA256 + Base64 signer
//! - Bybit HMAC-SHA256 signer
//!
//! All signers produce headers and/or query parameters that are attached
//! to authenticated requests.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use chrono::Utc;

use super::errors::ExchangeError;

type HmacSha256 = Hmac<Sha256>;

/// Result of signing a request.
#[derive(Debug, Clone)]
pub struct SignedRequest {
    /// Additional headers to include.
    pub headers: HeaderMap,
    /// Query parameters to append (for exchanges that sign via query string).
    pub query_params: Vec<(String, String)>,
    /// Modified body (for exchanges that sign via body).
    pub body: Option<serde_json::Value>,
}

impl Default for SignedRequest {
    fn default() -> Self {
        Self {
            headers: HeaderMap::new(),
            query_params: Vec::new(),
            body: None,
        }
    }
}

/// Trait for exchange-specific request signing.
pub trait Signer: Send + Sync {
    /// Sign a GET request.
    fn sign_get(
        &self,
        path: &str,
        query_params: &mut Vec<(String, String)>,
    ) -> Result<SignedRequest, ExchangeError>;

    /// Sign a POST request.
    fn sign_post(
        &self,
        path: &str,
        body: &mut serde_json::Value,
    ) -> Result<SignedRequest, ExchangeError>;
}

// ============================================================
// Binance Signer (HMAC-SHA256 via query string)
// ============================================================

pub struct BinanceSigner {
    api_key: String,
    api_secret: String,
}

impl BinanceSigner {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self { api_key, api_secret }
    }
}

impl Signer for BinanceSigner {
    fn sign_get(
        &self,
        _path: &str,
        query_params: &mut Vec<(String, String)>,
    ) -> Result<SignedRequest, ExchangeError> {
        let timestamp = Utc::now().timestamp_millis();
        query_params.push(("timestamp".into(), timestamp.to_string()));

        let query_string = query_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let signature = hmac_sha256_hex(&self.api_secret, &query_string);
        query_params.push(("signature".into(), signature));

        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-mbx-apikey"),
            HeaderValue::from_str(&self.api_key)
                .map_err(|e| ExchangeError::Internal(format!("Invalid API key header: {}", e)))?,
        );

        Ok(SignedRequest {
            headers,
            query_params: query_params.clone(),
            body: None,
        })
    }

    fn sign_post(
        &self,
        _path: &str,
        body: &mut serde_json::Value,
    ) -> Result<SignedRequest, ExchangeError> {
        // Binance signs POST via query string even for JSON body
        let mut query_params = vec![(
            "timestamp".into(),
            Utc::now().timestamp_millis().to_string(),
        )];

        // For application/x-www-form-urlencoded POST
        let form_body = if body.is_object() {
            let mut pairs: Vec<(String, String)> = body
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| {
                    let val = if v.is_string() {
                        v.as_str().unwrap().to_string()
                    } else {
                        v.to_string()
                    };
                    (k.clone(), val)
                })
                .collect();
            pairs.push(("timestamp".into(), query_params[0].1.clone()));

            let query_string = pairs
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");

            let signature = hmac_sha256_hex(&self.api_secret, &query_string);
            pairs.push(("signature".into(), signature));

            query_params = pairs;

            // Return form body
            Some(serde_json::Value::String(query_string))
        } else {
            None
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-mbx-apikey"),
            HeaderValue::from_str(&self.api_key)
                .map_err(|e| ExchangeError::Internal(format!("Invalid API key header: {}", e)))?,
        );

        Ok(SignedRequest {
            headers,
            query_params,
            body: form_body,
        })
    }
}

// ============================================================
// OKX Signer (HMAC-SHA256 + Base64, timestamp + passphrase)
// ============================================================

pub struct OkxSigner {
    api_key: String,
    api_secret: String,
    passphrase: String,
}

impl OkxSigner {
    pub fn new(api_key: String, api_secret: String, passphrase: String) -> Self {
        Self { api_key, api_secret, passphrase }
    }
}

impl Signer for OkxSigner {
    fn sign_get(
        &self,
        path: &str,
        query_params: &mut Vec<(String, String)>,
    ) -> Result<SignedRequest, ExchangeError> {
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        let query_string = query_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let sign_str = format!("{}GET{}{}", timestamp, path, query_string);
        let signature = hmac_sha256_base64(&self.api_secret, &sign_str);

        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("ok-access-key"),
            HeaderValue::from_str(&self.api_key)
                .map_err(|e| ExchangeError::Internal(format!("Invalid API key: {}", e)))?,
        );
        headers.insert(
            HeaderName::from_static("ok-access-sign"),
            HeaderValue::from_str(&signature)
                .map_err(|e| ExchangeError::Internal(format!("Invalid signature: {}", e)))?,
        );
        headers.insert(
            HeaderName::from_static("ok-access-timestamp"),
            HeaderValue::from_str(&timestamp)
                .map_err(|e| ExchangeError::Internal(format!("Invalid timestamp: {}", e)))?,
        );
        headers.insert(
            HeaderName::from_static("ok-access-passphrase"),
            HeaderValue::from_str(&self.passphrase)
                .map_err(|e| ExchangeError::Internal(format!("Invalid passphrase: {}", e)))?,
        );

        Ok(SignedRequest {
            headers,
            query_params: query_params.clone(),
            body: None,
        })
    }

    fn sign_post(
        &self,
        path: &str,
        body: &mut serde_json::Value,
    ) -> Result<SignedRequest, ExchangeError> {
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let body_str = serde_json::to_string(body)
            .map_err(|e| ExchangeError::Internal(format!("Failed to serialize body: {}", e)))?;

        let sign_str = format!("{}POST{}{}", timestamp, path, body_str);
        let signature = hmac_sha256_base64(&self.api_secret, &sign_str);

        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("ok-access-key"),
            HeaderValue::from_str(&self.api_key)
                .map_err(|e| ExchangeError::Internal(format!("Invalid API key: {}", e)))?,
        );
        headers.insert(
            HeaderName::from_static("ok-access-sign"),
            HeaderValue::from_str(&signature)
                .map_err(|e| ExchangeError::Internal(format!("Invalid signature: {}", e)))?,
        );
        headers.insert(
            HeaderName::from_static("ok-access-timestamp"),
            HeaderValue::from_str(&timestamp)
                .map_err(|e| ExchangeError::Internal(format!("Invalid timestamp: {}", e)))?,
        );
        headers.insert(
            HeaderName::from_static("ok-access-passphrase"),
            HeaderValue::from_str(&self.passphrase)
                .map_err(|e| ExchangeError::Internal(format!("Invalid passphrase: {}", e)))?,
        );
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        );

        Ok(SignedRequest {
            headers,
            query_params: Vec::new(),
            body: Some(body.clone()),
        })
    }
}

// ============================================================
// Bybit Signer (HMAC-SHA256, similar to Binance but with recv_window)
// ============================================================

pub struct BybitSigner {
    api_key: String,
    api_secret: String,
}

impl BybitSigner {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self { api_key, api_secret }
    }
}

impl Signer for BybitSigner {
    fn sign_get(
        &self,
        path: &str,
        query_params: &mut Vec<(String, String)>,
    ) -> Result<SignedRequest, ExchangeError> {
        let timestamp = Utc::now().timestamp_millis();
        query_params.push(("timestamp".into(), timestamp.to_string()));
        query_params.push(("recv_window".into(), "5000".into()));

        let query_string = query_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let pre_sign = format!("{}GET{}{}", timestamp, path, query_string);
        let signature = hmac_sha256_hex(&self.api_secret, &pre_sign);
        query_params.push(("sign".into(), signature));

        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-api-key"),
            HeaderValue::from_str(&self.api_key)
                .map_err(|e| ExchangeError::Internal(format!("Invalid API key: {}", e)))?,
        );

        Ok(SignedRequest {
            headers,
            query_params: query_params.clone(),
            body: None,
        })
    }

    fn sign_post(
        &self,
        path: &str,
        body: &mut serde_json::Value,
    ) -> Result<SignedRequest, ExchangeError> {
        let timestamp = Utc::now().timestamp_millis();
        let body_str = serde_json::to_string(body)
            .map_err(|e| ExchangeError::Internal(format!("Failed to serialize body: {}", e)))?;

        let pre_sign = format!("{}POST{}{}", timestamp, path, body_str);
        let signature = hmac_sha256_hex(&self.api_secret, &pre_sign);

        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-api-key"),
            HeaderValue::from_str(&self.api_key)
                .map_err(|e| ExchangeError::Internal(format!("Invalid API key: {}", e)))?,
        );
        headers.insert(
            HeaderName::from_static("x-timestamp"),
            HeaderValue::from_str(&timestamp.to_string())
                .map_err(|e| ExchangeError::Internal(format!("Invalid timestamp: {}", e)))?,
        );
        headers.insert(
            HeaderName::from_static("x-sign"),
            HeaderValue::from_str(&signature)
                .map_err(|e| ExchangeError::Internal(format!("Invalid signature: {}", e)))?,
        );
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        );

        Ok(SignedRequest {
            headers,
            query_params: Vec::new(),
            body: Some(body.clone()),
        })
    }
}

// ============================================================
// Helper functions
// ============================================================

/// Compute HMAC-SHA256 and return hex-encoded signature.
fn hmac_sha256_hex(secret: &str, message: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Compute HMAC-SHA256 and return Base64-encoded signature.
fn hmac_sha256_base64(secret: &str, message: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    let result = mac.finalize();
    BASE64.encode(result.into_bytes())
}
