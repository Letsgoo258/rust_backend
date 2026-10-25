pub mod admissions;
pub mod whatsapp;

use crate::state::AppState;
use axum::Router;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/whatsapp", whatsapp::router())
        .nest("/admissions", admissions::router())
}
