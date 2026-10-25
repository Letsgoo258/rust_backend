pub mod auth;

use axum::{
    routing::{get, post},
    Router,
};

use crate::modules::whatsapp::handler::WhatsAppHandler;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/stats", get(WhatsAppHandler::get_stats))
        .route("/inbound", get(WhatsAppHandler::get_inbound_messages))
        .route("/outbox", get(WhatsAppHandler::get_outbox_queue))
        .route("/send", post(WhatsAppHandler::send_message))
        .route(
            "/simulate-incoming",
            post(WhatsAppHandler::simulate_incoming),
        )
        // WhatsApp user linking & auth routes
        .route("/auth/generate-link-token", post(auth::generate_link_token))
        .route("/auth/verify-token", get(auth::verify_token))
        .route("/auth/send-otp", post(auth::send_otp))
        .route("/auth/complete", post(auth::complete_verification))
}
