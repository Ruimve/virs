mod migrator;
pub mod models;
mod pool;
mod repositories;

/* 重导出 PgPool：virs-database 是 PgPool 类型的唯一来源，
   下游 crate 通过 virs_database::PgPool 引用，无需直接依赖 sqlx */
pub use sqlx::PgPool;
pub use migrator::run_migrations;
pub use pool::create_pool;
pub use repositories::*;
