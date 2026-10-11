use crate::state::AppState;
use axum::{routing::get, Router};

pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(health_check))
}

pub async fn health_check() -> &'static str {
    "OK"
}
