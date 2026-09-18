use crate::config::AppConfig;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;

pub async fn init_pool(config: &AppConfig) -> Result<PgPool, sqlx::Error> {
    let mut db_url = config.database_url.clone();
    if !db_url.contains("sslrootcert") {
        db_url = format!("{}&sslrootcert=ca.pem", db_url);
    }

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await?;

    tracing::info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Database migrations applied successfully.");

    Ok(pool)
}
