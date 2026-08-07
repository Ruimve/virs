use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use virs_error::{VirsError, VirsResult};

/* JWT Claims 结构体：包含用户标识、用户名、角色和过期时间 */
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
}

impl Claims {
    /* 创建 Claims，exp 由当前时间加上有效期秒数计算 */
    pub fn new(sub: &str, username: &str, role: &str, exp_secs: i64) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            sub: sub.into(),
            username: username.into(),
            role: role.into(),
            iat: now,
            exp: now + exp_secs,
        }
    }
}

/* 编码 JWT：使用密钥对 Claims 进行签名，iat 更新为当前时间 */
pub fn encode_jwt(claims: &Claims, secret: &str) -> VirsResult<String> {
    let mut claims = claims.clone();
    claims.iat = chrono::Utc::now().timestamp();
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| VirsError::auth(format!("JWT encode error: {}", e)))?;
    Ok(token)
}

/* 解码 JWT：验证签名并提取 Claims，验证失败返回认证错误 */
pub fn decode_jwt(token: &str, secret: &str) -> VirsResult<Claims> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| VirsError::auth(format!("JWT decode error: {}", e)))?;
    Ok(token_data.claims)
}
