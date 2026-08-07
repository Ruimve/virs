use virs_error::VirsError;


/* 统一API响应包装：所有API返回统一的{success, data, message}格式 */
#[derive(serde::Serialize)]
pub struct ApiResponse {
    pub success: bool,
    pub data: serde_json::Value,
    pub message: Option<String>,
}

impl ApiResponse {
    pub fn ok(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data,
            message: None,
        }
    }
}


/* 从Authorization Bearer token中提取用户ID：解析JWT并验证签名，失败返回401 */
pub fn extract_user_id(
    headers: &axum::http::HeaderMap,
    jwt_secret: &str,
) -> Result<uuid::Uuid, VirsError> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(t) => t,
        None => {
            return Err(VirsError::unauthorized(
                "Missing or invalid authorization header",
            ))
        }
    };

    match virs_utils::decode_jwt(token, jwt_secret) {
        Ok(claims) => uuid::Uuid::parse_str(&claims.sub)
            .map_err(|_| VirsError::unauthorized("Invalid user ID in token")),
        Err(_) => Err(VirsError::unauthorized("Invalid token")),
    }
}
