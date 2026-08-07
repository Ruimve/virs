use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use sha2::Sha256;

use virs_error::ExchangeError;


type HmacSha256 = Hmac<Sha256>;


#[derive(Debug, Clone)]
pub struct SignedRequest {
    pub headers: HeaderMap,

    pub query_params: Vec<(String, String)>,

    pub body: Option<serde_json::Value>,
}


pub trait Signer: Send + Sync {

    fn sign_get(
        &self,
        path: &str,
        query_params: &mut Vec<(String, String)>,
    ) -> Result<SignedRequest, ExchangeError>;


    fn sign_post(
        &self,
        path: &str,
        body: &mut serde_json::Value,
    ) -> Result<SignedRequest, ExchangeError>;


    fn sign_put(
        &self,
        path: &str,
        body: &mut serde_json::Value,
    ) -> Result<SignedRequest, ExchangeError> {
        self.sign_post(path, body)
    }


    fn set_time_offset(&self, _offset_ms: i64) {}


    fn get_time_offset(&self) -> i64 {
        0
    }
}


pub fn hmac_sha256_hex(secret: &str, message: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}


pub fn make_header(name: &'static str, value: &str) -> Result<HeaderValue, ExchangeError> {
    HeaderValue::from_str(value)
        .map_err(|e| ExchangeError::Internal(format!("Invalid header '{}': {}", name, e)))
}


pub fn insert_header(
    headers: &mut HeaderMap,
    name: &'static str,
    value: &str,
) -> Result<(), ExchangeError> {
    headers.insert(HeaderName::from_static(name), make_header(name, value)?);
    Ok(())
}
