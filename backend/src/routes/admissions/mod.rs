use crate::{
    modules::admissions::handler::{
        create_admission, enroll_admission, list_admissions, update_admission_status,
    },
    state::AppState,
};
use axum::{
    routing::{get, post, put},
    Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_admissions).post(create_admission))
        .route("/:id/enroll", post(enroll_admission))
        .route("/:id/status", put(update_admission_status))
}
