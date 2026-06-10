//! Exchange authentication and request signing.
//!
//! Following CCXT's design, each exchange has its own signing algorithm.
//! This module provides:
//! - `Signer` trait for exchange-specific signing
//! - `SignedRequest` struct for signed request result
//! - Helper functions for HMAC-SHA256 signing
//!
//! Exchange-specific signers are located in their respective adapter modules:
//! - `adapter/binance/mod.rs` — BinanceSigner
//! - `adapter/okx/mod.rs` — OkxSigner  (TODO: future support)
//! - `adapter/bybit/mod.rs` — BybitSigner (TODO: future support)

use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

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

    /// Sign a PUT request (defaults to same logic as POST).
    fn sign_put(
        &self,
        path: &str,
        body: &mut serde_json::Value,
    ) -> Result<SignedRequest, ExchangeError> {
        self.sign_post(path, body)
    }
}

// ============================================================
// Helper functions (public for use by exchange-specific signers)
// ============================================================

/// Compute HMAC-SHA256 and return hex-encoded signature.
pub fn hmac_sha256_hex(secret: &str, message: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Compute HMAC-SHA256 and return Base64-encoded signature.
pub fn hmac_sha256_base64(secret: &str, message: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    let result = mac.finalize();
    BASE64.encode(result.into_bytes())
}

/// Helper to create a header from static name and string value.
pub fn make_header(name: &'static str, value: &str) -> Result<HeaderValue, ExchangeError> {
    HeaderValue::from_str(value)
        .map_err(|e| ExchangeError::Internal(format!("Invalid header '{}': {}", name, e)))
}

/// Helper to insert a header into HeaderMap.
pub fn insert_header(headers: &mut HeaderMap, name: &'static str, value: &str) -> Result<(), ExchangeError> {
    headers.insert(
        HeaderName::from_static(name),
        make_header(name, value)?,
    );
    Ok(())
}
