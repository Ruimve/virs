//! JWT authentication middleware and RBAC permission control.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode, header},
};
use uuid::Uuid;

use crate::models::{ApiResponse, UserRole};
use crate::utils::auth::decode_jwt;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: String,
    pub username: String,
    pub role: UserRole,
}

impl AuthUser {
    pub fn is_admin_or_manager(&self) -> bool {
        matches!(self.role, UserRole::Admin | UserRole::Manager)
    }

    pub fn is_admin(&self) -> bool {
        matches!(self.role, UserRole::Admin)
    }

    pub fn uuid(&self) -> Result<Uuid, String> {
        Uuid::parse_str(&self.user_id).map_err(|_| "Invalid user identity".to_string())
    }
}

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = (StatusCode, axum::Json<ApiResponse<serde_json::Value>>);

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");

        let token = if auth_header.starts_with("Bearer ") {
            &auth_header[7..]
        } else {
            return Err((
                StatusCode::UNAUTHORIZED,
                axum::Json(ApiResponse::<serde_json::Value>::err(
                    "Missing or invalid Authorization header. Use: Bearer <token>",
                )),
            ));
        };

        let secret_key = std::env::var("SECRET_KEY")
            .map_err(|_| (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiResponse::<serde_json::Value>::err("Server configuration error")),
            ))?;

        let claims = decode_jwt(token, &secret_key).map_err(|e| {
            (
                StatusCode::UNAUTHORIZED,
                axum::Json(ApiResponse::<serde_json::Value>::err(format!(
                    "Invalid or expired token: {}",
                    e
                ))),
            )
        })?;

        let now = chrono::Utc::now().timestamp();
        if claims.exp < now {
            return Err((
                StatusCode::UNAUTHORIZED,
                axum::Json(ApiResponse::<serde_json::Value>::err("Token has expired")),
            ));
        }

        let role = parse_role(&claims.role).unwrap_or(UserRole::User);

        Ok(AuthUser {
            user_id: claims.sub,
            username: claims.username,
            role,
        })
    }
}

#[derive(Debug, Clone)]
pub struct OptionalAuthUser(pub Option<AuthUser>);

impl<S: Send + Sync> FromRequestParts<S> for OptionalAuthUser {
    type Rejection = (StatusCode, axum::Json<ApiResponse<serde_json::Value>>);

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");

        if auth_header.is_empty() || !auth_header.starts_with("Bearer ") {
            return Ok(OptionalAuthUser(None));
        }

        let token = &auth_header[7..];

        let secret_key = match std::env::var("SECRET_KEY") {
            Ok(k) => k,
            Err(_) => return Ok(OptionalAuthUser(None)),
        };

        match decode_jwt(token, &secret_key) {
            Ok(claims) => {
                let now = chrono::Utc::now().timestamp();
                if claims.exp < now {
                    return Ok(OptionalAuthUser(None));
                }
                let role = parse_role(&claims.role).unwrap_or(UserRole::User);
                Ok(OptionalAuthUser(Some(AuthUser {
                    user_id: claims.sub,
                    username: claims.username,
                    role,
                })))
            }
            Err(_) => Ok(OptionalAuthUser(None)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RequireAdmin(pub AuthUser);

impl<S: Send + Sync> FromRequestParts<S> for RequireAdmin {
    type Rejection = (StatusCode, axum::Json<ApiResponse<serde_json::Value>>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if !user.is_admin() {
            return Err((
                StatusCode::FORBIDDEN,
                axum::Json(ApiResponse::<serde_json::Value>::err(
                    "Admin access required",
                )),
            ));
        }
        Ok(RequireAdmin(user))
    }
}

#[derive(Debug, Clone)]
pub struct RequireManager(pub AuthUser);

impl<S: Send + Sync> FromRequestParts<S> for RequireManager {
    type Rejection = (StatusCode, axum::Json<ApiResponse<serde_json::Value>>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if !user.is_admin_or_manager() {
            return Err((
                StatusCode::FORBIDDEN,
                axum::Json(ApiResponse::<serde_json::Value>::err(
                    "Admin or Manager access required",
                )),
            ));
        }
        Ok(RequireManager(user))
    }
}

fn parse_role(role_str: &str) -> Option<UserRole> {
    match role_str.to_lowercase().as_str() {
        "admin" => Some(UserRole::Admin),
        "manager" => Some(UserRole::Manager),
        "user" => Some(UserRole::User),
        "viewer" => Some(UserRole::Viewer),
        _ => None,
    }
}
