use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    error::AppError,
    modules::{
        auth::backend::AuthSession,
        lms::{application::LmsService, dto::*, repository::LmsRepository},
    },
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct DefaultAttendanceQuery {
    pub class_section_id: Uuid,
    pub subject_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct DailyReportQuery {
    pub date: Option<String>,
    pub teacher_id: Option<Uuid>,
}

pub struct LmsController;

impl LmsController {
    /// Create new LMS task (draft or published)
    pub async fn create_task(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<CreateLmsTaskRequest>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        if !user.capabilities.contains(&"lms:create".to_string())
            && !user.roles.iter().any(|r| {
                r == "SUPER_ADMIN" || r == "SCHOOL_ADMIN" || r == "TEACHER" || r == "CLASS_TEACHER"
            })
        {
            return Err(AppError::Forbidden(
                "Permission denied to create LMS tasks".into(),
            ));
        }

        let res = LmsService::create_task(&state.db, &user, payload).await?;
        Ok((StatusCode::CREATED, Json(res)))
    }

    /// List LMS tasks with filters
    pub async fn list_tasks(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(filter): Query<TaskFilterQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let list = LmsService::list_tasks(&state.db, &user, filter).await?;
        Ok(Json(serde_json::json!({ "tasks": list })))
    }

    /// Retrieve task details
    pub async fn get_task_detail(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(task_id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let item = LmsService::get_task_details(&state.db, &user, task_id).await?;
        Ok(Json(item))
    }

    /// Update a task
    pub async fn update_task(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(task_id): Path<Uuid>,
        Json(payload): Json<UpdateLmsTaskRequest>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let res = LmsService::update_task(&state.db, &user, task_id, payload).await?;
        Ok(Json(res))
    }

    /// Publish a draft task
    pub async fn publish_task(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(task_id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let res = LmsService::publish_task(&state.db, &user, task_id).await?;
        Ok(Json(res))
    }

    /// Cancel a task
    pub async fn cancel_task(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(task_id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let res = LmsService::cancel_task(&state.db, &user, task_id).await?;
        Ok(Json(res))
    }

    /// Add attachment to a task
    pub async fn add_attachment(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(task_id): Path<Uuid>,
        Json(payload): Json<CreateAttachmentInput>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let att =
            LmsRepository::add_attachment(&state.db, user.school_id, task_id, user.id, &payload)
                .await?;
        Ok((StatusCode::CREATED, Json(att)))
    }

    /// Delete attachment from a task
    pub async fn delete_attachment(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path((task_id, attachment_id)): Path<(Uuid, Uuid)>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        LmsRepository::remove_attachment(
            &state.db,
            user.school_id,
            task_id,
            attachment_id,
            user.id,
        )
        .await?;
        Ok(Json(serde_json::json!({ "success": true })))
    }

    /// Retrieve evaluation spreadsheet rows for marks entry
    pub async fn get_evaluation_marks(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(task_id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let rows =
            LmsRepository::get_evaluation_spreadsheet_rows(&state.db, user.school_id, task_id)
                .await?;
        let eval =
            LmsRepository::get_evaluation_by_task_id(&state.db, user.school_id, task_id).await?;

        Ok(Json(serde_json::json!({
            "evaluation": eval,
            "students": rows
        })))
    }

    /// Bulk save evaluation marks from the spreadsheet component
    pub async fn save_evaluation_marks(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(task_id): Path<Uuid>,
        Json(payload): Json<SaveEvaluationMarksRequest>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        if !user.capabilities.contains(&"lms:marks_enter".to_string())
            && !user.roles.iter().any(|r| {
                r == "TEACHER" || r == "CLASS_TEACHER" || r == "SUPER_ADMIN" || r == "SCHOOL_ADMIN"
            })
        {
            return Err(AppError::Forbidden(
                "Permission denied to record evaluation marks".into(),
            ));
        }

        LmsRepository::save_evaluation_marks(
            &state.db,
            user.school_id,
            task_id,
            user.id,
            &payload.marks,
        )
        .await?;
        Ok(Json(serde_json::json!({ "success": true })))
    }

    /// Retrieve overview dashboard stats
    pub async fn get_overview_stats(
        auth_session: AuthSession,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let stats = LmsService::get_overview_stats(&state.db, &user).await?;
        Ok(Json(stats))
    }

    /// Get default attendance session for class & subject
    pub async fn get_default_attendance(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(q): Query<DefaultAttendanceQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let snippet = LmsRepository::get_default_attendance_session(
            &state.db,
            user.school_id,
            user.id,
            q.class_section_id,
            q.subject_id,
        )
        .await?;

        Ok(Json(serde_json::json!({ "session": snippet })))
    }

    /// Get attended classes for the user to populate Create Task class options with auto-fill subject
    pub async fn get_attended_classes(
        auth_session: AuthSession,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let is_admin = user
            .roles
            .iter()
            .any(|r| r == "SUPER_ADMIN" || r == "SCHOOL_ADMIN" || r == "PRINCIPAL");

        let classes =
            LmsRepository::get_attended_classes(&state.db, user.school_id, user.id, is_admin)
                .await?;

        Ok(Json(serde_json::json!({ "classes": classes })))
    }

    /// Get daily teacher task report
    pub async fn get_daily_report(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(q): Query<DailyReportQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let date = match q.date {
            Some(d) => NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                .unwrap_or_else(|_| chrono::Local::now().date_naive()),
            None => chrono::Local::now().date_naive(),
        };

        let is_admin = user
            .roles
            .iter()
            .any(|r| r == "SUPER_ADMIN" || r == "SCHOOL_ADMIN" || r == "PRINCIPAL");
        let teacher_filter = if is_admin {
            q.teacher_id
        } else {
            Some(user.id)
        };

        let report = LmsRepository::get_daily_teacher_report(
            &state.db,
            user.school_id,
            date,
            teacher_filter,
        )
        .await?;

        Ok(Json(serde_json::json!({ "date": date, "report": report })))
    }
}
