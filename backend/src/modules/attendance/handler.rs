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
        attendance::{
            dto::{
                AttendanceConfigDto, BulkEditAttendanceRequest, RaiseAttendanceTicketRequest,
                RecordAttendanceRequest, RegisterQuery, ResolveCollisionRequest,
                UpdateSingleRecordRequest, WeeklyHeatmapQuery,
            },
            service::AttendanceService,
        },
        auth::backend::AuthSession,
    },
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct ListSessionsQuery {
    pub date: Option<String>,
    pub class_section_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct RosterQuery {
    pub subject_id: Option<Uuid>,
}

pub struct AttendanceHandler;

impl AttendanceHandler {
    /// Retrieve enrolled students for roll-call, optionally filtered by subject
    pub async fn get_class_roster(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(class_section_id): Path<Uuid>,
        Query(q): Query<RosterQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let roster = AttendanceService::get_class_roster(
            &state.db,
            user.school_id,
            class_section_id,
            q.subject_id,
        )
        .await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "roster": roster,
            "count": roster.len()
        })))
    }

    /// Record initial class or period attendance
    pub async fn record_attendance(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<RecordAttendanceRequest>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let session_id = AttendanceService::record_attendance(&state.db, &user, payload).await?;

        Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({
                "success": true,
                "message": "Attendance recorded successfully",
                "session_id": session_id
            })),
        ))
    }

    /// Administrative bulk edit of student records with mandatory reason
    pub async fn bulk_edit_attendance(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<BulkEditAttendanceRequest>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let updated_count =
            AttendanceService::bulk_edit_attendance(&state.db, &user, payload).await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "message": format!("Successfully updated {} student attendance records.", updated_count),
            "updated_count": updated_count
        })))
    }

    /// Update a single attendance record
    pub async fn update_record(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(record_id): Path<Uuid>,
        Json(payload): Json<UpdateSingleRecordRequest>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        AttendanceService::update_single_record(&state.db, &user, record_id, payload).await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Attendance record updated successfully."
        })))
    }

    /// Search and list recorded sessions
    pub async fn list_sessions(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(params): Query<ListSessionsQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let date_opt = params
            .date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let sessions =
            AttendanceService::list_sessions(&state.db, &user, date_opt, params.class_section_id)
                .await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "sessions": sessions,
            "count": sessions.len()
        })))
    }

    /// Get session details with records and correction logs
    pub async fn get_session_detail(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(session_id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let detail = AttendanceService::get_session_detail(&state.db, &user, session_id).await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "data": detail
        })))
    }

    /// Live KPIs for attendance dashboard
    pub async fn get_today_stats(
        auth_session: AuthSession,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let stats = AttendanceService::get_today_stats(&state.db, &user).await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "date": stats.date,
            "total_students": stats.total_enrolled,
            "total_enrolled": stats.total_enrolled,
            "present_count": stats.present_students,
            "present_students": stats.present_students,
            "absent_count": stats.absent_students,
            "absent_students": stats.absent_students,
            "late_count": stats.late_students,
            "late_students": stats.late_students,
            "excused_count": 0,
            "sessions_recorded": stats.completed_sections,
            "completed_sections": stats.completed_sections,
            "total_expected_sessions": stats.total_sections,
            "total_sections": stats.total_sections,
            "pending_sections": stats.pending_sections,
            "open_collisions_count": stats.collision_alerts_count,
            "collision_alerts_count": stats.collision_alerts_count,
            "attendance_percentage": stats.attendance_percentage,
            "substitution_deadlocks_count": stats.substitution_deadlocks_count,
            "stats": stats
        })))
    }

    /// Retrieve flat attendance records for daily register view
    pub async fn get_register(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(params): Query<RegisterQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let date_opt = params
            .date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let records =
            AttendanceService::get_register(&state.db, &user, date_opt, params.class_section_id)
                .await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "records": records,
            "count": records.len()
        })))
    }

    /// List collision incidents and tickets
    pub async fn list_collisions(
        auth_session: AuthSession,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let collisions = AttendanceService::list_collisions(&state.db, &user).await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "collisions": collisions,
            "count": collisions.len()
        })))
    }

    /// Resolve an attendance collision ticket
    pub async fn resolve_collision(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(collision_id): Path<Uuid>,
        Json(payload): Json<ResolveCollisionRequest>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        AttendanceService::resolve_collision(&state.db, &user, collision_id, payload).await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Collision ticket resolved successfully."
        })))
    }

    /// Raise an attendance collision / conflict ticket
    pub async fn raise_collision_ticket(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<RaiseAttendanceTicketRequest>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let ticket_number =
            AttendanceService::raise_collision_ticket(&state.db, &user, payload).await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "ticket_number": ticket_number,
            "message": format!("Verification ticket #{} has been raised for administrative resolution.", ticket_number)
        })))
    }

    /// Get attendance configuration
    pub async fn get_config(
        auth_session: AuthSession,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let config = AttendanceService::get_config(&state.db, &user).await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "config": config
        })))
    }

    /// Update attendance configuration
    pub async fn update_config(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<AttendanceConfigDto>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        AttendanceService::update_config(&state.db, &user, payload).await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Attendance policy settings updated successfully."
        })))
    }

    /// Get today's effective slots for teacher
    pub async fn get_today_teacher_slots(
        auth_session: AuthSession,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let staff_id_opt: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM staff WHERE user_id = $1 AND school_id = $2")
                .bind(user.id)
                .bind(user.school_id)
                .fetch_optional(&state.db)
                .await?;

        let today = chrono::Local::now().date_naive();
        let slots =
            crate::modules::academic::timetable::service::TimetableService::get_today_schedule(
                &state.db,
                user.school_id,
                today,
                None,
                staff_id_opt,
            )
            .await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "slots": slots
        })))
    }

    /// Delete an attendance session and its records
    pub async fn delete_session(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(session_id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        AttendanceService::delete_session(&state.db, &user, session_id).await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Attendance session and associated student records deleted successfully."
        })))
    }

    /// Class-wise daily attendance heat map for a date range
    pub async fn get_weekly_heatmap(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(params): Query<WeeklyHeatmapQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let start_date_opt = params
            .start_date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let end_date_opt = params
            .end_date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let heatmap =
            AttendanceService::get_weekly_heatmap(&state.db, &user, start_date_opt, end_date_opt)
                .await?;

        Ok(Json(heatmap))
    }
}
