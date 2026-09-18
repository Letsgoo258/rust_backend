mod config;
mod db;
mod error;
mod middleware;
mod modules;
mod routes;
mod state;
mod utils;

use axum::Router;
use config::AppConfig;
use state::AppState;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting ERAVAYA ERP backend initialization...");

    let config = AppConfig::from_env();

    tracing::info!("Connecting to the database...");
    let pool = match db::init_pool(&config).await {
        Ok(pool) => pool,
        Err(e) => {
            tracing::warn!("Failed to connect to the database: {:?}", e);
            return Err(e.into());
        }
    };

    // Auth Layer
    let auth_layer = middleware::auth::build_auth_layer(pool.clone()).await;

    // Create App State
    let state = AppState::new(pool, config.clone());

    // Build our application with routes
    let app = Router::new()
        .nest("/api/v1", routes::health::router())
        .nest("/api/v1/auth", modules::auth::router())
        .layer(auth_layer)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
