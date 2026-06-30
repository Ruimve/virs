use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
}

impl Claims {
    /// Create a new Claims with iat set to now and exp = now + exp_secs.
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

pub fn encode_jwt(claims: &Claims, secret: &str) -> anyhow::Result<String> {
    let mut claims = claims.clone();
    claims.iat = chrono::Utc::now().timestamp();
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok(token)
}

pub fn decode_jwt(token: &str, secret: &str) -> anyhow::Result<Claims> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}
