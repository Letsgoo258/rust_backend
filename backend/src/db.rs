use crate::config::AppConfig;
use sqlx::{postgres::{PgConnectOptions, PgPoolOptions}, ConnectOptions, PgPool};
use std::str::FromStr;
use std::time::Duration;

pub async fn init_pool(config: &AppConfig) -> Result<PgPool, sqlx::Error> {
    let mut db_url = config.database_url.clone();
    if !db_url.contains("sslrootcert") {
        db_url = format!("{}&sslrootcert=ca.pem", db_url);
    }

    let connect_options = PgConnectOptions::from_str(&db_url)?;
    // Increase slow statement threshold from 1s to 5s to avoid noisy logs in Supabase
    let connect_options = connect_options.log_slow_statements(tracing::log::LevelFilter::Warn, Duration::from_secs(5));

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(connect_options)
        .await?;

    tracing::info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Database migrations applied successfully.");

    Ok(pool)
}
