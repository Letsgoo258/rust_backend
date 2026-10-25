mod config;
mod db;
mod error;
mod middleware;
mod modules;
mod routes;
mod state;
mod utils;

use axum::http::{
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, COOKIE, SET_COOKIE},
    Method,
};
use config::AppConfig;
use state::AppState;
use std::net::SocketAddr;
use std::panic;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::cors::{AllowOrigin, CorsLayer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize structured concurrency tracing (Console + File + Grafana OpenTelemetry)
    dotenvy::dotenv().ok();
    let _tokio_handle_guard = tokio::runtime::Handle::current().enter();
    let tracer_provider = utils::telemetry::init_telemetry()?;

    // 2. Set up global panic hook to prevent silent thread deaths
    panic::set_hook(Box::new(|panic_info| {
        let location = panic_info
            .location()
            .map(|l| l.to_string())
            .unwrap_or_else(|| "unknown".into());
        let msg = match panic_info.payload().downcast_ref::<&str>() {
            Some(s) => *s,
            None => match panic_info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<dyn Any>",
            },
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

    // Spawn background workers
    tokio::spawn(modules::telemetry::worker::run_telemetry_batch_worker(
        pool.clone(),
    ));
    tokio::spawn(modules::auth::worker::run_login_security_worker(
        pool.clone(),
    ));

    // Create App State
    let state = AppState::new(pool, config.clone());

    // Initialize in-process WhatsApp Service (restores session if exists, starts queue consumer)
    let wa_config = utils::whatsapp::WhatsAppConfig::new(state.db.clone());
    match utils::whatsapp::WhatsAppService::start(wa_config).await {
        Ok(wa_service) => {
            let wa_arc = std::sync::Arc::new(wa_service);
            *state.whatsapp.write().await = Some(wa_arc);
            tracing::info!("In-process WhatsApp service initialized on boot");
        }
        Err(e) => {
            tracing::warn!("WhatsApp service not started on boot: {e:#}");
        }
    }

    // Recover & clean background upload jobs on startup (Auto-purges old jobs >3 days)
    utils::jobs::recover_and_clean_jobs(&state.db, state.jobs.clone()).await;

    // IP-based Rate Limiting (200 requests per second, burst 1000)
    let governor_conf = Box::new(
        GovernorConfigBuilder::default()
            .per_millisecond(5)
            .burst_size(1000)
            .finish()
            .unwrap(),
    );

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            ACCEPT,
            AUTHORIZATION,
            CONTENT_TYPE,
            COOKIE,
            SET_COOKIE,
            axum::http::HeaderName::from_static("x-school-id"),
            axum::http::HeaderName::from_static("x-tenant-id"),
        ])
        .allow_credentials(true);
    let governor_layer = GovernorLayer {
        config: Box::leak(governor_conf),
    };

    // Build our application with organized modular routes
    let app = routes::app_router()
        .layer(axum::middleware::from_fn(
            middleware::auth::enforce_tenant_isolation_middleware,
        ))
        .layer(axum::middleware::from_fn(
            middleware::signature::verify_url_signature_middleware,
        ))
        .layer(auth_layer)
        .layer(governor_layer)
        .layer(cors)
        .layer(tower_http::trace::TraceLayer::new_for_http().on_response(
            |response: &axum::response::Response,
             latency: std::time::Duration,
             _span: &tracing::Span| {
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
        ))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // 4. Serve with Graceful Shutdown to ensure threads exit intelligently
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(utils::shutdown::shutdown_signal())
    .await?;

    tracing::info!("Server shut down gracefully. All worker threads cleaned up.");
    // Ensure all OTLP telemetry traces are flushed before the program exits
    if let Some(provider) = tracer_provider {
        let _ = provider.shutdown();
    }
    Ok(())
}
