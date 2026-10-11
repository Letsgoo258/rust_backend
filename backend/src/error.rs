use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("internal server error: {0}")]
    InternalServerError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            AppError::Database(err) => {
                if let Some(db_err) = err.as_database_error() {
                    if let Some(constraint) = db_err.constraint() {
                        if constraint == "schools_code_key" {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(json!({ "error": "A school with this code already exists." })),
                            )
                                .into_response();
                        }
                        if constraint == "users_email_key"
                            || constraint == "users_school_id_email_key"
                        {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(json!({ "error": "A user with this email already exists." })),
                            )
                                .into_response();
                        }
                        if constraint == "users_username_key"
                            || constraint == "users_school_id_username_key"
                        {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(json!({ "error": "This username is already taken." })),
                            )
                                .into_response();
                        }
                    }
                }
                tracing::error!("Database error: {:?}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Database error: {}", err),
                )
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::InternalServerError(msg) => {
                tracing::error!("Internal server error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg.clone())
            }
        };

        let body = Json(json!({ "error": error_message }));
        (status, body).into_response()
    }
}
