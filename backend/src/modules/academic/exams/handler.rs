use crate::{
    error::AppError,
    modules::{
        academic::exams::{
            dto::{
                AddClassExamSchedulePayload, AssignmentFilterQuery, CreateAssignmentPayload,
                CreateExamEventPayload, CreateExamPayload, ExamFilterQuery,
                SaveClassExamTimetablePayload, SaveMarksPayload,
            },
            repository::ExamRepository,
        },
        auth::backend::AuthSession,
    },
    state::AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ExamEventQuery {
    pub academic_year_id: Option<Uuid>,
    pub search: Option<String>,
}

pub struct ExamHandler;

impl ExamHandler {
    /// POST /api/v1/academic/exam-events
    /// Strictly School Admins and Super Admins
    pub async fn create_exam_event(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<CreateExamEventPayload>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        if !user.is_school_admin() {
            return Err(AppError::Forbidden(
                "Access denied: Creating exam events is strictly reserved for School Administrators."
                    .into(),
            ));
        }

        let event =
            ExamRepository::create_exam_event(&state.db, user.school_id, user.id, payload).await?;
        Ok((StatusCode::CREATED, Json(event)))
    }

    /// GET /api/v1/academic/exam-events
    pub async fn list_exam_events(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(query): Query<ExamEventQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let events = ExamRepository::list_exam_events(
            &state.db,
            user.school_id,
            query.academic_year_id,
            query.search,
        )
        .await?;
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "events": events })),
        ))
    }

    /// GET /api/v1/academic/exam-events/:id
    pub async fn get_exam_event_detail(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let event = ExamRepository::get_exam_event_detail(&state.db, user.school_id, id).await?;
        Ok((StatusCode::OK, Json(event)))
    }

    /// GET /api/v1/academic/class-section/:id/core-subjects
    /// Returns ALL subjects (Core + Elective) for the class section for exam scheduling
    pub async fn get_section_core_subjects(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(class_section_id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let subjects =
            ExamRepository::get_section_all_subjects(&state.db, user.school_id, class_section_id)
                .await?;

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "subjects": subjects })),
        ))
    }

    /// POST /api/v1/academic/exam-events/:event_id/classes/:class_section_id/timetable
    /// Save per-subject timetable slots for a class (each subject its own date/time/marks)
    pub async fn save_class_timetable(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path((event_id, class_section_id)): Path<(Uuid, Uuid)>,
        Json(payload): Json<SaveClassExamTimetablePayload>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let saved = ExamRepository::save_class_exam_timetable(
            &state.db,
            user.school_id,
            user.id,
            event_id,
            class_section_id,
            payload,
        )
        .await?;

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "saved": saved, "message": "Timetable saved successfully" })),
        ))
    }

    /// POST /api/v1/academic/exam-events/:event_id/classes/:class_section_id/publish-whatsapp
    /// Sends a single consolidated WhatsApp message for this class with the full exam schedule
    pub async fn publish_class_whatsapp(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path((event_id, class_section_id)): Path<(Uuid, Uuid)>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let sent = ExamRepository::publish_class_timetable_whatsapp(
            &state.db,
            user.school_id,
            event_id,
            class_section_id,
        )
        .await?;

        Ok((
            StatusCode::OK,
            Json(
                serde_json::json!({ "sent": sent, "message": format!("Timetable published to {sent} recipient(s)") }),
            ),
        ))
    }

    /// POST /api/v1/academic/exam-events/:id/classes
    /// Allowed for School Admins, Class Teachers, Subject Teachers, or users with schedule capabilities
    pub async fn add_class_schedule(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(event_id): Path<Uuid>,
        Json(payload): Json<AddClassExamSchedulePayload>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let is_admin = user.is_school_admin();
        let can_manage = ExamRepository::can_user_manage_exam_schedule(
            &state.db,
            user.school_id,
            user.id,
            is_admin,
            Some(payload.class_section_id),
            payload.subject_id,
        )
        .await?;

        if !can_manage {
            return Err(AppError::Forbidden(
                "Access denied: You do not have permission to configure exam timetable for this class or subject.".into()
            ));
        }

        let schedules = ExamRepository::add_class_schedule_to_event(
            &state.db,
            user.school_id,
            user.id,
            event_id,
            payload,
        )
        .await?;

        Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({ "schedules": schedules })),
        ))
    }

    /// DELETE /api/v1/academic/exam-events/:event_id/classes/:schedule_id
    pub async fn delete_class_schedule(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path((_event_id, schedule_id)): Path<(Uuid, Uuid)>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let is_admin = user.is_school_admin();
        let exam = ExamRepository::get_exam_by_id(&state.db, user.school_id, schedule_id).await?;

        let can_manage = ExamRepository::can_user_manage_exam_schedule(
            &state.db,
            user.school_id,
            user.id,
            is_admin,
            Some(exam.class_section_id),
            Some(exam.subject_id),
        )
        .await?;

        if !can_manage {
            return Err(AppError::Forbidden(
                "Access denied: You do not have permission to remove this exam schedule.".into(),
            ));
        }

        ExamRepository::delete_exam_schedule(&state.db, user.school_id, schedule_id).await?;
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "message": "Exam schedule removed successfully" })),
        ))
    }

    /// POST /api/v1/academic/exams
    pub async fn create_exam(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<CreateExamPayload>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let is_admin = user.is_school_admin()
            || user.roles.iter().any(|r| {
                r.eq_ignore_ascii_case("SUPER_ADMIN")
                    || r.eq_ignore_ascii_case("EXAM_COORDINATOR")
                    || r.eq_ignore_ascii_case("VICE_PRINCIPAL")
                    || r.eq_ignore_ascii_case("PRINCIPAL")
            });

        if !is_admin {
            return Err(AppError::Forbidden(
                "Access denied: Registering examination events is reserved for School Admins and Exam Coordinators."
                    .into(),
            ));
        }

        let exam = ExamRepository::create_exam(&state.db, user.school_id, user.id, payload).await?;
        Ok((StatusCode::CREATED, Json(exam)))
    }

    /// GET /api/v1/academic/exams
    pub async fn list_exams(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(query): Query<ExamFilterQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let exams = ExamRepository::list_exams(&state.db, user.school_id, query).await?;
        Ok((StatusCode::OK, Json(serde_json::json!({ "exams": exams }))))
    }

    /// GET /api/v1/academic/exams/:id
    pub async fn get_exam_detail(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let is_admin = user.is_school_admin()
            || user.roles.iter().any(|r| {
                r.eq_ignore_ascii_case("SUPER_ADMIN")
                    || r.eq_ignore_ascii_case("EXAM_COORDINATOR")
                    || r.eq_ignore_ascii_case("VICE_PRINCIPAL")
                    || r.eq_ignore_ascii_case("PRINCIPAL")
            });

        let detail =
            ExamRepository::get_exam_detail(&state.db, user.school_id, user.id, is_admin, id)
                .await?;

        Ok((StatusCode::OK, Json(detail)))
    }

    /// POST /api/v1/academic/exams/:id/marks
    pub async fn save_exam_marks(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(id): Path<Uuid>,
        Json(payload): Json<SaveMarksPayload>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let is_admin = user.is_school_admin()
            || user.roles.iter().any(|r| {
                r.eq_ignore_ascii_case("SUPER_ADMIN")
                    || r.eq_ignore_ascii_case("EXAM_COORDINATOR")
                    || r.eq_ignore_ascii_case("VICE_PRINCIPAL")
                    || r.eq_ignore_ascii_case("PRINCIPAL")
            });

        ExamRepository::save_exam_marks(&state.db, user.school_id, user.id, is_admin, id, payload)
            .await?;

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "message": "Exam marks saved successfully"
            })),
        ))
    }

    /// POST /api/v1/academic/exams/:id/notify
    pub async fn notify_exam(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let count = ExamRepository::notify_exam_whatsapp(&state.db, user.school_id, id).await?;

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "messages_queued": count
            })),
        ))
    }

    /// POST /api/v1/academic/assignments
    pub async fn create_assignment(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<CreateAssignmentPayload>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let is_teacher = user.roles.iter().any(|r| {
            r.eq_ignore_ascii_case("TEACHER")
                || r.eq_ignore_ascii_case("CLASS_TEACHER")
                || r.eq_ignore_ascii_case("SUBJECT_COORDINATOR")
                || r.eq_ignore_ascii_case("SUPER_ADMIN")
                || r.eq_ignore_ascii_case("SCHOOL_ADMIN")
        }) || user.is_class_teacher
            || user.is_school_admin();

        if !is_teacher {
            return Err(AppError::Forbidden(
                "Access denied: Creating assignments is reserved for Subject Teachers and School Administrators."
                    .into(),
            ));
        }

        let assignment =
            ExamRepository::create_assignment(&state.db, user.school_id, user.id, payload).await?;

        Ok((StatusCode::CREATED, Json(assignment)))
    }

    /// GET /api/v1/academic/assignments
    pub async fn list_assignments(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(query): Query<AssignmentFilterQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let assignments =
            ExamRepository::list_assignments(&state.db, user.school_id, query).await?;

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "assignments": assignments })),
        ))
    }

    /// GET /api/v1/academic/assignments/:id
    pub async fn get_assignment_detail(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let is_admin = user.is_school_admin()
            || user.roles.iter().any(|r| {
                r.eq_ignore_ascii_case("SUPER_ADMIN")
                    || r.eq_ignore_ascii_case("EXAM_COORDINATOR")
                    || r.eq_ignore_ascii_case("VICE_PRINCIPAL")
                    || r.eq_ignore_ascii_case("PRINCIPAL")
            });

        let detail =
            ExamRepository::get_assignment_detail(&state.db, user.school_id, user.id, is_admin, id)
                .await?;

        Ok((StatusCode::OK, Json(detail)))
    }

    /// POST /api/v1/academic/assignments/:id/marks
    pub async fn save_assignment_marks(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(id): Path<Uuid>,
        Json(payload): Json<SaveMarksPayload>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let is_admin = user.is_school_admin()
            || user.roles.iter().any(|r| {
                r.eq_ignore_ascii_case("SUPER_ADMIN")
                    || r.eq_ignore_ascii_case("EXAM_COORDINATOR")
                    || r.eq_ignore_ascii_case("VICE_PRINCIPAL")
                    || r.eq_ignore_ascii_case("PRINCIPAL")
            });

        ExamRepository::save_assignment_marks(
            &state.db,
            user.school_id,
            user.id,
            is_admin,
            id,
            payload,
        )
        .await?;

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "message": "Assignment marks saved successfully"
            })),
        ))
    }

    /// POST /api/v1/academic/assignments/:id/notify
    pub async fn notify_assignment(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let count =
            ExamRepository::notify_assignment_whatsapp(&state.db, user.school_id, id).await?;

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "messages_queued": count
            })),
        ))
    }

    /// GET /api/v1/academic/class-section/:id/scores (Class Teacher Read-Only View)
    pub async fn get_class_section_scores(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let scores =
            ExamRepository::get_class_section_scores(&state.db, user.school_id, id).await?;

        Ok((StatusCode::OK, Json(scores)))
    }
}
