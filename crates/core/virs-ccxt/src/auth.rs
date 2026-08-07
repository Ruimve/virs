use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use sha2::Sha256;

use virs_error::ExchangeError;


/* HMAC-SHA256 类型别名，用于币安 API 请求签名 */
type HmacSha256 = Hmac<Sha256>;


#[derive(Debug, Clone)]
pub struct SignedRequest {
    pub headers: HeaderMap,

    pub query_params: Vec<(String, String)>,

    pub body: Option<serde_json::Value>,
}


/* 签名器 trait 抽象，支持 HMAC-SHA256 和 Ed25519 两种签名方式，实现多态切换 */
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


    /* 设置本地时间与交易所服务器时间的偏移量，用于签名时校正 timestamp */
    fn set_time_offset(&self, _offset_ms: i64) {}


    fn get_time_offset(&self) -> i64 {
        0
    }
}


/* HMAC-SHA256 签名核心函数：用 api_secret 对请求参数生成十六进制签名，币安用于验证请求合法性 */
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
