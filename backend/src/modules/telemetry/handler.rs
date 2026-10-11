use super::dto::TelemetryEventRequest;
use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

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

pub async fn record_event(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<TelemetryEventRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (school_id, user_id) = if let Some(user) = auth_session.user {
        (Some(user.school_id), Some(user.id))
    } else {
        (None, None)
    };

    let telemetry_payload = crate::utils::telemetry::TelemetryPayload {
        school_id,
        user_id,
        session_id: payload.session_id,
        event_type: payload.event_type,
        severity: "INFO".into(),
        url: payload.url,
        component: payload.component,
        error_message: payload.error_message,
        error_stack: payload.error_stack,
        duration_ms: payload.duration_ms,
        metadata: payload.metadata,
        timestamp: time::OffsetDateTime::now_utc().to_string(),
    };

    let dispatcher =
        crate::utils::telemetry::TelemetryDispatcher::with_default_adapters(Some(state.db.clone()));
    dispatcher.dispatch(telemetry_payload).await;

    Ok(StatusCode::CREATED)
}
