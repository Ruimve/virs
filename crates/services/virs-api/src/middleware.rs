//! API middleware — JWT auth extraction, error handling.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

/// JWT 认证中间件
pub async fn auth_middleware(request: Request, next: Next) -> Response {
    let auth_header = request.headers().get("Authorization");
    if auth_header.is_none() {
        return (StatusCode::UNAUTHORIZED, "Missing authorization header").into_response();
    }

    let token = auth_header
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    if token.is_none() {
        return (StatusCode::UNAUTHORIZED, "Invalid authorization format").into_response();
    }

    // Token validation is done per-handler using the token value
    next.run(request).await
}
