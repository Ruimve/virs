use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use virs_error::VirsResult;

/* 管理员引导：检查admin用户是否存在，不存在则创建 */
pub async fn ensure_admin(
    pool: &PgPool,
    username: &str,
    password_hash: &str,
) -> VirsResult<Uuid> {
    let admin_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM qd_users WHERE username = $1)")
            .bind(username)
            .fetch_one(pool)
            .await?;

    if !admin_exists {
        let row: (Uuid,) = sqlx::query_as(
            "INSERT INTO qd_users (username, password_hash, role, is_active) VALUES ($1, $2, 'admin', true) RETURNING id",
        )
        .bind(username)
        .bind(password_hash)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    } else {
        let row: (Uuid,) = sqlx::query_as(
            "SELECT id FROM qd_users WHERE username = $1 AND role = 'admin' LIMIT 1",
        )
        .bind(username)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }
}

/* 用户列表查询：返回所有用户的基础信息 */
pub async fn list_users(pool: &PgPool) -> VirsResult<Vec<(Uuid, String, String, Option<String>, bool, DateTime<Utc>)>> {
    let users = sqlx::query_as::<
        _,
        (Uuid, String, String, Option<String>, bool, DateTime<Utc>),
    >(
        r#"SELECT id, username, role, email, is_active, created_at FROM qd_users ORDER BY created_at DESC"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(users)
}

/* 创建用户：管理员创建新用户（仅允许user角色） */
pub async fn create_user(
    pool: &PgPool,
    id: Uuid,
    username: &str,
    password_hash: &str,
    role: &str,
    email: Option<&str>,
) -> VirsResult<()> {
    sqlx::query(
        r#"INSERT INTO qd_users (id, username, password_hash, role, email, is_active, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, true, NOW(), NOW())"#,
    )
    .bind(id)
    .bind(username)
    .bind(password_hash)
    .bind(role)
    .bind(email)
    .execute(pool)
    .await?;
    Ok(())
}

/* 更新用户：支持修改角色、邮箱、启用状态 */
pub async fn update_user(
    pool: &PgPool,
    id: Uuid,
    role: Option<&str>,
    email: Option<&str>,
    is_active: Option<bool>,
) -> VirsResult<()> {
    sqlx::query(
        r#"UPDATE qd_users SET role = COALESCE($2, role), email = COALESCE($3, email),
           is_active = COALESCE($4, is_active), updated_at = NOW() WHERE id = $1"#,
    )
    .bind(id)
    .bind(role)
    .bind(email)
    .bind(is_active)
    .execute(pool)
    .await?;
    Ok(())
}

/* 删除用户 */
pub async fn delete_user(pool: &PgPool, id: Uuid) -> VirsResult<()> {
    sqlx::query(r#"DELETE FROM qd_users WHERE id = $1"#)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/* 按用户名查询：登录时验证用户凭据 */
pub async fn find_user_by_username(
    pool: &PgPool,
    username: &str,
) -> VirsResult<Option<(Uuid, String, String, String, Option<String>, bool)>> {
    let row = sqlx::query_as::<
        _,
        (Uuid, String, String, String, Option<String>, bool),
    >(
        r#"SELECT id, username, password_hash, role, email, is_active FROM qd_users WHERE username = $1"#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/* 查询用户信息：用于获取当前登录用户的详情 */
pub async fn get_user_info(
    pool: &PgPool,
    user_id: Uuid,
) -> VirsResult<Option<(String, String, Option<String>, bool)>> {
    let row = sqlx::query_as::<_, (String, String, Option<String>, bool)>(
        r#"SELECT username, role, email, is_active FROM qd_users WHERE id = $1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}
