use crate::modules::cron::handler::{
    cleanup_fifteen_days, cleanup_otps, cleanup_password_resets, cleanup_sessions,
    run_all_cron_jobs,
};
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/run", post(run_all_cron_jobs))
        .route("/run-all", post(run_all_cron_jobs))
        .route("/status", get(run_all_cron_jobs))
        .route("/cleanup/otps", post(cleanup_otps))
        .route("/cleanup/sessions", post(cleanup_sessions))
        .route("/cleanup/password-resets", post(cleanup_password_resets))
        .route("/cleanup/15-days", post(cleanup_fifteen_days))
        .route("/cleanup/compliance-15-days", post(cleanup_fifteen_days))
        .route(
            "/cleanup/fifteen-days-retention",
            post(cleanup_fifteen_days),
        )
}
