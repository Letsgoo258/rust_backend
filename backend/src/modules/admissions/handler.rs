use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use uuid::Uuid;

use super::{
    dto::{CreateAdmissionRequest, UpdateAdmissionStatusRequest},
    repository::AdmissionRepository,
};
use crate::{
    error::AppError,
    modules::auth::backend::AuthSession,
    state::AppState,
    utils::{audit, datetime::DateUtils, write_controller::DbWriteController},
};

#[tracing::instrument(err, skip(state, auth_session, payload))]
pub async fn create_admission(
    auth_session: AuthSession,
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<CreateAdmissionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SCHOOL_ADMIN".to_string())
        && !user.roles.contains(&"RECEPTIONIST".to_string())
        && !user.roles.contains(&"CLERK".to_string())
        && !user.roles.contains(&"SUPER_ADMIN".to_string())
        && !user.roles.contains(&"HEADMASTER".to_string())
        && !user.roles.contains(&"PRINCIPAL".to_string())
    {
        return Err(AppError::Forbidden(
            "Only Admins or Front Office staff can create admissions".into(),
        ));
    }

    let dob =
        DateUtils::parse_iso_date(&payload.date_of_birth).map_err(|e| AppError::BadRequest(e))?;

    let _lock = state
        .key_locks
        .acquire_lock(&(
            user.school_id,
            format!("admission:new:{}", payload.applicant_last_name),
        ))
        .await;

    let mut tx_guard = DbWriteController::begin(&state.db).await?;
    let admission_id = AdmissionRepository::create_admission_submission(
        tx_guard.conn(),
        user.school_id,
        &payload.applicant_first_name,
        payload.applicant_middle_name.as_deref(),
        &payload.applicant_last_name,
        dob,
        payload.applicant_gender.as_deref(),
        payload.applicant_phone.as_deref(),
        payload.applicant_email.as_deref(),
        &payload.guardian_name,
        &payload.guardian_relationship,
        &payload.guardian_phone,
        payload.guardian_email.as_deref(),
        payload.guardian_occupation.as_deref(),
        &payload.custom_fields,
        user.id,
    )
    .await?;
    tx_guard.commit().await?;

    audit::log_action(
        &state.db,
        Some(user.school_id),
        Some(user.id),
        "CREATE_ADMISSION_SRD",
        None,
        None,
        None,
        Some(addr.to_string()),
    )
    .await
    .ok();

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "message": "Admission submitted successfully",
            "admission_id": admission_id
        })),
    ))
}

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn enroll_admission(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(admission_id): Path<Uuid>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SCHOOL_ADMIN".to_string())
        && !user.roles.contains(&"SUPER_ADMIN".to_string())
        && !user.roles.contains(&"HEADMASTER".to_string())
        && !user.roles.contains(&"PRINCIPAL".to_string())
    {
        return Err(AppError::Forbidden(
            "Only School Admins can officially enroll students from admissions".into(),
        ));
    }

    let _lock = state
        .key_locks
        .acquire_lock(&(user.school_id, format!("admission:enroll:{}", admission_id)))
        .await;

    let mut tx_guard = DbWriteController::begin(&state.db).await?;
    let admission_number = AdmissionRepository::enroll_admission(
        tx_guard.conn(),
        user.school_id,
        admission_id,
        user.id,
    )
    .await?;
    tx_guard.commit().await?;

    audit::log_action(
        &state.db,
        Some(user.school_id),
        Some(user.id),
        "SYSTEM_ENROLL_ADMISSION",
        None,
        None,
        None,
        Some(addr.to_string()),
    )
    .await
    .ok();

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "message": "Student successfully enrolled from admission",
            "admission_number": admission_number
        })),
    ))
}

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn list_admissions(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let status_filter = query.get("status").map(|s| s.as_str());
    let admissions =
        AdmissionRepository::list_admissions(&state.db, user.school_id, status_filter).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "admissions": admissions,
            "total": admissions.len()
        })),
    ))
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
pub async fn update_admission_status(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(admission_id): Path<Uuid>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<UpdateAdmissionStatusRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SCHOOL_ADMIN".to_string())
        && !user.roles.contains(&"SUPER_ADMIN".to_string())
        && !user.roles.contains(&"HEADMASTER".to_string())
        && !user.roles.contains(&"PRINCIPAL".to_string())
    {
        return Err(AppError::Forbidden(
            "Only School Admins can update admission status".into(),
        ));
    }

    let mut tx_guard = DbWriteController::begin(&state.db).await?;
    AdmissionRepository::update_status(
        tx_guard.conn(),
        user.school_id,
        admission_id,
        &payload.status,
        user.id,
        payload.reason.as_deref(),
    )
    .await?;
    tx_guard.commit().await?;

    audit::log_action(
        &state.db,
        Some(user.school_id),
        Some(user.id),
        "UPDATE_ADMISSION_STATUS",
        None,
        None,
        None,
        Some(addr.to_string()),
    )
    .await
    .ok();

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "message": "Admission status updated successfully"
        })),
    ))
}
