use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use virs_error::VirsResult;

/* ========== BotEngine 使用的查询 ========== */

/* 查询用户的AI凭据（加密状态）：供 PgCredentialStore 解密后传给 BotEngine */
pub async fn load_ai_credentials_for_bot(
    pool: &PgPool,
    user_id: Uuid,
) -> VirsResult<Vec<(String, String, Option<String>)>> {
    let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
        r#"SELECT provider, encrypted_api_key, model FROM qd_ai_credentials WHERE user_id = $1"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/* ========== API handler 使用的查询 ========== */

/* 查询用户的交易所凭据列表 */
pub async fn list_exchange_credentials(
    pool: &PgPool,
    user_id: Uuid,
) -> VirsResult<Vec<(Uuid, String, String, DateTime<Utc>)>> {
    let creds = sqlx::query_as::<_, (Uuid, String, String, DateTime<Utc>)>(
        r#"SELECT id, exchange, label, created_at FROM qd_exchange_credentials WHERE user_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(creds)
}

/* 保存或更新交易所凭据（UPSERT） */
pub async fn save_exchange_credential(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    exchange: &str,
    label: &str,
    encrypted_api_key: &str,
    encrypted_secret: &str,
    encrypted_passphrase: Option<&str>,
) -> VirsResult<()> {
    sqlx::query(
        r#"INSERT INTO qd_exchange_credentials (id, user_id, exchange, label, encrypted_api_key, encrypted_api_secret, encrypted_passphrase, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
           ON CONFLICT (user_id, exchange)
           DO UPDATE SET encrypted_api_key = $5, encrypted_api_secret = $6, encrypted_passphrase = $7, label = $4, updated_at = NOW()"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(exchange)
    .bind(label)
    .bind(encrypted_api_key)
    .bind(encrypted_secret)
    .bind(encrypted_passphrase)
    .execute(pool)
    .await?;
    Ok(())
}

/* 删除交易所凭据 */
pub async fn delete_exchange_credential(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> VirsResult<()> {
    sqlx::query(r#"DELETE FROM qd_exchange_credentials WHERE id = $1 AND user_id = $2"#)
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/* 查询用户最新的交易所名称（用于状态检查） */
pub async fn get_user_exchange(pool: &PgPool, user_id: Uuid) -> VirsResult<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"SELECT exchange FROM qd_exchange_credentials WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(exchange,)| exchange))
}

/* 查询所有交易所凭据（重启恢复时解密使用） */
pub async fn get_all_exchange_credentials(
    pool: &PgPool,
) -> VirsResult<Vec<(String, String, String, Option<String>)>> {
    let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        r#"SELECT exchange, encrypted_api_key, encrypted_api_secret, encrypted_passphrase
           FROM qd_exchange_credentials"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/* 查询用户的AI凭据列表 */
pub async fn list_ai_credentials(
    pool: &PgPool,
    user_id: Uuid,
) -> VirsResult<Vec<(Uuid, String, Option<String>, bool, DateTime<Utc>, DateTime<Utc>)>> {
    let creds = sqlx::query_as::<
        _,
        (Uuid, String, Option<String>, bool, DateTime<Utc>, DateTime<Utc>),
    >(
        r#"SELECT id, provider, label, is_default, created_at, updated_at FROM qd_ai_credentials WHERE user_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(creds)
}

/* 保存或更新AI凭据（UPSERT） */
pub async fn save_ai_credential(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    provider: &str,
    encrypted_key: &str,
    model: Option<&str>,
    label: &str,
    is_default: bool,
) -> VirsResult<()> {
    sqlx::query(
        r#"INSERT INTO qd_ai_credentials (id, user_id, provider, encrypted_api_key, model, label, is_default, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
           ON CONFLICT (user_id, provider)
           DO UPDATE SET encrypted_api_key = $4, model = $5, label = $6, is_default = $7, updated_at = NOW()"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(provider)
    .bind(encrypted_key)
    .bind(model)
    .bind(label)
    .bind(is_default)
    .execute(pool)
    .await?;
    Ok(())
}

/* 删除AI凭据 */
pub async fn delete_ai_credential(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> VirsResult<()> {
    sqlx::query(r#"DELETE FROM qd_ai_credentials WHERE id = $1 AND user_id = $2"#)
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/* 查询用户默认AI凭据（测试连接/获取模型列表时使用） */
pub async fn get_default_ai_credential(
    pool: &PgPool,
    user_id: Uuid,
) -> VirsResult<Option<(String, String)>> {
    let row: Option<(String, String)> = sqlx::query_as(
        r#"SELECT provider, encrypted_api_key FROM qd_ai_credentials WHERE user_id = $1 AND is_default = true ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/* 查询所有已配置的AI provider列表 */
pub async fn get_ai_providers(pool: &PgPool) -> VirsResult<Vec<String>> {
    let rows: Vec<String> =
        sqlx::query_scalar(r#"SELECT DISTINCT provider FROM qd_ai_credentials"#)
            .fetch_all(pool)
            .await?;
    Ok(rows)
}

/* 查询最新的LLM凭据（全局，非per-user）：供StrategyEngine和AppState使用 */
pub async fn get_latest_llm_credential(pool: &PgPool) -> VirsResult<Option<(String, String)>> {
    let row: Option<(String, String)> = sqlx::query_as(
        r#"SELECT provider, encrypted_api_key FROM qd_ai_credentials ORDER BY created_at DESC LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}
