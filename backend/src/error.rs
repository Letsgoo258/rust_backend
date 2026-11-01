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
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("internal server error: {0}")]
    InternalServerError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            AppError::Database(err) => {
                if let Some(db_err) = err.as_database_error() {
                    let code = db_err.code().map(|c| c.to_string()).unwrap_or_default();
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
                        if constraint == "users_school_id_admission_number_key"
                            || constraint == "idx_users_upper_admission_number"
                        {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(json!({ "error": "An admission number with this value already exists in this school. Admission numbers must be unique." })),
                            )
                                .into_response();
                        }
                        if constraint == "enrollments_student_id_academic_year_id_key" {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(json!({ "error": "This student already has an active enrollment record for this academic year." })),
                            )
                                .into_response();
                        }
                        if constraint.contains("fkey") {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(json!({ "error": "Referenced entity does not exist or was removed." })),
                            )
                                .into_response();
                        }
                    }

                    // Check by SQL state code
                    if code == "23505" {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({ "error": "Duplicate entry: A record with this unique value already exists." })),
                        )
                            .into_response();
                    } else if code == "23503" {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({ "error": "Referenced record does not exist or has been removed." })),
                        )
                            .into_response();
                    } else if code == "23502" {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({ "error": "A required database field was left empty." })),
                        )
                            .into_response();
                    } else if code == "22001" {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({ "error": "One or more values exceed the maximum allowed length." })),
                        )
                            .into_response();
                    } else if code == "22007" {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({ "error": "Invalid date format provided. Please use YYYY-MM-DD." })),
                        )
                            .into_response();
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
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::InternalServerError(msg) => {
                tracing::error!("Internal server error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg.clone())
            }
        };

        let body = Json(json!({ "error": error_message }));
        (status, body).into_response()
    }
}
