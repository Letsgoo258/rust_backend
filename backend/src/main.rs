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
use std::panic;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize structured concurrency tracing (Console + File + Grafana OpenTelemetry)
    dotenvy::dotenv().ok();
    let _provider = utils::telemetry::init_telemetry()?;

    // 2. Set up global panic hook to prevent silent thread deaths
    panic::set_hook(Box::new(|panic_info| {
        let location = panic_info.location().map(|l| l.to_string()).unwrap_or_else(|| "unknown".into());
        let msg = match panic_info.payload().downcast_ref::<&str>() {
            Some(s) => *s,
            None => match panic_info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<dyn Any>",
            }
        };
        tracing::error!("PANIC at {}: {}", location, msg);
    }));

    tracing::info!("Starting ERAVAYA ERP backend initialization...");

    let config = AppConfig::from_env();

    // 3. Database with connection timeout to prevent deadlocks hanging the startup
    tracing::info!("Connecting to the database...");
    let pool = match db::init_pool(&config).await {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("FATAL: Failed to connect to the database: {:?}", e);
            return Err(e.into());
        }
    };

    // Auth Layer
    let auth_layer = middleware::auth::build_auth_layer(pool.clone()).await;

    // Create App State
    let state = AppState::new(pool, config.clone());


    // IP-based Rate Limiting (5 requests per second, burst 20)
    let governor_conf = Box::new(
        GovernorConfigBuilder::default()
            .per_second(2)
            .burst_size(20)
            .finish()
            .unwrap()
    );
    let governor_layer = GovernorLayer {
        config: Box::leak(governor_conf),
    };

    // Build our application with routes

    let app = Router::new()
        .nest("/api/v1", routes::health::router())
        .nest("/api/v1/auth", modules::auth::router())
        .nest("/api/v1/sis", modules::sis::handler::router())
        .layer(auth_layer)
        .layer(governor_layer)
        
        .layer(
            tower_http::trace::TraceLayer::new_for_http()
                .on_response(
                    |response: &axum::response::Response, latency: std::time::Duration, _span: &tracing::Span| {
                        if response.status().is_server_error() || response.status().is_client_error() {
                            tracing::error!(
                                latency_ms = latency.as_millis(),
                                status = response.status().as_u16(),
                                otel.status_code = "ERROR",
                                "HTTP Request failed"
                            );
                        } else {
                            tracing::info!(
                                latency_ms = latency.as_millis(),
                                status = response.status().as_u16(),
                                "HTTP Request succeeded"
                            );
                        }
                    },
                ),
        )
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // 4. Serve with Graceful Shutdown to ensure threads exit intelligently
    axum::serve(listener, app)
        .with_graceful_shutdown(utils::shutdown::shutdown_signal())
        .await?;

    tracing::info!("Server shut down gracefully. All worker threads cleaned up.");
    // Ensure all OTLP telemetry traces are flushed before the program exits
    Ok(())
}
