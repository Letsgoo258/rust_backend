use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use super::dto::TelemetryEventRequest;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct TelemetryRecord {
    pub school_id: Option<uuid::Uuid>,
    pub user_id: Option<uuid::Uuid>,
    pub session_id: String,
    pub event_type: String,
    pub url: Option<String>,
    pub component: Option<String>,
    pub error_message: Option<String>,
    pub error_stack: Option<String>,
    pub duration_ms: Option<i32>,
    pub metadata: Option<serde_json::Value>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/event", post(record_event))
}

async fn record_event(
    auth_session: AuthSession,
    State(_state): State<AppState>,
    Json(payload): Json<TelemetryEventRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (school_id, user_id) = if let Some(user) = auth_session.user {
        (Some(user.school_id), Some(user.id))
    } else {
        (None, None)
    };

    let record = TelemetryRecord {
        school_id,
        user_id,
        session_id: payload.session_id,
        event_type: payload.event_type,
        url: payload.url,
        component: payload.component,
        error_message: payload.error_message,
        error_stack: payload.error_stack,
        duration_ms: payload.duration_ms,
        metadata: payload.metadata,
    };
    
    if let Ok(json_line) = serde_json::to_string(&record) {
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("telemetry_buffer.jsonl") {
            use std::io::Write;
            let _ = writeln!(file, "{}", json_line);
        }
    }

    Ok(StatusCode::CREATED)
}
