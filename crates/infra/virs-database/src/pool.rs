use std::time::Duration;

use sqlx::PgPool;
use virs_error::VirsResult;

/* 创建PostgreSQL连接池：接收原始连接参数，由上层从配置转换为参数后传入 */
pub async fn create_pool(
    url: &str,
    min_connections: u32,
    max_connections: u32,
    acquire_timeout: Duration,
) -> VirsResult<PgPool> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .min_connections(min_connections)
        .max_connections(max_connections)
        .acquire_timeout(acquire_timeout)
        .connect(url)
        .await?;
    Ok(pool)
}
