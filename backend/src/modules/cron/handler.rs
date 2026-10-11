use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use std::time::Instant;
use time::OffsetDateTime;

use crate::{
    error::AppError,
    modules::auth::backend::AuthSession,
    modules::cron::{
        dto::{CronJobResult, CronOptionsQuery, CronSummaryResponse},
        repository::CronRepository,
    },
    state::AppState,
};

/// Validates that the request is authorized either by an active admin session or a valid cron secret
fn verify_cron_authorization(
    headers: &HeaderMap,
    auth_session: &AuthSession,
) -> Result<(), AppError> {
    // 1. Check if valid cron secret key header is provided
    if let Ok(expected) = std::env::var("CRON_SECRET") {
        if !expected.is_empty() {
            if let Some(header_val) = headers.get("x-cron-key").and_then(|h| h.to_str().ok()) {
                if header_val == expected {
                    return Ok(());
                }
            }
            if let Some(auth_header) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
                if let Some(token) = auth_header.strip_prefix("Bearer ") {
                    if token == expected {
                        return Ok(());
                    }
                }
            }
        }
    }

    // 2. Check if logged-in session has Super Admin or Admin role
    if let Some(user) = &auth_session.user {
        if user.roles.iter().any(|r| {
            let upper = r.to_uppercase();
            upper == "SUPER_ADMIN" || upper == "ADMIN" || upper == "SCHOOL_ADMIN"
        }) {
            return Ok(());
        }
    }

    Err(AppError::Forbidden(
        "Unauthorized cron execution. Valid X-Cron-Key header or Admin session required.".into(),
    ))
}

/// Execute all system cron jobs & periodic maintenance tasks
pub async fn run_all_cron_jobs(
    headers: HeaderMap,
    auth_session: AuthSession,
    Query(params): Query<CronOptionsQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    verify_cron_authorization(&headers, &auth_session)?;

    let start_time = Instant::now();
    let mut results = Vec::new();
    let mut total_affected: u64 = 0;

    // 1. Expired OTPs
    match CronRepository::cleanup_expired_otps(&state.db).await {
        Ok(count) => {
            total_affected += count;
            results.push(CronJobResult {
                task_name: "cleanup_expired_otps".into(),
                records_affected: count,
                status: "SUCCESS".into(),
                message: Some(format!("Purged {} expired action OTPs", count)),
            });
        }
        Err(e) => {
            results.push(CronJobResult {
                task_name: "cleanup_expired_otps".into(),
                records_affected: 0,
                status: "ERROR".into(),
                message: Some(e.to_string()),
            });
        }
    }

    // 2. Expired Password Resets
    match CronRepository::cleanup_expired_password_resets(&state.db).await {
        Ok(count) => {
            total_affected += count;
            results.push(CronJobResult {
                task_name: "cleanup_expired_password_resets".into(),
                records_affected: count,
                status: "SUCCESS".into(),
                message: Some(format!("Purged {} expired password reset tokens", count)),
            });
        }
        Err(e) => {
            results.push(CronJobResult {
                task_name: "cleanup_expired_password_resets".into(),
                records_affected: 0,
                status: "ERROR".into(),
                message: Some(e.to_string()),
            });
        }
    }

    // 3. Expired User Sessions
    match CronRepository::cleanup_expired_sessions(&state.db).await {
        Ok(count) => {
            total_affected += count;
            results.push(CronJobResult {
                task_name: "cleanup_expired_sessions".into(),
                records_affected: count,
                status: "SUCCESS".into(),
                message: Some(format!("Purged {} expired user sessions", count)),
            });
        }
        Err(e) => {
            results.push(CronJobResult {
                task_name: "cleanup_expired_sessions".into(),
                records_affected: 0,
                status: "ERROR".into(),
                message: Some(e.to_string()),
            });
        }
    }

    // 4. Cleanup Old Telemetry
    let retention_days = params.telemetry_retention_days.unwrap_or(30);
    match CronRepository::cleanup_old_telemetry(&state.db, retention_days).await {
        Ok(count) => {
            total_affected += count;
            results.push(CronJobResult {
                task_name: "cleanup_old_telemetry".into(),
                records_affected: count,
                status: "SUCCESS".into(),
                message: Some(format!(
                    "Purged {} telemetry records older than {} days",
                    count, retention_days
                )),
            });
        }
        Err(e) => {
            results.push(CronJobResult {
                task_name: "cleanup_old_telemetry".into(),
                records_affected: 0,
                status: "ERROR".into(),
                message: Some(e.to_string()),
            });
        }
    }

    // 5. Expired Capability Grants
    match CronRepository::cleanup_expired_capability_grants(&state.db).await {
        Ok(count) => {
            total_affected += count;
            results.push(CronJobResult {
                task_name: "cleanup_expired_capability_grants".into(),
                records_affected: count,
                status: "SUCCESS".into(),
                message: Some(format!(
                    "Purged {} stale temporary capability grants",
                    count
                )),
            });
        }
        Err(e) => {
            results.push(CronJobResult {
                task_name: "cleanup_expired_capability_grants".into(),
                records_affected: 0,
                status: "ERROR".into(),
                message: Some(e.to_string()),
            });
        }
    }

    // 6. 15-Day Compliance Edit History Pruning
    match CronRepository::cleanup_expired_edit_history(&state.db, 15).await {
        Ok(count) => {
            total_affected += count;
            results.push(CronJobResult {
                task_name: "cleanup_expired_edit_history".into(),
                records_affected: count,
                status: "SUCCESS".into(),
                message: Some(format!(
                    "Pruned {} edit history records older than 15 days",
                    count
                )),
            });
        }
        Err(e) => {
            results.push(CronJobResult {
                task_name: "cleanup_expired_edit_history".into(),
                records_affected: 0,
                status: "ERROR".into(),
                message: Some(e.to_string()),
            });
        }
    }

    let elapsed = start_time.elapsed().as_millis();
    let response = CronSummaryResponse {
        status: "COMPLETED".into(),
        executed_at: OffsetDateTime::now_utc().to_string(),
        duration_ms: elapsed,
        total_records_affected: total_affected,
        results,
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Cleanup only expired OTPs
pub async fn cleanup_otps(
    headers: HeaderMap,
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    verify_cron_authorization(&headers, &auth_session)?;

    let count = CronRepository::cleanup_expired_otps(&state.db).await?;

    Ok(Json(serde_json::json!({
        "status": "SUCCESS",
        "task": "cleanup_expired_otps",
        "records_purged": count
    })))
}

/// Cleanup only expired sessions
pub async fn cleanup_sessions(
    headers: HeaderMap,
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    verify_cron_authorization(&headers, &auth_session)?;

    let count = CronRepository::cleanup_expired_sessions(&state.db).await?;

    Ok(Json(serde_json::json!({
        "status": "SUCCESS",
        "task": "cleanup_expired_sessions",
        "records_purged": count
    })))
}

/// Cleanup only expired password resets
pub async fn cleanup_password_resets(
    headers: HeaderMap,
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    verify_cron_authorization(&headers, &auth_session)?;

    let count = CronRepository::cleanup_expired_password_resets(&state.db).await?;

    Ok(Json(serde_json::json!({
        "status": "SUCCESS",
        "task": "cleanup_expired_password_resets",
        "records_purged": count
    })))
}

/// Cleanup all records older than 15 days for compliance (edit histories, resolved notifications, expired OTPs, telemetry)
pub async fn cleanup_fifteen_days(
    headers: HeaderMap,
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    verify_cron_authorization(&headers, &auth_session)?;

    let metrics = CronRepository::cleanup_15_day_records(&state.db)
        .await
        .map_err(|e| AppError::Database(e))?;

    Ok(Json(serde_json::json!({
        "status": "SUCCESS",
        "task": "cleanup_fifteen_days_retention",
        "metrics": metrics
    })))
}
