use axum::{routing::post, Router};
use crate::state::AppState;
use super::admins::add_admin;
use super::students_bulk::bulk_upload_students;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admins", post(add_admin))
        .route("/students/bulk-upload", post(bulk_upload_students))
}
