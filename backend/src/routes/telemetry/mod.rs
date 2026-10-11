use crate::{modules::telemetry::handler::record_event, state::AppState};
use axum::{routing::post, Router};

pub fn router() -> Router<AppState> {
    Router::new().route("/event", post(record_event))
}
