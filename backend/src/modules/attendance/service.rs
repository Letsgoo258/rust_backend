use chrono::{DateTime, Local, NaiveDate, NaiveTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    error::AppError,
    modules::{
        academic::timetable::service::TimetableService,
        attendance::{
            dto::{
                AttendanceConfigDto, AttendanceSessionDetailResponse, AttendanceSessionSummary,
                BulkEditAttendanceRequest, CollisionItem, RecordAttendanceRequest,
                ResolveCollisionRequest, StudentRosterItem, TodayAttendanceStats,
                UpdateSingleRecordRequest,
            },
            repository::AttendanceRepository,
        },
        auth::backend::User,
    },
};

pub struct AttendanceService;

impl AttendanceService {
    /// Retrieve enrolled students roster for taking attendance
    pub async fn get_class_roster(
        pool: &PgPool,
        school_id: Uuid,
        class_section_id: Uuid,
        subject_id: Option<Uuid>,
    ) -> Result<Vec<StudentRosterItem>, AppError> {
        AttendanceRepository::get_enrolled_roster(pool, school_id, class_section_id, subject_id)
            .await
    }

    /// Record initial class or period attendance
    pub async fn record_attendance(
        pool: &PgPool,
        user: &User,
        payload: RecordAttendanceRequest,
    ) -> Result<Uuid, AppError> {
        let school_id = user.school_id;

        // 1. Strict Admin Restriction: School Admins cannot take initial attendance
        if user.is_school_admin()
            || user.roles.iter().any(|r| {
                r.eq_ignore_ascii_case("SUPER_ADMIN") || r.eq_ignore_ascii_case("SCHOOL_ADMIN")
            })
        {
            return Err(AppError::Forbidden(
                "School Administrators are restricted from recording initial attendance. Daily class attendance must be recorded by designated teachers or appointed substitutes.".into(),
            ));
        }

        // 2. Capability check: Teachers must possess attendance:record or attendance:mark
        if !user.has_capability("attendance:record") && !user.has_capability("attendance:mark") {
            return Err(AppError::Forbidden(
                "User lacks required 'attendance:record' capability to mark attendance".into(),
            ));
        }

        let date = NaiveDate::parse_from_str(&payload.date, "%Y-%m-%d")
            .map_err(|e| AppError::BadRequest(format!("Invalid date format: {e}")))?;

        // 3. Timetable alteration & today's timetable check on first attendance record
        // Ensures current_day_timetable is up-to-date and checks for substitution deadlocks
        TimetableService::reconcile_and_verify_today_schedule(pool, school_id, date).await?;

        // 4. Resolve academic year
        let academic_year_id =
            AttendanceRepository::get_active_academic_year_id(pool, school_id).await?;

        // 5. Subject Type check: Does this subject support multiple teachers/co-teaching?
        let (is_shared_subject, is_authorized_teacher) =
            AttendanceRepository::check_subject_shared_and_authorization(
                pool,
                school_id,
                user.id,
                payload.class_section_id,
                date,
                payload.period_number,
                payload.subject_id,
            )
            .await?;

        // 6. Collision & duplicate session check
        let existing = AttendanceRepository::get_existing_session_id(
            pool,
            school_id,
            payload.class_section_id,
            date,
            payload.period_number,
            payload.subject_id,
        )
        .await?;

        let mut target_session_id = Uuid::new_v4();

        if let Some((existing_session_id, existing_marked_by, _status, is_locked, created_at)) =
            existing
        {
            target_session_id = existing_session_id;

            if is_locked {
                return Err(AppError::Forbidden(
                    "This attendance session has been finalized and locked as per school policy."
                        .into(),
                ));
            }

            if is_shared_subject && is_authorized_teacher {
                // Legitimate shared/co-taught/practical session:
                // Multiple teachers recording is valid collaborative instruction!
                // NO collision and NO ticket raised.
                tracing::info!(
                    "Collaborative attendance session update for shared subject by teacher {}",
                    user.username
                );
            } else if existing_marked_by == user.id {
                // Same teacher re-submitting / updating their session
                // Check freezing period
                let config = AttendanceRepository::get_attendance_config(pool, school_id).await?;
                let elapsed_minutes = Utc::now().signed_duration_since(created_at).num_minutes();
                if elapsed_minutes > config.allow_teacher_edit_window_minutes as i64 {
                    return Err(AppError::Forbidden(format!(
                        "The {}-minute teacher edit window for this session has expired. Modifications require administrative review.",
                        config.allow_teacher_edit_window_minutes
                    )));
                }
            } else {
                // True single-teacher collision: conflicting teacher attempting to overwrite
                let ticket_number = AttendanceRepository::report_collision_and_raise_ticket(
                    pool,
                    school_id,
                    payload.class_section_id,
                    date,
                    payload.period_number,
                    user.id,
                    user.roles.first().map(|s| s.as_str()).unwrap_or("TEACHER"),
                    Some(existing_session_id),
                    Some(existing_marked_by),
                    "DUPLICATE_SESSION_ATTEMPT",
                    &format!(
                        "Teacher '{}' attempted to record attendance for class section {}, but attendance was already recorded by teacher ID {}.",
                        user.username, payload.class_section_id, existing_marked_by
                    ),
                )
                .await?;

                return Err(AppError::Conflict(format!(
                    "Attendance collision detected: This class section has already been marked. A high-priority verification ticket #{} has been raised for administrative resolution.",
                    ticket_number
                )));
            }
        }

        // 7. Check freezing period cutoff for today's session
        let config = AttendanceRepository::get_attendance_config(pool, school_id).await?;
        if payload.period_number.is_none() {
            // Daily roll-call: compare with daily_cutoff_time
            if let Ok(cutoff) = NaiveTime::parse_from_str(&config.daily_cutoff_time, "%H:%M") {
                let current_time = Local::now().time();
                let today = Local::now().date_naive();
                if date == today && current_time > cutoff {
                    tracing::warn!(
                        "Daily attendance recorded after cutoff time {} (at {})",
                        config.daily_cutoff_time,
                        current_time
                    );
                }
            }
        }

        // 8. Execute atomic transactional insert
        let mut tx = pool.begin().await?;

        let attendance_type = payload.attendance_type.unwrap_or_else(|| {
            if payload.period_number.is_some() {
                "PERIOD_WISE".to_string()
            } else {
                "DAILY".to_string()
            }
        });

        let role_str = user.roles.first().map(|s| s.as_str()).unwrap_or("TEACHER");

        let session_id = AttendanceRepository::upsert_attendance_session(
            &mut tx,
            target_session_id,
            school_id,
            academic_year_id,
            payload.class_section_id,
            date,
            payload.period_number,
            &attendance_type,
            payload.subject_id,
            user.id,
            role_str,
            is_shared_subject,
        )
        .await?;

        AttendanceRepository::insert_attendance_records(
            &mut tx,
            school_id,
            session_id,
            user.id,
            &payload.records,
        )
        .await?;

        tx.commit().await?;

        // 9. Emit audit event
        let _ = crate::utils::audit::log_action(
            pool,
            Some(school_id),
            Some(user.id),
            "ATTENDANCE_RECORDED",
            Some("attendance_sessions"),
            Some(session_id),
            Some(serde_json::json!({
                "class_section_id": payload.class_section_id,
                "date": date,
                "period_number": payload.period_number,
                "attendance_type": attendance_type,
                "records_count": payload.records.len(),
                "is_shared_subject": is_shared_subject,
            })),
            None,
        )
        .await;

        Ok(session_id)
    }

    /// Administrative bulk edit of student attendance records with mandatory justification
    pub async fn bulk_edit_attendance(
        pool: &PgPool,
        user: &User,
        payload: BulkEditAttendanceRequest,
    ) -> Result<usize, AppError> {
        let school_id = user.school_id;

        // 1. Capability check: Requires attendance:bulk_edit
        if !user.has_capability("attendance:bulk_edit") && !user.is_school_admin() {
            return Err(AppError::Forbidden(
                "User lacks administrative 'attendance:bulk_edit' capability".into(),
            ));
        }

        // 2. Strict validation: Reason is mandatory!
        let clean_reason = payload.reason.trim();
        if clean_reason.is_empty() {
            return Err(AppError::BadRequest(
                "A mandatory justification or reason is strictly required for administrative attendance corrections.".into(),
            ));
        }

        // 3. Verify session exists in school
        let session_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM attendance_sessions WHERE id = $1 AND school_id = $2)",
        )
        .bind(payload.session_id)
        .bind(school_id)
        .fetch_one(pool)
        .await?;

        if !session_exists {
            return Err(AppError::NotFound("Attendance session not found".into()));
        }

        let mut tx = pool.begin().await?;
        let mut updated_count = 0;
        let role_str = user.roles.first().map(|s| s.as_str()).unwrap_or("ADMIN");

        for upd in &payload.updates {
            let clean_status = upd.status.trim().to_uppercase();

            // Fetch old record
            let old_record = sqlx::query_as::<_, (Uuid, String)>(
                r#"
                SELECT id, status 
                FROM attendance_records 
                WHERE attendance_session_id = $1 AND student_id = $2
                "#,
            )
            .bind(payload.session_id)
            .bind(upd.student_id)
            .fetch_optional(&mut *tx)
            .await?;

            if let Some((record_id, old_status)) = old_record {
                if old_status != clean_status {
                    // Update record
                    sqlx::query(
                        r#"
                        UPDATE attendance_records
                        SET status = $1, remarks = COALESCE($2, remarks), updated_at = NOW()
                        WHERE id = $3
                        "#,
                    )
                    .bind(&clean_status)
                    .bind(upd.remarks.as_deref())
                    .bind(record_id)
                    .execute(&mut *tx)
                    .await?;

                    // Record critical audit correction
                    AttendanceRepository::record_correction(
                        &mut tx,
                        school_id,
                        record_id,
                        payload.session_id,
                        upd.student_id,
                        &old_status,
                        &clean_status,
                        clean_reason,
                        user.id,
                        role_str,
                    )
                    .await?;

                    updated_count += 1;
                }
            } else {
                // Insert new record if was not previously marked
                let record_id = Uuid::new_v4();
                sqlx::query(
                    r#"
                    INSERT INTO attendance_records (
                        id, school_id, attendance_session_id, student_id,
                        status, marked_at, marked_by, remarks, created_at, updated_at
                    ) VALUES ($1, $2, $3, $4, $5, NOW(), $6, $7, NOW(), NOW())
                    "#,
                )
                .bind(record_id)
                .bind(school_id)
                .bind(payload.session_id)
                .bind(upd.student_id)
                .bind(&clean_status)
                .bind(user.id)
                .bind(upd.remarks.as_deref())
                .execute(&mut *tx)
                .await?;

                AttendanceRepository::record_correction(
                    &mut tx,
                    school_id,
                    record_id,
                    payload.session_id,
                    upd.student_id,
                    "UNRECORDED",
                    &clean_status,
                    clean_reason,
                    user.id,
                    role_str,
                )
                .await?;

                updated_count += 1;
            }
        }

        tx.commit().await?;

        // 4. Critical audit logging
        let _ = crate::utils::audit::record_entity_change(
            pool,
            school_id,
            "ATTENDANCE_SESSION",
            payload.session_id,
            "CRITICAL_BULK_EDIT",
            Some(user.id),
            None,
            Some(serde_json::json!({
                "reason": clean_reason,
                "updated_count": updated_count,
            })),
            None,
            None,
        )
        .await;

        Ok(updated_count)
    }

    /// Single record update by teacher or administrator
    pub async fn update_single_record(
        pool: &PgPool,
        user: &User,
        record_id: Uuid,
        payload: UpdateSingleRecordRequest,
    ) -> Result<(), AppError> {
        let school_id = user.school_id;

        // Fetch record and session details
        let info = sqlx::query(
            r#"
            SELECT
                r.id as record_id, r.attendance_session_id, r.student_id, r.status as old_status,
                s.marked_by, s.is_locked, s.created_at, s.class_section_id
            FROM attendance_records r
            JOIN attendance_sessions s ON r.attendance_session_id = s.id
            WHERE r.id = $1 AND r.school_id = $2
            "#,
        )
        .bind(record_id)
        .bind(school_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Attendance record not found".into()))?;

        let session_id: Uuid = info.get("attendance_session_id");
        let student_id: Uuid = info.get("student_id");
        let old_status: String = info.get("old_status");
        let marked_by: Uuid = info.get("marked_by");
        let is_locked: bool = info.get("is_locked");
        let created_at: DateTime<Utc> = info.get("created_at");
        let class_section_id: Uuid = info.get("class_section_id");

        let is_admin = user.is_school_admin() || user.has_capability("attendance:bulk_edit");
        let is_marker = marked_by == user.id;

        // Check class teacher status
        let is_class_teacher: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM class_sections cs
                JOIN classes c ON c.id = cs.class_id
                WHERE cs.id = $1 AND c.school_id = $2 AND (
                    cs.class_teacher_staff_id = (
                        SELECT id FROM staff WHERE user_id = $3 AND school_id = $2
                    ) OR cs.class_teacher_staff_id = $3
                )
            )
            "#,
        )
        .bind(class_section_id)
        .bind(school_id)
        .bind(user.id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        let can_edit = is_admin
            || (is_marker && user.has_capability("attendance:edit_subject"))
            || (is_class_teacher && user.has_capability("attendance:edit_class"));

        if !can_edit {
            return Err(AppError::Forbidden(
                "Unauthorized to modify this student's attendance record".into(),
            ));
        }

        let config = AttendanceRepository::get_attendance_config(pool, school_id).await?;
        let elapsed_minutes = Utc::now().signed_duration_since(created_at).num_minutes();
        let is_past_window = elapsed_minutes > config.allow_teacher_edit_window_minutes as i64;

        if !is_admin && (is_locked || is_past_window) {
            return Err(AppError::Forbidden(
                "Attendance edit grace period has expired. Only School Administrators can modify frozen records with a recorded reason.".into(),
            ));
        }

        let reason = payload.reason.as_deref().map(|s| s.trim()).unwrap_or("");

        if is_admin && reason.is_empty() {
            return Err(AppError::BadRequest(
                "A mandatory reason/justification is required for administrative updates.".into(),
            ));
        }

        let clean_status = payload.status.trim().to_uppercase();
        let role_str = user.roles.first().map(|s| s.as_str()).unwrap_or("TEACHER");

        let mut tx = pool.begin().await?;

        sqlx::query(
            r#"
            UPDATE attendance_records
            SET status = $1, remarks = COALESCE($2, remarks), updated_at = NOW()
            WHERE id = $3
            "#,
        )
        .bind(&clean_status)
        .bind(payload.remarks.as_deref())
        .bind(record_id)
        .execute(&mut *tx)
        .await?;

        if old_status != clean_status {
            AttendanceRepository::record_correction(
                &mut tx,
                school_id,
                record_id,
                session_id,
                student_id,
                &old_status,
                &clean_status,
                if reason.is_empty() {
                    "Teacher in-window adjustment"
                } else {
                    reason
                },
                user.id,
                role_str,
            )
            .await?;
        }

        tx.commit().await?;

        Ok(())
    }

    /// Retrieve session details with records and correction logs
    pub async fn get_session_detail(
        pool: &PgPool,
        user: &User,
        session_id: Uuid,
    ) -> Result<AttendanceSessionDetailResponse, AppError> {
        AttendanceRepository::get_session_detail(pool, user.school_id, session_id).await
    }

    /// Search and list attendance sessions
    pub async fn list_sessions(
        pool: &PgPool,
        user: &User,
        date_opt: Option<NaiveDate>,
        class_section_id_opt: Option<Uuid>,
    ) -> Result<Vec<AttendanceSessionSummary>, AppError> {
        AttendanceRepository::list_sessions(pool, user.school_id, date_opt, class_section_id_opt)
            .await
    }

    /// Live KPIs for attendance dashboard
    pub async fn get_today_stats(
        pool: &PgPool,
        user: &User,
    ) -> Result<TodayAttendanceStats, AppError> {
        let today = Local::now().date_naive();
        AttendanceRepository::get_today_stats(pool, user.school_id, today).await
    }

    /// List collision incidents and tickets
    pub async fn list_collisions(
        pool: &PgPool,
        user: &User,
    ) -> Result<Vec<CollisionItem>, AppError> {
        if !user.has_capability("attendance:resolve_tickets") && !user.is_school_admin() {
            return Err(AppError::Forbidden(
                "Access restricted to School Administrators".into(),
            ));
        }

        AttendanceRepository::list_collisions(pool, user.school_id).await
    }

    /// Resolve an attendance collision ticket
    pub async fn resolve_collision(
        pool: &PgPool,
        user: &User,
        collision_id: Uuid,
        payload: ResolveCollisionRequest,
    ) -> Result<(), AppError> {
        if !user.has_capability("attendance:resolve_tickets") && !user.is_school_admin() {
            return Err(AppError::Forbidden(
                "Access restricted to School Administrators".into(),
            ));
        }

        AttendanceRepository::resolve_collision(
            pool,
            user.school_id,
            collision_id,
            user.id,
            &payload.action,
            &payload.resolution_notes,
        )
        .await
    }

    /// Get attendance configuration
    pub async fn get_config(pool: &PgPool, user: &User) -> Result<AttendanceConfigDto, AppError> {
        AttendanceRepository::get_attendance_config(pool, user.school_id).await
    }

    /// Update attendance configuration
    pub async fn update_config(
        pool: &PgPool,
        user: &User,
        config: AttendanceConfigDto,
    ) -> Result<(), AppError> {
        if !user.is_school_admin() && !user.has_capability("attendance:manage_config") {
            return Err(AppError::Forbidden(
                "Only School Administrators can modify attendance configurations".into(),
            ));
        }

        AttendanceRepository::save_attendance_config(pool, user.school_id, &config).await
    }

    /// Administrative or authorized deletion of an attendance session and its student records
    pub async fn delete_session(
        pool: &PgPool,
        user: &User,
        session_id: Uuid,
    ) -> Result<(), AppError> {
        let school_id = user.school_id;

        // 1. Capability check: Requires attendance:delete or is_school_admin
        if !user.has_capability("attendance:delete") && !user.is_school_admin() {
            return Err(AppError::Forbidden(
                "User lacks required 'attendance:delete' capability to delete attendance sessions"
                    .into(),
            ));
        }

        // 2. Check if session exists in this school
        let session_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM attendance_sessions WHERE id = $1 AND school_id = $2)",
        )
        .bind(session_id)
        .bind(school_id)
        .fetch_one(pool)
        .await?;

        if !session_exists {
            return Err(AppError::NotFound("Attendance session not found".into()));
        }

        let mut tx = pool.begin().await?;

        // Delete records
        sqlx::query(
            "DELETE FROM attendance_records WHERE attendance_session_id = $1 AND school_id = $2",
        )
        .bind(session_id)
        .bind(school_id)
        .execute(&mut *tx)
        .await?;

        // Delete session
        sqlx::query("DELETE FROM attendance_sessions WHERE id = $1 AND school_id = $2")
            .bind(session_id)
            .bind(school_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        // Audit log
        let _ = crate::utils::audit::record_entity_change(
            pool,
            school_id,
            "ATTENDANCE_SESSION",
            session_id,
            "SESSION_DELETED",
            Some(user.id),
            None,
            None,
            None,
            None,
        )
        .await;

        Ok(())
    }
}
