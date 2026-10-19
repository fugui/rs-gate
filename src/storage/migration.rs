use sqlx::SqlitePool;

/// 执行嵌入式声明式自迁移，保障幂等性与版本化追溯
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
