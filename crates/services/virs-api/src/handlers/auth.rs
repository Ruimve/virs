use axum::{extract::State, Json};
use virs_error::VirsError;

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;


#[derive(serde::Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}


#[derive(serde::Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserInfo,
}


#[derive(serde::Serialize)]
pub struct UserInfo {
    pub id: uuid::Uuid,
    pub username: String,
    pub role: String,
    pub email: Option<String>,
    pub is_active: bool,
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<ApiResponse>, VirsError> {

    let row = sqlx::query_as::<_, (uuid::Uuid, String, String, String, Option<String>, bool)>(
        r#"SELECT id, username, password_hash, role, email, is_active FROM qd_users WHERE username = $1"#,
    )
    .bind(&req.username)
    .fetch_optional(&state.db_pool)
    .await?;

    let (id, username, password_hash, role, email, is_active) = match row {
        Some(r) => r,
        None => {
            return Err(VirsError::unauthorized("Invalid credentials"));
        }
    };

    if !is_active {
        return Err(VirsError::Http {
            status: 403,
            message: "Account is disabled".into(),
        });
    }

    let valid = virs_utils::verify_password(&req.password, &password_hash);
    if !valid {
        return Err(VirsError::unauthorized("Invalid credentials"));
    }


    let secret = &state.jwt_secret;
    let expiration_hours: i64 = state.jwt_expiration_hours;

    let claims = virs_utils::Claims::new(
        &id.to_string(),
        &username,
        &role,
        expiration_hours * 3600,
    );
    let token = virs_utils::encode_jwt(&claims, secret)?;

    let login_resp = serde_json::to_value(LoginResponse {
        token,
        user: UserInfo {
            id,
            username,
            role,
            email,
            is_active,
        },
    })
    .map_err(|e| VirsError::Http {
        status: 500,
        message: format!("Failed to serialize login response: {}", e),
    })?;

    Ok(Json(ApiResponse::ok(login_resp)))
}

pub async fn logout() -> Result<Json<ApiResponse>, VirsError> {
    Ok(Json(ApiResponse::ok(
        serde_json::json!({"message": "Logged out"}),
    )))
}

pub async fn get_user_info(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let row = sqlx::query_as::<_, (String, String, Option<String>, bool)>(
        r#"SELECT username, role, email, is_active FROM qd_users WHERE id = $1"#,
    )
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await?;

    match row {
        Some((db_username, db_role, email, is_active)) => {
            if !is_active {
                return Err(VirsError::Http {
                    status: 403,
                    message: "Account is disabled".into(),
                });
            }
            Ok(Json(ApiResponse::ok(serde_json::json!({
                "id": user_id.to_string(),
                "username": db_username,
                "role": db_role,
                "email": email,
                "is_active": is_active,
            }))))
        }
        None => Err(VirsError::not_found("User not found")),
    }
}
