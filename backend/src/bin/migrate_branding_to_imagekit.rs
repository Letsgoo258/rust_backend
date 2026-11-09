use eravaya::routes::storage::run_legacy_asset_migration;
use sqlx::postgres::PgPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    println!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    println!("Running legacy branding migration to ImageKit...");
    let report = run_legacy_asset_migration(&pool).await?;

    println!("============================================================");
    println!("LEGACY ASSET MIGRATION TO IMAGEKIT COMPLETE");
    println!("============================================================");
    println!("Migrated Logos:        {}", report.migrated_logos);
    println!("ImageKit Logo URL:     {:?}", report.imagekit_logo_url);
    println!("Settings Rows Updated: {}", report.settings_updated);
    println!(
        "Database Bytes Freed:  {} bytes (~{:.2} MB)",
        report.bytes_freed,
        report.bytes_freed as f64 / (1024.0 * 1024.0)
    );
    println!("============================================================");

    Ok(())
}
