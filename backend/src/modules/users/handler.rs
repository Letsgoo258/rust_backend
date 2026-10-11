use super::repository::UserRepository;
use crate::{
    error::AppError, modules::auth::backend::AuthSession, state::AppState,
    utils::filter_engine::FilterEngine,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::collections::HashMap;

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn get_users_list(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Query(raw_query): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    // Dynamically and safely sanitize incoming query parameters against the configured page schema
    let sanitized_filters = FilterEngine::sanitize_params("users", &raw_query);
    let (users, total_count) =
        UserRepository::list_users(&state.db, user.school_id, &sanitized_filters).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "users": users,
            "total_count": total_count,
            "applied_filters": sanitized_filters
        })),
    ))
}

pub async fn get_users_filter_schema() -> impl IntoResponse {
    let schema = FilterEngine::get_schema("users");
    (StatusCode::OK, Json(schema))
}
