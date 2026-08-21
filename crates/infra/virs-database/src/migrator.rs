use sqlx::PgPool;
use virs_error::{Context, VirsResult};

/* 编译时嵌入初始化SQL：消除运行时文件路径依赖，Docker部署无需挂载migrations目录 */
const INIT_SQL: &str = include_str!("../migrations/init.sql");

/* 执行数据库迁移：一次性执行全量DDL（CREATE TABLE IF NOT EXISTS + CREATE OR REPLACE VIEW） */
pub async fn run_migrations(pool: &PgPool) -> VirsResult<()> {
    sqlx::raw_sql(INIT_SQL)
        .execute(pool)
        .await
        .context("Failed to execute database migrations")?;
    Ok(())
}
