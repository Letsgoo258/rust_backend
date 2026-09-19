use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use super::dto::TelemetryEventRequest;

pub fn router() -> Router<AppState> {
    Router::new().route("/event", post(record_event))
}

async fn record_event(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<TelemetryEventRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (school_id, user_id) = if let Some(user) = auth_session.user {
        (Some(user.school_id), Some(user.id))
    } else {
        (None, None)
    };

    sqlx::query!(
        r#"
        INSERT INTO frontend_telemetry 
        (school_id, user_id, session_id, event_type, url, component, error_message, error_stack, duration_ms, metadata)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
        school_id,
        user_id,
        payload.session_id,
        payload.event_type,
        payload.url,
        payload.component,
        payload.error_message,
        payload.error_stack,
        payload.duration_ms,
        payload.metadata
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::CREATED)
}
