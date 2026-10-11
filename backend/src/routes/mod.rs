pub mod academic;
pub mod admissions;
pub mod auth;
pub mod cron;
pub mod health;
pub mod telemetry;
pub mod users;

use crate::state::AppState;
use axum::Router;

pub fn app_router() -> Router<AppState> {
    Router::new()
        .route("/health", axum::routing::get(health::health_check))
        .nest("/api/v1", health::router())
        .nest("/api/v1/auth", auth::router())
        .nest("/api/v1/users", users::router())
        .nest("/api/v1/admissions", admissions::router())
        .nest("/api/v1/academic", academic::router())
        .nest("/api/v1/telemetry", telemetry::router())
        .nest("/api/v1/cron", cron::router())
}
