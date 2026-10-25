use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::modules::whatsapp::repository::WhatsAppRepository;
use crate::modules::whatsapp::service::WhatsAppMessageService;
use crate::state::AppState;
use crate::utils::whatsapp::send_whatsapp_message;

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub recipient: String,
    pub text: String,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SimulateIncomingRequest {
    pub sender_jid: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct SimulateIncomingResponse {
    pub success: bool,
    pub sender_jid: String,
    pub incoming_text: String,
    pub reply_sent: Option<String>,
}

pub struct WhatsAppHandler;

impl WhatsAppHandler {
    /// Retrieve WhatsApp metrics & queue counts.
    pub async fn get_stats(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
        let stats = WhatsAppRepository::get_stats(&state.db).await?;
        Ok((StatusCode::OK, Json(stats)))
    }

    /// Retrieve paginated list of inbound messages.
    pub async fn get_inbound_messages(
        State(state): State<AppState>,
        Query(params): Query<PaginationQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let limit = params.limit.unwrap_or(50).clamp(1, 100);
        let offset = params.offset.unwrap_or(0).max(0);

        let messages = WhatsAppRepository::get_inbound_messages(&state.db, limit, offset).await?;
        Ok((StatusCode::OK, Json(messages)))
    }

    /// Retrieve paginated list of outbox queue rows.
    pub async fn get_outbox_queue(
        State(state): State<AppState>,
        Query(params): Query<PaginationQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let limit = params.limit.unwrap_or(50).clamp(1, 100);
        let offset = params.offset.unwrap_or(0).max(0);

        let items = WhatsAppRepository::get_outbox_queue(&state.db, limit, offset).await?;
        Ok((StatusCode::OK, Json(items)))
    }

    /// Admin endpoint to send an outbound WhatsApp message.
    pub async fn send_message(
        State(state): State<AppState>,
        Json(req): Json<SendMessageRequest>,
    ) -> Result<impl IntoResponse, AppError> {
        let trimmed_text = req.text.trim();
        if trimmed_text.is_empty() {
            return Err(AppError::BadRequest("Message text cannot be empty".into()));
        }

        let outbox_id = send_whatsapp_message(
            &state.db,
            &req.recipient,
            trimmed_text,
            req.idempotency_key.as_deref(),
        )
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to enqueue message: {e}")))?;

        Ok((
            StatusCode::ACCEPTED,
            Json(serde_json::json!({
                "success": true,
                "outbox_id": outbox_id,
                "status": "queued"
            })),
        ))
    }

    /// Developer/Admin endpoint to simulate receiving an inbound WhatsApp message.
    /// Runs the message through greeting/help/AI/fallback engine and enqueues the reply.
    pub async fn simulate_incoming(
        State(state): State<AppState>,
        Json(req): Json<SimulateIncomingRequest>,
    ) -> Result<impl IntoResponse, AppError> {
        let reply =
            WhatsAppMessageService::process_incoming(&state.db, &req.sender_jid, &req.text).await?;

        Ok((
            StatusCode::OK,
            Json(SimulateIncomingResponse {
                success: true,
                sender_jid: req.sender_jid,
                incoming_text: req.text,
                reply_sent: reply,
            }),
        ))
    }
}
