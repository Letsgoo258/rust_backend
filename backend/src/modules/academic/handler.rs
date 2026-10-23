use crate::{
    error::AppError, modules::academic::repository::AcademicRepository,
    modules::auth::backend::AuthSession, state::AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateAcademicYearPayload {
    pub name: String,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Deserialize)]
pub struct ClassInput {
    pub name: String,
    pub code: String,
    pub sequence: Option<i32>,
    pub sections: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct BulkClassesPayload {
    pub classes: Vec<ClassInput>,
}

#[derive(Debug, Deserialize)]
pub struct SubjectInput {
    pub name: String,
    pub code: String,
    pub subject_type: Option<String>,
    pub system_subject_type: Option<String>,
    pub subject_type_id: Option<uuid::Uuid>,
    pub color: Option<String>,
    pub department: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BulkSubjectsPayload {
    pub subjects: Vec<SubjectInput>,
}

#[derive(Debug, Deserialize)]
pub struct SubjectTypeInput {
    pub name: String,
    pub code: String,
    pub system_subject_type: Option<String>,
    pub is_elective: Option<bool>,
    pub is_mandatory: Option<bool>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BulkSubjectTypesPayload {
    pub subject_types: Vec<SubjectTypeInput>,
}

#[derive(Debug, Deserialize)]
pub struct AssignStudentsBulkPayload {
    pub class_id: Option<uuid::Uuid>,
    pub section_id: Option<uuid::Uuid>,
    pub target_class_section_id: Option<uuid::Uuid>,
    pub admission_numbers: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct LookupAdmissionNumbersPayload {
    pub admission_numbers: Vec<String>,
    pub target_class_section_id: uuid::Uuid,
}

#[derive(Debug, serde::Serialize)]
pub struct LookupReport {
    pub found: Vec<crate::modules::academic::repository::LookupStudentRow>,
    pub not_found: Vec<String>,
    pub clashes: Vec<crate::modules::academic::repository::LookupStudentRow>,
    pub ready: Vec<crate::modules::academic::repository::LookupStudentRow>,
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
pub async fn create_academic_year(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<CreateAcademicYearPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.is_school_admin() && !user.has_capability("academics:manage_curriculum") {
        return Err(AppError::Forbidden(
            "Only School Admins or authorized faculty can manage academic years".into(),
        ));
    }

    let start_date = crate::utils::datetime::DateUtils::parse_iso_date(&payload.start_date)
        .map_err(|e| AppError::BadRequest(format!("Invalid start date: {}", e)))?;

    let end_date = crate::utils::datetime::DateUtils::parse_iso_date(&payload.end_date)
        .map_err(|e| AppError::BadRequest(format!("Invalid end date: {}", e)))?;

    let academic_year_id = AcademicRepository::create_academic_year(
        &state.db,
        user.school_id,
        payload.name.trim(),
        start_date,
        end_date,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "message": "Academic year created successfully",
            "academic_year_id": academic_year_id
        })),
    ))
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
pub async fn create_bulk_classes(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<BulkClassesPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.is_school_admin() && !user.has_capability("academics:manage_curriculum") {
        return Err(AppError::Forbidden(
            "Only School Admins or authorized faculty can manage classes".into(),
        ));
    }

    let active_ay_id =
        AcademicRepository::get_active_academic_year_id(&state.db, user.school_id).await?;

    let _lock = state
        .key_locks
        .acquire_lock(&(user.school_id, "academic:classes:bulk"))
        .await;

    let tx_guard = crate::utils::write_controller::DbWriteController::begin(&state.db).await?;
    let created_count = AcademicRepository::batch_upsert_classes(
        &state.db,
        user.school_id,
        active_ay_id,
        payload.classes,
    )
    .await?;
    tx_guard.commit().await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "message": format!("Successfully processed {} classes", created_count)
        })),
    ))
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
pub async fn create_bulk_subjects(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<BulkSubjectsPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.is_school_admin() && !user.has_capability("academics:manage_curriculum") {
        return Err(AppError::Forbidden(
            "Only School Admins or authorized faculty can manage subjects".into(),
        ));
    }

    let _lock = state
        .key_locks
        .acquire_lock(&(user.school_id, "academic:subjects:bulk"))
        .await;

    let tx_guard = crate::utils::write_controller::DbWriteController::begin(&state.db).await?;
    let processed_count =
        AcademicRepository::batch_upsert_subjects(&state.db, user.school_id, payload.subjects)
            .await?;
    tx_guard.commit().await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "message": format!("Successfully added {} subjects", processed_count)
        })),
    ))
}

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn get_class_capacity(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let classes = AcademicRepository::get_class_capacities(&state.db, user.school_id).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "classes": classes })),
    ))
}

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn get_school_stats(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let stats = AcademicRepository::get_school_stats(&state.db, user.school_id).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::to_value(stats).unwrap_or_default()),
    ))
}

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn get_subjects(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let subjects = AcademicRepository::get_subjects(&state.db, user.school_id).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "subjects": subjects })),
    ))
}

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn delete_subject(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.is_school_admin() && !user.has_capability("academics:manage_curriculum") {
        return Err(AppError::Forbidden(
            "Only School Admins or authorized faculty can delete subjects".into(),
        ));
    }

    let deleted = AcademicRepository::delete_subject(&state.db, user.school_id, id).await?;
    if !deleted {
        return Err(AppError::NotFound("Subject not found".into()));
    }

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "message": "Subject deleted successfully" })),
    ))
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
pub async fn create_bulk_subject_types(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<BulkSubjectTypesPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.is_school_admin() && !user.has_capability("academics:manage_curriculum") {
        return Err(AppError::Forbidden(
            "Only School Admins or authorized faculty can manage subject types".into(),
        ));
    }

    let _lock = state
        .key_locks
        .acquire_lock(&(user.school_id, "academic:subject_types:bulk"))
        .await;

    let processed_count = AcademicRepository::batch_upsert_subject_types(
        &state.db,
        user.school_id,
        payload.subject_types,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "message": format!("Successfully processed {} subject types", processed_count)
        })),
    ))
}

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn get_subject_types(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let subject_types = AcademicRepository::get_subject_types(&state.db, user.school_id).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "subject_types": subject_types })),
    ))
}

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn delete_subject_type(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.is_school_admin() && !user.has_capability("academics:manage_curriculum") {
        return Err(AppError::Forbidden(
            "Only School Admins or authorized faculty can delete subject types".into(),
        ));
    }

    let deleted = AcademicRepository::delete_subject_type(&state.db, user.school_id, id).await?;
    if !deleted {
        return Err(AppError::NotFound("Subject type not found".into()));
    }

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "message": "Subject type deleted successfully" })),
    ))
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
pub async fn assign_students_bulk(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<AssignStudentsBulkPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.is_school_admin() && !user.has_capability("academics:assign_students") {
        return Err(AppError::Forbidden(
            "Only School Admins or authorized faculty can assign students".into(),
        ));
    }

    let ay_id = AcademicRepository::get_active_academic_year_id(&state.db, user.school_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(
                "No active academic year found. Please complete Step 3 first.".into(),
            )
        })?;

    let class_section_id = match payload.target_class_section_id {
        Some(id) => id,
        None => {
            let class_id = payload.class_id.ok_or_else(|| {
                AppError::BadRequest("class_id or target_class_section_id required".into())
            })?;
            let section_id = payload.section_id.ok_or_else(|| {
                AppError::BadRequest("section_id or target_class_section_id required".into())
            })?;
            AcademicRepository::get_or_create_class_section(&state.db, ay_id, class_id, section_id)
                .await?
        }
    };

    let _lock = state
        .key_locks
        .acquire_lock(&(
            user.school_id,
            format!("academic:assign:{}", class_section_id),
        ))
        .await;

    let tx_guard = crate::utils::write_controller::DbWriteController::begin(&state.db).await?;
    let (assigned_count, unmatched) = AcademicRepository::batch_assign_students(
        &state.db,
        user.school_id,
        ay_id,
        class_section_id,
        payload.admission_numbers,
    )
    .await?;
    tx_guard.commit().await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "message": format!("Successfully assigned {} student(s) to target class section.", assigned_count),
            "assigned_count": assigned_count,
            "unmatched_admissions": unmatched
        })),
    ))
}

#[derive(Debug, Deserialize)]
pub struct UpdateClassSectionPayload {
    pub class_section_id: uuid::Uuid,
    pub capacity: Option<i32>,
    pub room_id: Option<uuid::Uuid>,
    pub class_teacher_staff_id: Option<uuid::Uuid>,
    pub status: Option<String>,
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
pub async fn update_class_section(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<UpdateClassSectionPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.is_school_admin() && !user.has_capability("academics:manage_curriculum") {
        return Err(AppError::Forbidden(
            "Only School Admins or authorized staff can update class sections".into(),
        ));
    }

    let current = AcademicRepository::get_class_section_detail(
        &state.db,
        payload.class_section_id,
        user.school_id,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Class section not found".into()))?;

    let mut tx = state.db.begin().await?;

    AcademicRepository::update_class_section(
        &mut tx,
        payload.class_section_id,
        payload.capacity,
        payload.room_id,
        payload.class_teacher_staff_id,
        payload.status.as_deref(),
        user.id,
    )
    .await?;

    if let Some(new_cap) = payload.capacity {
        let old_val = current
            .capacity
            .map(|c| c.to_string())
            .unwrap_or_else(|| "Uncapped".into());
        if current.capacity != Some(new_cap) {
            AcademicRepository::record_class_section_history(
                &mut tx,
                user.school_id,
                payload.class_section_id,
                user.id,
                "CAPACITY_CHANGE",
                Some(&old_val),
                Some(&new_cap.to_string()),
            )
            .await?;
        }
    }

    if let Some(ref new_status) = payload.status {
        let old_val = current.status.as_deref().unwrap_or("ACTIVE");
        if old_val != new_status {
            AcademicRepository::record_class_section_history(
                &mut tx,
                user.school_id,
                payload.class_section_id,
                user.id,
                "STATUS_CHANGE",
                Some(old_val),
                Some(new_status),
            )
            .await?;
        }
    }

    if payload.room_id.is_some() && payload.room_id != current.room_id {
        let old_val = current
            .room_id
            .map(|r| r.to_string())
            .unwrap_or_else(|| "None".into());
        let new_val = payload
            .room_id
            .map(|r| r.to_string())
            .unwrap_or_else(|| "None".into());
        AcademicRepository::record_class_section_history(
            &mut tx,
            user.school_id,
            payload.class_section_id,
            user.id,
            "ROOM_CHANGE",
            Some(&old_val),
            Some(&new_val),
        )
        .await?;
    }

    if payload.class_teacher_staff_id.is_some()
        && payload.class_teacher_staff_id != current.class_teacher_staff_id
    {
        let old_val = current
            .class_teacher_staff_id
            .map(|t| t.to_string())
            .unwrap_or_else(|| "None".into());
        let new_val = payload
            .class_teacher_staff_id
            .map(|t| t.to_string())
            .unwrap_or_else(|| "None".into());
        AcademicRepository::record_class_section_history(
            &mut tx,
            user.school_id,
            payload.class_section_id,
            user.id,
            "TEACHER_CHANGE",
            Some(&old_val),
            Some(&new_val),
        )
        .await?;
    }

    let before_json = serde_json::json!({
        "capacity": current.capacity,
        "room_id": current.room_id,
        "class_teacher_staff_id": current.class_teacher_staff_id,
        "status": current.status
    });

    let after_json = serde_json::json!({
        "capacity": payload.capacity.or(current.capacity),
        "room_id": payload.room_id.or(current.room_id),
        "class_teacher_staff_id": payload.class_teacher_staff_id.or(current.class_teacher_staff_id),
        "status": payload.status.clone().or(current.status)
    });

    if let Ok(history_id) = crate::utils::audit::record_entity_change(
        &state.db,
        user.school_id,
        "CLASS_SECTION",
        payload.class_section_id,
        "UPDATE",
        Some(user.id),
        Some(before_json),
        Some(after_json),
        None,
        None,
    )
    .await
    {
        let _ = AcademicRepository::set_class_section_last_history_id(
            &mut tx,
            payload.class_section_id,
            history_id,
        )
        .await;
    }

    tx.commit().await?;

    Ok((
        StatusCode::OK,
        Json(
            serde_json::json!({ "message": "Class section updated successfully and change recorded" }),
        ),
    ))
}

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn get_class_section_history(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(class_section_id): Path<uuid::Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let history =
        AcademicRepository::get_class_section_history(&state.db, class_section_id, user.school_id)
            .await?;

    let list: Vec<serde_json::Value> = history
        .into_iter()
        .map(|h| {
            serde_json::json!({
                "id": h.id,
                "change_type": h.change_type,
                "old_value": h.old_value,
                "new_value": h.new_value,
                "created_at": h.created_at,
                "updated_by": {
                    "id": h.updated_by_id,
                    "name": h.updated_by_name.unwrap_or_default()
                }
            })
        })
        .collect();

    Ok((StatusCode::OK, Json(serde_json::json!({ "history": list }))))
}

pub async fn get_academics_dashboard_config(
    auth_session: AuthSession,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let config = crate::config::dashboard::DashboardConfig::get_academics_dashboard(&user);
    Ok((StatusCode::OK, Json(config)))
}

#[derive(Debug, Deserialize)]
pub struct BulkAssignClassPayload {
    pub class_section_id: uuid::Uuid,
    pub student_ids: Vec<uuid::Uuid>,
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
pub async fn lookup_admission_numbers(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<LookupAdmissionNumbersPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let ay_id = AcademicRepository::get_active_academic_year_id(&state.db, user.school_id)
        .await?
        .ok_or_else(|| AppError::NotFound("No active academic year found".into()))?;

    let found = AcademicRepository::lookup_admission_numbers(
        &state.db,
        user.school_id,
        ay_id,
        &payload.admission_numbers,
    )
    .await?;

    let found_adm_nos: std::collections::HashSet<String> =
        found.iter().map(|f| f.admission_number.clone()).collect();
    let not_found: Vec<String> = payload
        .admission_numbers
        .into_iter()
        .filter(|n| !found_adm_nos.contains(n))
        .collect();

    let mut clashes = Vec::new();
    let mut ready = Vec::new();

    for f in &found {
        if let Some(cs_id) = f.current_class_section_id {
            if cs_id != payload.target_class_section_id {
                clashes.push(f.clone());
            } else {
                // Already in this section, safe to "ready" or ignore. We'll put them in ready.
                ready.push(f.clone());
            }
        } else {
            ready.push(f.clone());
        }
    }

    let report = LookupReport {
        found,
        not_found,
        clashes,
        ready,
    };

    Ok((StatusCode::OK, Json(report)))
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
pub async fn bulk_assign_class(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<BulkAssignClassPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.is_school_admin() && !user.has_capability("academics:assign_students") {
        return Err(AppError::Forbidden("Unauthorized".into()));
    }

    let ay_id = AcademicRepository::get_active_academic_year_id(&state.db, user.school_id)
        .await?
        .ok_or_else(|| AppError::NotFound("No active academic year found".into()))?;

    AcademicRepository::assign_students_bulk_by_id(
        &state.db,
        user.school_id,
        ay_id,
        payload.class_section_id,
        &payload.student_ids,
    )
    .await?;

    Ok((StatusCode::OK, Json(serde_json::json!({ "success": true }))))
}
