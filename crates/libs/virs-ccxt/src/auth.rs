use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use sha2::Sha256;

use virs_error::ExchangeError;

// HMAC-SHA256 类型别名
type HmacSha256 = Hmac<Sha256>;

// 签名后的请求，包含 headers(x-mbx-apikey)、query_params(含timestamp/recvWindow/signature)、body(POST时为form-urlencoded字符串)
#[derive(Debug, Clone)]
pub struct SignedRequest {
    pub headers: HeaderMap,

    pub query_params: Vec<(String, String)>,

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

// 交易所签名接口，sign_get用于GET/DELETE，sign_post用于POST/PUT
pub trait Signer: Send + Sync {
    // 签名 GET/DELETE 请求，在 query_params 中追加签名参数
    fn sign_get(
        &self,
        path: &str,
        query_params: &mut Vec<(String, String)>,
    ) -> Result<SignedRequest, ExchangeError>;

    // 签名 POST/PUT 请求，对 body 进行签名
    fn sign_post(
        &self,
        path: &str,
        body: &mut serde_json::Value,
    ) -> Result<SignedRequest, ExchangeError>;

    // 签名 PUT 请求，默认委托给 sign_post
    fn sign_put(
        &self,
        path: &str,
        body: &mut serde_json::Value,
    ) -> Result<SignedRequest, ExchangeError> {
        self.sign_post(path, body)
    }

    // 设置服务器时间偏移(毫秒)，用于时钟同步
    fn set_time_offset(&self, _offset_ms: i64) {}

    // 获取当前时间偏移(毫秒)
    fn get_time_offset(&self) -> i64 {
        0
    }
}

// HMAC-SHA256签名，输出十六进制字符串
pub fn hmac_sha256_hex(secret: &str, message: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes()) // 输出十六进制签名
}

// 构造 HTTP HeaderValue，校验值合法性
pub fn make_header(name: &'static str, value: &str) -> Result<HeaderValue, ExchangeError> {
    HeaderValue::from_str(value)
        .map_err(|e| ExchangeError::Internal(format!("Invalid header '{}': {}", name, e)))
}

// 向 HeaderMap 插入 header 的便捷工具
pub fn insert_header(
    headers: &mut HeaderMap,
    name: &'static str,
    value: &str,
) -> Result<(), ExchangeError> {
    headers.insert(HeaderName::from_static(name), make_header(name, value)?);
    Ok(())
}
