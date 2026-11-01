use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::timetable::generator::AutoGenerateRequest;
use crate::modules::academic::timetable::service::{
    BatchSaveSlotsPayload, CheckClashRequest, CreateSubstitutionInput, TimetableService,
};
use crate::modules::auth::backend::AuthSession;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct DateQuery {
    pub date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TeacherSlotsQuery {
    pub teacher_id: Uuid,
    pub day_of_week: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct AvailableTeachersQuery {
    pub date: String,
    pub day_of_week: i32,
    pub period_number: i32,
    pub exclude_staff_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct TodayScheduleQuery {
    pub date: Option<String>,
    pub class_section_id: Option<Uuid>,
    pub teacher_staff_id: Option<Uuid>,
}

pub struct TimetableHandler;

impl TimetableHandler {
    /// GET /api/v1/academic/timetable/section/:id
    pub async fn get_section_timetable(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(class_section_id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let slots =
            TimetableService::get_section_timetable(&state.db, user.school_id, class_section_id)
                .await?;

        Ok((StatusCode::OK, Json(serde_json::json!({ "slots": slots }))))
    }

    /// GET /api/v1/academic/timetable/teacher-slots
    pub async fn get_teacher_slots(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(q): Query<TeacherSlotsQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let slots = TimetableService::get_teacher_slots(
            &state.db,
            user.school_id,
            q.teacher_id,
            q.day_of_week,
        )
        .await?;

        Ok((StatusCode::OK, Json(serde_json::json!({ "slots": slots }))))
    }

    /// POST /api/v1/academic/timetable/slots
    pub async fn batch_upsert_slots(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<BatchSaveSlotsPayload>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        if !user.is_school_admin() && !user.has_capability("academics:manage_curriculum") {
            return Err(AppError::Forbidden(
                "Only School Admins or authorized faculty can edit timetable".into(),
            ));
        }

        let saved =
            TimetableService::batch_upsert_slots(&state.db, user.school_id, payload).await?;

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "message": format!("Successfully saved {} timetable slots", saved),
                "count": saved
            })),
        ))
    }

    /// POST /api/v1/academic/timetable/check-clash
    pub async fn check_clash(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(req): Json<CheckClashRequest>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let res = TimetableService::check_clashes(&state.db, user.school_id, &req).await?;
        Ok((StatusCode::OK, Json(res)))
    }

    /// DELETE /api/v1/academic/timetable/slots/:id
    pub async fn delete_slot(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(slot_id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        if !user.is_school_admin() && !user.has_capability("academics:manage_curriculum") {
            return Err(AppError::Forbidden(
                "Only School Admins or authorized faculty can delete timetable slots".into(),
            ));
        }

        TimetableService::delete_slot(&state.db, user.school_id, slot_id).await?;

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "message": "Timetable slot deleted successfully" })),
        ))
    }

    /// POST /api/v1/academic/timetable/auto-generate
    pub async fn auto_generate_preview(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(req): Json<AutoGenerateRequest>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        if !user.is_school_admin() && !user.has_capability("academics:manage_curriculum") {
            return Err(AppError::Forbidden(
                "Only School Admins or authorized faculty can auto-generate timetable".into(),
            ));
        }

        let preview =
            TimetableService::auto_generate_preview(&state.db, user.school_id, req).await?;

        Ok((StatusCode::OK, Json(preview)))
    }

    /// GET /api/v1/academic/timetable/substitutions
    pub async fn list_substitutions(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(q): Query<DateQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let date = match q.date {
            Some(d) => NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                .map_err(|e| AppError::BadRequest(format!("Invalid date: {e}")))?,
            None => chrono::Utc::now().date_naive(),
        };

        let list = TimetableService::list_substitutions(&state.db, user.school_id, date).await?;

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "substitutions": list })),
        ))
    }

    /// POST /api/v1/academic/timetable/substitutions
    pub async fn create_substitution(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(input): Json<CreateSubstitutionInput>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        if !user.is_school_admin() && !user.has_capability("academics:manage_curriculum") {
            return Err(AppError::Forbidden(
                "Only School Admins or authorized staff can assign substitutions".into(),
            ));
        }

        let sub_id =
            TimetableService::create_substitution(&state.db, user.school_id, user.id, input)
                .await?;

        Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({
                "message": "Substitution assigned and synced to today's schedule successfully",
                "substitution_id": sub_id
            })),
        ))
    }

    /// DELETE /api/v1/academic/timetable/substitutions/:id
    pub async fn cancel_substitution(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(sub_id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        if !user.is_school_admin() && !user.has_capability("academics:manage_curriculum") {
            return Err(AppError::Forbidden(
                "Only School Admins or authorized staff can cancel substitutions".into(),
            ));
        }

        TimetableService::cancel_substitution(&state.db, user.school_id, sub_id).await?;

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "message": "Substitution cancelled successfully" })),
        ))
    }

    /// GET /api/v1/academic/timetable/substitutions/available-teachers
    pub async fn get_available_substitute_teachers(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(q): Query<AvailableTeachersQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let date = NaiveDate::parse_from_str(&q.date, "%Y-%m-%d")
            .map_err(|e| AppError::BadRequest(format!("Invalid date: {e}")))?;

        let teachers = TimetableService::get_available_substitute_teachers(
            &state.db,
            user.school_id,
            date,
            q.day_of_week,
            q.period_number,
            q.exclude_staff_id,
        )
        .await?;

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "teachers": teachers })),
        ))
    }

    /// GET /api/v1/academic/timetable/today
    pub async fn get_today_schedule(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(q): Query<TodayScheduleQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let date = match q.date {
            Some(d) => NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                .map_err(|e| AppError::BadRequest(format!("Invalid date: {e}")))?,
            None => chrono::Utc::now().date_naive(),
        };

        let schedule = TimetableService::get_today_schedule(
            &state.db,
            user.school_id,
            date,
            q.class_section_id,
            q.teacher_staff_id,
        )
        .await?;

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "schedule": schedule })),
        ))
    }

    /// POST /api/v1/academic/timetable/daily/sync
    pub async fn sync_daily_timetable(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(q): Query<DateQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        if !user.is_school_admin() && !user.has_capability("academics:manage_curriculum") {
            return Err(AppError::Forbidden(
                "Only School Admins can trigger schedule synchronization".into(),
            ));
        }

        let date = match q.date {
            Some(d) => NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                .map_err(|e| AppError::BadRequest(format!("Invalid date: {e}")))?,
            None => chrono::Utc::now().date_naive(),
        };

        let count = TimetableService::sync_daily_timetable_for_date(&state.db, date).await?;

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "message": format!("Daily schedule synced successfully: {count} slots populated/updated"),
                "date": date.to_string(),
                "count": count
            })),
        ))
    }

    /// GET /api/v1/academic/timetable/config
    pub async fn get_timetable_config(
        auth_session: AuthSession,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let mut config = TimetableService::get_timetable_config(&state.db, user.school_id).await?;

        // If config was saved double-nested under {"config": {...}}, unwrap it
        if let Some(inner) = config.get("config") {
            if inner.is_object() {
                config = inner.clone();
            }
        }

        // Return flattened config with "config" alias for 100% compatibility across all client versions
        let mut response = config.clone();
        if let serde_json::Value::Object(ref mut map) = response {
            map.insert("config".to_string(), config);
        }

        Ok((StatusCode::OK, Json(response)))
    }

    /// POST /api/v1/academic/timetable/config
    pub async fn save_timetable_config(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<serde_json::Value>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let is_admin = user
            .roles
            .iter()
            .any(|r| r.eq_ignore_ascii_case("SUPER_ADMIN"))
            || user.is_school_admin()
            || user.has_capability("academics:manage_curriculum");

        if !is_admin {
            return Err(AppError::Forbidden(
                "Only School Admins can modify timetable configuration".into(),
            ));
        }

        // Clean unwrap if payload was wrapped in {"config": ...}
        let config_to_save = if let Some(inner) = payload.get("config") {
            if inner.is_object() {
                inner.clone()
            } else {
                payload
            }
        } else {
            payload
        };

        TimetableService::save_timetable_config(&state.db, user.school_id, config_to_save).await?;

        Ok((
            StatusCode::OK,
            Json(
                serde_json::json!({ "message": "School timetable configuration saved successfully" }),
            ),
        ))
    }
}
