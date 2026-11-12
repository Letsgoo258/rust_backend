use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{
    error::AppError,
    modules::attendance::dto::{
        AttendanceConfigDto, AttendanceCorrectionLog, AttendanceSessionDetailResponse,
        AttendanceSessionSummary, ClassDayAttendanceSlot, ClassHeatmapRow, CollisionItem,
        HeatmapDayHeader, RecordWithSessionInfo, RegisterRow, StudentAttendanceDetail,
        StudentAttendanceInput, StudentRosterItem, TodayAttendanceStats, WeeklyHeatmapResponse,
        WeeklyHeatmapSummary,
    },
};

pub struct AttendanceRepository;

impl AttendanceRepository {
    /// Retrieve active academic year ID for the school
    pub async fn get_active_academic_year_id(
        pool: &PgPool,
        school_id: Uuid,
    ) -> Result<Uuid, AppError> {
        let year_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT id FROM academic_years WHERE school_id = $1 AND status = 'ACTIVE' LIMIT 1"#,
        )
        .bind(school_id)
        .fetch_optional(pool)
        .await?;

        if let Some(id) = year_id {
            Ok(id)
        } else {
            // Fallback to most recent academic year
            let fallback_id: Option<Uuid> = sqlx::query_scalar(
                r#"SELECT id FROM academic_years WHERE school_id = $1 ORDER BY start_date DESC LIMIT 1"#,
            )
            .bind(school_id)
            .fetch_optional(pool)
            .await?;

            fallback_id.ok_or_else(|| {
                AppError::BadRequest("No academic year configured for this school".into())
            })
        }
    }

    /// Retrieve active roster of students enrolled in the class section, optionally filtered by elective subject
    pub async fn get_enrolled_roster(
        pool: &PgPool,
        school_id: Uuid,
        class_section_id: Uuid,
        subject_id: Option<Uuid>,
    ) -> Result<Vec<StudentRosterItem>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                u.id as student_id,
                COALESCE(NULLIF(u.display_name, ''), NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''), u.username) as full_name,
                u.admission_number,
                COALESCE((
                    SELECT e.roll_number 
                    FROM enrollments e 
                    WHERE e.student_id = u.id AND (e.class_section_id = $1 OR e.class_section_id = u.class_section_id)
                    ORDER BY e.created_at DESC LIMIT 1
                ), '') as roll_number,
                u.gender
            FROM users u
            WHERE (u.class_section_id = $1 OR EXISTS (
                SELECT 1 FROM enrollments e WHERE e.student_id = u.id AND e.class_section_id = $1
            ))
              AND (
                  $3::uuid IS NULL
                  OR NOT EXISTS (
                      SELECT 1 FROM student_subject_enrollments sse
                      WHERE sse.school_id = $2 AND sse.class_section_id = $1 AND sse.subject_id = $3 AND sse.status = 'ACTIVE'
                  )
                  OR EXISTS (
                      SELECT 1 FROM student_subject_enrollments sse
                      WHERE sse.school_id = $2 AND sse.student_id = u.id AND sse.class_section_id = $1 AND sse.subject_id = $3 AND sse.status = 'ACTIVE'
                  )
              )
              AND u.school_id = $2
              AND u.status = 'ACTIVE'
            ORDER BY COALESCE(NULLIF(u.admission_number, ''), '999999'), full_name ASC
            "#,
        )
        .bind(class_section_id)
        .bind(school_id)
        .bind(subject_id)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| StudentRosterItem {
                student_id: r.get("student_id"),
                full_name: r.get("full_name"),
                admission_number: r.get("admission_number"),
                roll_number: r.get("roll_number"),
                gender: r.get("gender"),
            })
            .collect();

        Ok(list)
    }

    /// Check if an attendance session already exists for this slot and subject
    pub async fn get_existing_session_id(
        pool: &PgPool,
        school_id: Uuid,
        class_section_id: Uuid,
        date: NaiveDate,
        period_number: Option<i32>,
        subject_id: Option<Uuid>,
    ) -> Result<Option<(Uuid, Uuid, String, bool, DateTime<Utc>)>, AppError> {
        let row = sqlx::query_as::<_, (Uuid, Uuid, String, bool, DateTime<Utc>)>(
            r#"
            SELECT id, marked_by, status, is_locked, created_at
            FROM attendance_sessions
            WHERE school_id = $1 AND class_section_id = $2 AND date = $3
              AND COALESCE(period_number, 0) = COALESCE($4, 0)
              AND COALESCE(subject_id, '00000000-0000-0000-0000-000000000000'::uuid) = COALESCE($5, '00000000-0000-0000-0000-000000000000'::uuid)
            LIMIT 1
            "#,
        )
        .bind(school_id)
        .bind(class_section_id)
        .bind(date)
        .bind(period_number)
        .bind(subject_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// Check if subject allows multiple teachers and if the user is authorized to record/co-mark
    pub async fn check_subject_shared_and_authorization(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        class_section_id: Uuid,
        date: NaiveDate,
        period_number: Option<i32>,
        subject_id: Option<Uuid>,
    ) -> Result<(bool, bool), AppError> {
        // Find staff id for this user
        let staff_id_opt: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM staff WHERE user_id = $1 AND school_id = $2")
                .bind(user_id)
                .bind(school_id)
                .fetch_optional(pool)
                .await?;

        let staff_id = staff_id_opt.unwrap_or(user_id);

        // 1. Check if user is the assigned class teacher for this class_section
        let is_class_teacher: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM class_sections cs
                JOIN classes c ON c.id = cs.class_id
                WHERE cs.id = $1 AND c.school_id = $2 AND (cs.class_teacher_staff_id = $3 OR cs.class_teacher_staff_id = $4)
            )
            "#,
        )
        .bind(class_section_id)
        .bind(school_id)
        .bind(staff_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        // If period is None (morning homeroom roll-call), class teacher is inherently authorized
        if period_number.is_none() {
            return Ok((true, is_class_teacher));
        }

        let period = period_number.unwrap_or(1);

        // 2. Check subject properties
        let mut is_shared_subject = false;
        if let Some(sid) = subject_id {
            let row = sqlx::query(
                r#"
                SELECT s.name, st.allow_multiple_teachers, st.system_subject_type
                FROM subjects s
                LEFT JOIN subject_types st ON s.subject_type_id = st.id
                WHERE s.id = $1 AND s.school_id = $2
                "#,
            )
            .bind(sid)
            .bind(school_id)
            .fetch_optional(pool)
            .await?;

            if let Some(r) = row {
                let allow_multi: bool = r.try_get("allow_multiple_teachers").unwrap_or(false);
                let sys_type: Option<String> = r.try_get("system_subject_type").ok();
                let sys_type_lower = sys_type.unwrap_or_default().to_lowercase();

                if allow_multi
                    || sys_type_lower == "practical"
                    || sys_type_lower == "lab"
                    || sys_type_lower == "elective"
                    || sys_type_lower == "co-teaching"
                    || sys_type_lower == "vocational"
                {
                    is_shared_subject = true;
                }
            }
        }

        // 3. Check current day timetable assignment for this slot
        let slot_info = sqlx::query(
            r#"
            SELECT effective_teacher_id, co_teacher_ids, is_shared_subject
            FROM current_day_timetable
            WHERE school_id = $1 AND class_section_id = $2 AND date = $3 AND period_number = $4
            LIMIT 1
            "#,
        )
        .bind(school_id)
        .bind(class_section_id)
        .bind(date)
        .bind(period)
        .fetch_optional(pool)
        .await?;

        let mut is_authorized = is_class_teacher;

        if let Some(slot) = slot_info {
            let eff_teacher: Uuid = slot.get("effective_teacher_id");
            let co_teachers: Vec<Uuid> = slot.try_get("co_teacher_ids").unwrap_or_default();
            let slot_shared: bool = slot.try_get("is_shared_subject").unwrap_or(false);

            if slot_shared {
                is_shared_subject = true;
            }

            if eff_teacher == staff_id || co_teachers.contains(&staff_id) {
                is_authorized = true;
            }
        }

        Ok((is_shared_subject, is_authorized))
    }

    /// Report collision and automatically generate a high-priority ticket in data_change_tickets
    pub async fn report_collision_and_raise_ticket(
        pool: &PgPool,
        school_id: Uuid,
        class_section_id: Uuid,
        date: NaiveDate,
        period_number: Option<i32>,
        attempted_by_user_id: Uuid,
        attempted_by_role: &str,
        existing_session_id: Option<Uuid>,
        existing_marked_by_user_id: Option<Uuid>,
        collision_type: &str,
        reason: &str,
    ) -> Result<String, AppError> {
        let ticket_number = format!(
            "TKT-ATT-{}-{}",
            date.format("%Y%m%d"),
            Uuid::new_v4().to_string()[..6].to_uppercase()
        );

        let ticket_id = Uuid::new_v4();

        // 1. Create ticket in data_change_tickets
        sqlx::query(
            r#"
            INSERT INTO data_change_tickets (
                id, school_id, user_id, requested_by_user_id, ticket_number,
                ticket_type, status, field_name, new_value, reason, priority,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5,
                'ATTENDANCE_COLLISION', 'PENDING', $6, $7, $8, 'HIGH',
                NOW(), NOW()
            )
            "#,
        )
        .bind(ticket_id)
        .bind(school_id)
        .bind(attempted_by_user_id)
        .bind(attempted_by_user_id)
        .bind(&ticket_number)
        .bind(format!(
            "class_section_{}_period_{}",
            class_section_id,
            period_number.unwrap_or(0)
        ))
        .bind(collision_type)
        .bind(reason)
        .execute(pool)
        .await?;

        // 2. Insert into attendance_collisions
        sqlx::query(
            r#"
            INSERT INTO attendance_collisions (
                school_id, class_section_id, date, period_number,
                attempted_by_user_id, attempted_by_role,
                existing_session_id, existing_marked_by_user_id,
                collision_type, ticket_id, status, created_at
            ) VALUES (
                $1, $2, $3, $4,
                $5, $6,
                $7, $8,
                $9, $10, 'REPORTED', NOW()
            )
            "#,
        )
        .bind(school_id)
        .bind(class_section_id)
        .bind(date)
        .bind(period_number)
        .bind(attempted_by_user_id)
        .bind(attempted_by_role)
        .bind(existing_session_id)
        .bind(existing_marked_by_user_id)
        .bind(collision_type)
        .bind(ticket_id)
        .execute(pool)
        .await?;

        Ok(ticket_number)
    }

    /// Retrieve school attendance configuration from school_settings
    pub async fn get_attendance_config(
        pool: &PgPool,
        school_id: Uuid,
    ) -> Result<AttendanceConfigDto, AppError> {
        let val: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT value FROM school_settings WHERE school_id = $1 AND key = 'attendance_config'",
        )
        .bind(school_id)
        .fetch_optional(pool)
        .await?;

        if let Some(json_val) = val {
            let cfg: AttendanceConfigDto = serde_json::from_value(json_val).unwrap_or_default();
            Ok(cfg)
        } else {
            Ok(AttendanceConfigDto::default())
        }
    }

    /// Save school attendance configuration to school_settings
    pub async fn save_attendance_config(
        pool: &PgPool,
        school_id: Uuid,
        config: &AttendanceConfigDto,
    ) -> Result<(), AppError> {
        let json_val = serde_json::to_value(config)
            .map_err(|e| AppError::BadRequest(format!("Invalid config payload: {e}")))?;

        sqlx::query(
            r#"
            INSERT INTO school_settings (school_id, key, value, updated_at)
            VALUES ($1, 'attendance_config', $2, NOW())
            ON CONFLICT (school_id, key) DO UPDATE SET
                value = EXCLUDED.value,
                updated_at = NOW()
            "#,
        )
        .bind(school_id)
        .bind(json_val)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Insert or collaboratively update an attendance session
    pub async fn upsert_attendance_session<'a>(
        tx: &mut Transaction<'a, Postgres>,
        session_id: Uuid,
        school_id: Uuid,
        academic_year_id: Uuid,
        class_section_id: Uuid,
        date: NaiveDate,
        period_number: Option<i32>,
        attendance_type: &str,
        subject_id: Option<Uuid>,
        marked_by: Uuid,
        marked_by_role: &str,
        is_shared_session: bool,
    ) -> Result<Uuid, AppError> {
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO attendance_sessions (
                id, school_id, academic_year_id, class_section_id, date, period_number,
                attendance_type, subject_id, marked_by, marked_by_role,
                co_marked_by, is_shared_session, status, is_locked, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9, $10,
                ARRAY[$9::uuid], $11, 'RECORDED', false, NOW(), NOW()
            )
            ON CONFLICT (school_id, academic_year_id, class_section_id, date, COALESCE(period_number, 0), COALESCE(subject_id, '00000000-0000-0000-0000-000000000000'::uuid)) DO UPDATE SET
                co_marked_by = array_append(
                    array_remove(attendance_sessions.co_marked_by, EXCLUDED.marked_by),
                    EXCLUDED.marked_by
                ),
                updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(session_id)
        .bind(school_id)
        .bind(academic_year_id)
        .bind(class_section_id)
        .bind(date)
        .bind(period_number)
        .bind(attendance_type)
        .bind(subject_id)
        .bind(marked_by)
        .bind(marked_by_role)
        .bind(is_shared_session)
        .fetch_one(&mut **tx)
        .await?;

        Ok(id)
    }

    /// Bulk insert student attendance records
    pub async fn insert_attendance_records<'a>(
        tx: &mut Transaction<'a, Postgres>,
        school_id: Uuid,
        session_id: Uuid,
        marked_by: Uuid,
        records: &[StudentAttendanceInput],
    ) -> Result<usize, AppError> {
        let mut count = 0;
        for rec in records {
            let clean_status = rec.status.trim().to_uppercase();
            sqlx::query(
                r#"
                INSERT INTO attendance_records (
                    school_id, attendance_session_id, student_id, status,
                    marked_at, marked_by, remarks, created_at, updated_at
                ) VALUES ($1, $2, $3, $4, NOW(), $5, $6, NOW(), NOW())
                ON CONFLICT (attendance_session_id, student_id) DO UPDATE SET
                    status = EXCLUDED.status,
                    remarks = EXCLUDED.remarks,
                    marked_at = NOW(),
                    marked_by = EXCLUDED.marked_by,
                    updated_at = NOW()
                "#,
            )
            .bind(school_id)
            .bind(session_id)
            .bind(rec.student_id)
            .bind(&clean_status)
            .bind(marked_by)
            .bind(rec.remarks.as_deref())
            .execute(&mut **tx)
            .await?;

            count += 1;
        }

        Ok(count)
    }

    /// Record critical audit correction entry
    pub async fn record_correction<'a>(
        tx: &mut Transaction<'a, Postgres>,
        school_id: Uuid,
        record_id: Uuid,
        session_id: Uuid,
        student_id: Uuid,
        old_status: &str,
        new_status: &str,
        reason: &str,
        changed_by: Uuid,
        changed_by_role: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO attendance_corrections (
                school_id, attendance_record_id, attendance_session_id,
                student_id, old_status, new_status, reason,
                changed_by, changed_by_role, is_critical, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, true, NOW())
            "#,
        )
        .bind(school_id)
        .bind(record_id)
        .bind(session_id)
        .bind(student_id)
        .bind(old_status)
        .bind(new_status)
        .bind(reason)
        .bind(changed_by)
        .bind(changed_by_role)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    /// List recorded attendance sessions
    pub async fn list_sessions(
        pool: &PgPool,
        school_id: Uuid,
        date_opt: Option<NaiveDate>,
        class_section_id_opt: Option<Uuid>,
    ) -> Result<Vec<AttendanceSessionSummary>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                s.id, s.school_id, s.academic_year_id, s.class_section_id,
                c.name as class_name, sec.name as section_name,
                s.date::text as date, s.period_number, s.attendance_type,
                s.subject_id, subj.name as subject_name,
                s.marked_by,
                COALESCE(NULLIF(u.display_name, ''), u.username, 'Teacher') as marked_by_name,
                s.marked_by_role, s.is_shared_session,
                COALESCE(cardinality(s.co_marked_by), 1) as co_marked_count,
                s.status, s.is_locked,
                COUNT(r.id) as total_students,
                COUNT(CASE WHEN r.status = 'PRESENT' THEN 1 END) as present_count,
                COUNT(CASE WHEN r.status = 'ABSENT' THEN 1 END) as absent_count,
                COUNT(CASE WHEN r.status = 'LATE' THEN 1 END) as late_count,
                COUNT(CASE WHEN r.status = 'EXCUSED' THEN 1 END) as excused_count,
                s.created_at, s.updated_at
            FROM attendance_sessions s
            JOIN class_sections cs ON s.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections sec ON cs.section_id = sec.id
            LEFT JOIN subjects subj ON s.subject_id = subj.id
            JOIN users u ON s.marked_by = u.id
            LEFT JOIN attendance_records r ON s.id = r.attendance_session_id
            WHERE s.school_id = $1
              AND ($2::date IS NULL OR s.date = $2)
              AND ($3::uuid IS NULL OR s.class_section_id = $3)
            GROUP BY s.id, c.name, sec.name, subj.name, u.display_name, u.username
            ORDER BY s.date DESC, s.period_number ASC, c.name ASC
            "#,
        )
        .bind(school_id)
        .bind(date_opt)
        .bind(class_section_id_opt)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| AttendanceSessionSummary {
                id: r.get("id"),
                school_id: r.get("school_id"),
                academic_year_id: r.get("academic_year_id"),
                class_section_id: r.get("class_section_id"),
                class_name: r.get("class_name"),
                section_name: r.get("section_name"),
                date: r.get("date"),
                period_number: r.get("period_number"),
                attendance_type: r.get("attendance_type"),
                subject_id: r.get("subject_id"),
                subject_name: r.get("subject_name"),
                marked_by: r.get("marked_by"),
                marked_by_name: r.get("marked_by_name"),
                marked_by_role: r.get("marked_by_role"),
                is_shared_session: r.get("is_shared_session"),
                co_marked_count: r.get("co_marked_count"),
                status: r.get("status"),
                is_locked: r.get("is_locked"),
                total_students: r.get("total_students"),
                present_count: r.get("present_count"),
                absent_count: r.get("absent_count"),
                late_count: r.get("late_count"),
                excused_count: r.get("excused_count"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect();

        Ok(list)
    }

    /// Retrieve detailed session view with all student records and correction logs
    pub async fn get_session_detail(
        pool: &PgPool,
        school_id: Uuid,
        session_id: Uuid,
    ) -> Result<AttendanceSessionDetailResponse, AppError> {
        let sessions = Self::list_sessions(pool, school_id, None, None).await?;
        let session = sessions
            .into_iter()
            .find(|s| s.id == session_id)
            .ok_or_else(|| AppError::NotFound("Attendance session not found".into()))?;

        // Student records
        let rec_rows = sqlx::query(
            r#"
            SELECT
                r.id as record_id, r.student_id,
                COALESCE(NULLIF(u.display_name, ''), NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''), u.username) as student_name,
                u.admission_number, e.roll_number,
                r.status, r.remarks, r.marked_at,
                (SELECT COUNT(*) FROM attendance_corrections ac WHERE ac.attendance_record_id = r.id) as corrections_count
            FROM attendance_records r
            JOIN users u ON r.student_id = u.id
            LEFT JOIN enrollments e ON e.student_id = u.id AND e.status = 'ACTIVE'
            WHERE r.attendance_session_id = $1
            ORDER BY COALESCE(NULLIF(e.roll_number, ''), '999999'), student_name ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(pool)
        .await?;

        let records = rec_rows
            .into_iter()
            .map(|r| StudentAttendanceDetail {
                record_id: r.get("record_id"),
                student_id: r.get("student_id"),
                student_name: r.get("student_name"),
                admission_number: r.get("admission_number"),
                roll_number: r.get("roll_number"),
                status: r.get("status"),
                remarks: r.get("remarks"),
                marked_at: r.get("marked_at"),
                corrections_count: r.get("corrections_count"),
            })
            .collect();

        // Correction history
        let corr_rows = sqlx::query(
            r#"
            SELECT
                ac.id, ac.student_id,
                COALESCE(NULLIF(u_stu.display_name, ''), u_stu.username, 'Student') as student_name,
                ac.old_status, ac.new_status, ac.reason,
                ac.changed_by,
                COALESCE(NULLIF(u_adm.display_name, ''), u_adm.username, 'User') as changed_by_name,
                ac.changed_by_role, ac.is_critical, ac.created_at
            FROM attendance_corrections ac
            JOIN users u_stu ON ac.student_id = u_stu.id
            JOIN users u_adm ON ac.changed_by = u_adm.id
            WHERE ac.attendance_session_id = $1
            ORDER BY ac.created_at DESC
            "#,
        )
        .bind(session_id)
        .fetch_all(pool)
        .await?;

        let corrections = corr_rows
            .into_iter()
            .map(|r| AttendanceCorrectionLog {
                id: r.get("id"),
                student_id: r.get("student_id"),
                student_name: r.get("student_name"),
                old_status: r.get("old_status"),
                new_status: r.get("new_status"),
                reason: r.get("reason"),
                changed_by: r.get("changed_by"),
                changed_by_name: r.get("changed_by_name"),
                changed_by_role: r.get("changed_by_role"),
                is_critical: r.get("is_critical"),
                created_at: r.get("created_at"),
            })
            .collect();

        Ok(AttendanceSessionDetailResponse {
            session,
            records,
            corrections,
        })
    }

    /// Retrieve live KPIs for today's attendance
    pub async fn get_today_stats(
        pool: &PgPool,
        school_id: Uuid,
        date: NaiveDate,
    ) -> Result<TodayAttendanceStats, AppError> {
        let total_enrolled: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(u.id)
            FROM users u
            WHERE u.school_id = $1 AND u.class_section_id IS NOT NULL AND u.status = 'ACTIVE'
            "#,
        )
        .bind(school_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        let total_sections: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(cs.id)
            FROM class_sections cs
            JOIN classes c ON c.id = cs.class_id
            WHERE c.school_id = $1 AND cs.status = 'ACTIVE'
            "#,
        )
        .bind(school_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        let completed_sections: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(DISTINCT class_section_id) 
            FROM attendance_sessions 
            WHERE school_id = $1 AND date = $2
            "#,
        )
        .bind(school_id)
        .bind(date)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        let pending_sections = (total_sections - completed_sections).max(0);

        let counts_row = sqlx::query(
            r#"
            SELECT
                COUNT(r.id) as marked_students,
                COUNT(CASE WHEN r.status = 'PRESENT' THEN 1 END) as present_students,
                COUNT(CASE WHEN r.status = 'ABSENT' THEN 1 END) as absent_students,
                COUNT(CASE WHEN r.status = 'LATE' THEN 1 END) as late_students
            FROM attendance_records r
            JOIN attendance_sessions s ON r.attendance_session_id = s.id
            WHERE s.school_id = $1 AND s.date = $2
            "#,
        )
        .bind(school_id)
        .bind(date)
        .fetch_one(pool)
        .await?;

        let marked_students: i64 = counts_row.get("marked_students");
        let present_students: i64 = counts_row.get("present_students");
        let absent_students: i64 = counts_row.get("absent_students");
        let late_students: i64 = counts_row.get("late_students");

        let attendance_percentage = if marked_students > 0 {
            ((present_students as f64) / (marked_students as f64)) * 100.0
        } else {
            0.0
        };

        let collision_alerts_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM attendance_collisions WHERE school_id = $1 AND date = $2 AND status = 'REPORTED'",
        )
        .bind(school_id)
        .bind(date)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        let substitution_deadlocks_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM data_change_tickets
            WHERE school_id = $1 AND ticket_type = 'SUBSTITUTION_DEADLOCK' AND status = 'PENDING'
            "#,
        )
        .bind(school_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        Ok(TodayAttendanceStats {
            date: date.to_string(),
            total_enrolled,
            marked_students,
            present_students,
            absent_students,
            late_students,
            attendance_percentage: (attendance_percentage * 10.0).round() / 10.0,
            total_sections,
            completed_sections,
            pending_sections,
            collision_alerts_count,
            substitution_deadlocks_count,
        })
    }

    /// List all collision incidents and linked tickets
    pub async fn list_collisions(
        pool: &PgPool,
        school_id: Uuid,
    ) -> Result<Vec<CollisionItem>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                ac.id, ac.class_section_id,
                c.name as class_name, s.name as section_name,
                ac.date::text as date, ac.period_number,
                ac.attempted_by_user_id,
                COALESCE(NULLIF(u_att.display_name, ''), u_att.username, 'Teacher') as attempted_by_name,
                ac.attempted_by_role,
                ac.existing_session_id,
                COALESCE(NULLIF(u_ex.display_name, ''), u_ex.username, 'Teacher') as existing_marked_by_name,
                ac.collision_type,
                ac.ticket_id,
                t.ticket_number,
                ac.status,
                ac.resolution_notes,
                ac.created_at
            FROM attendance_collisions ac
            JOIN class_sections cs ON ac.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            JOIN users u_att ON ac.attempted_by_user_id = u_att.id
            LEFT JOIN users u_ex ON ac.existing_marked_by_user_id = u_ex.id
            LEFT JOIN data_change_tickets t ON ac.ticket_id = t.id
            WHERE ac.school_id = $1
            ORDER BY ac.created_at DESC
            LIMIT 100
            "#,
        )
        .bind(school_id)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| CollisionItem {
                id: r.get("id"),
                class_section_id: r.get("class_section_id"),
                class_name: r.get("class_name"),
                section_name: r.get("section_name"),
                date: r.get("date"),
                period_number: r.get("period_number"),
                attempted_by_user_id: r.get("attempted_by_user_id"),
                attempted_by_name: r.get("attempted_by_name"),
                attempted_by_role: r.get("attempted_by_role"),
                existing_session_id: r.get("existing_session_id"),
                existing_marked_by_name: r.get("existing_marked_by_name"),
                collision_type: r.get("collision_type"),
                ticket_id: r.get("ticket_id"),
                ticket_number: r.get("ticket_number"),
                status: r.get("status"),
                resolution_notes: r.get("resolution_notes"),
                created_at: r.get("created_at"),
            })
            .collect();

        Ok(list)
    }

    /// Resolve collision ticket administratively
    pub async fn resolve_collision(
        pool: &PgPool,
        school_id: Uuid,
        collision_id: Uuid,
        reviewer_user_id: Uuid,
        action: &str,
        resolution_notes: &str,
    ) -> Result<(), AppError> {
        let ticket_id_opt: Option<Uuid> = sqlx::query_scalar(
            "SELECT ticket_id FROM attendance_collisions WHERE id = $1 AND school_id = $2",
        )
        .bind(collision_id)
        .bind(school_id)
        .fetch_optional(pool)
        .await?;

        let status = match action.trim().to_uppercase().as_str() {
            "DISMISS" | "DISMISSED" => "DISMISSED",
            _ => "RESOLVED",
        };

        sqlx::query(
            r#"
            UPDATE attendance_collisions
            SET status = $1, resolution_notes = $2
            WHERE id = $3 AND school_id = $4
            "#,
        )
        .bind(status)
        .bind(resolution_notes)
        .bind(collision_id)
        .bind(school_id)
        .execute(pool)
        .await?;

        if let Some(t_id) = ticket_id_opt {
            sqlx::query(
                r#"
                UPDATE data_change_tickets
                SET status = $1, reviewer_user_id = $2, reviewer_notes = $3, updated_at = NOW()
                WHERE id = $4 AND school_id = $5
                "#,
            )
            .bind(status)
            .bind(reviewer_user_id)
            .bind(resolution_notes)
            .bind(t_id)
            .bind(school_id)
            .execute(pool)
            .await?;
        }

        Ok(())
    }

    /// Retrieve flat attendance records for daily register view
    pub async fn get_register(
        pool: &PgPool,
        school_id: Uuid,
        date_opt: Option<NaiveDate>,
        class_section_id_opt: Option<Uuid>,
    ) -> Result<Vec<RegisterRow>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                r.id as record_id,
                r.student_id,
                COALESCE(NULLIF(u.display_name, ''), NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''), u.username, 'Student') as student_name,
                COALESCE((
                    SELECT e.roll_number 
                    FROM enrollments e 
                    WHERE e.student_id = u.id AND (e.class_section_id = s.class_section_id OR e.class_section_id = u.class_section_id)
                    ORDER BY e.created_at DESC LIMIT 1
                ), '') as roll_number,
                u.admission_number,
                c.name as class_name,
                sec.name as section_name,
                subj.name as subject_name,
                s.attendance_type as session_type,
                r.status,
                r.remarks,
                COALESCE(NULLIF(u_marker.display_name, ''), u_marker.username, 'Teacher') as marked_by_name,
                r.created_at::text as created_at
            FROM attendance_records r
            JOIN attendance_sessions s ON r.attendance_session_id = s.id
            JOIN class_sections cs ON s.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections sec ON cs.section_id = sec.id
            LEFT JOIN subjects subj ON s.subject_id = subj.id
            JOIN users u ON r.student_id = u.id
            LEFT JOIN users u_marker ON s.marked_by = u_marker.id
            WHERE s.school_id = $1
              AND ($2::date IS NULL OR s.date = $2)
              AND ($3::uuid IS NULL OR s.class_section_id = $3)
            ORDER BY s.date DESC, c.name ASC, sec.name ASC, COALESCE(NULLIF(u.admission_number, ''), '999999'), student_name ASC
            "#,
        )
        .bind(school_id)
        .bind(date_opt)
        .bind(class_section_id_opt)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| RegisterRow {
                record_id: r.get("record_id"),
                student_id: r.get("student_id"),
                student_name: r.get("student_name"),
                roll_number: r.get("roll_number"),
                admission_number: r.get("admission_number"),
                class_name: r.get("class_name"),
                section_name: r.get("section_name"),
                subject_name: r.get("subject_name"),
                session_type: r.get("session_type"),
                status: r.get("status"),
                remarks: r.get("remarks"),
                marked_by_name: r.get("marked_by_name"),
                created_at: r.get("created_at"),
            })
            .collect();

        Ok(list)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Verifies if an attendance session exists for the school.
    // 2. Joins & Foreign Keys: attendance_sessions.id, attendance_sessions.school_id
    // 3. Why & Correctness: Multi-tenant safety check before administrative modifications or deletions.
    // =========================================================================
    pub async fn session_exists(
        pool: &PgPool,
        school_id: Uuid,
        session_id: Uuid,
    ) -> Result<bool, AppError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM attendance_sessions WHERE id = $1 AND school_id = $2)",
        )
        .bind(session_id)
        .bind(school_id)
        .fetch_one(pool)
        .await?;

        Ok(exists)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Fetches the existing attendance record ID and status for a student within a session.
    // 2. Joins & Foreign Keys: attendance_records.attendance_session_id, attendance_records.student_id (users.id)
    // 3. Why & Correctness: Executes within transaction to check if an update or new insert is required.
    // =========================================================================
    pub async fn get_record_for_student(
        conn: &mut Transaction<'_, Postgres>,
        session_id: Uuid,
        student_id: Uuid,
    ) -> Result<Option<(Uuid, String)>, AppError> {
        let record = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            SELECT id, status 
            FROM attendance_records 
            WHERE attendance_session_id = $1 AND student_id = $2
            "#,
        )
        .bind(session_id)
        .bind(student_id)
        .fetch_optional(&mut **conn)
        .await?;

        Ok(record)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Updates an attendance record's status and remarks with timestamp refreshed.
    // 2. Joins & Foreign Keys: attendance_records.id
    // 3. Why & Correctness: Executes within transaction; preserves existing remarks if new remarks is NULL.
    // =========================================================================
    pub async fn update_record_status(
        conn: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
        status: &str,
        remarks: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE attendance_records
            SET status = $1, remarks = COALESCE($2, remarks), updated_at = NOW()
            WHERE id = $3
            "#,
        )
        .bind(status)
        .bind(remarks)
        .bind(record_id)
        .execute(&mut **conn)
        .await?;

        Ok(())
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Inserts a single attendance record for a student in a session.
    // 2. Joins & Foreign Keys:
    //    - student_id -> users.id (Student identity)
    //    - marked_by -> users.id (Marker identity)
    //    - attendance_session_id -> attendance_sessions.id
    // 3. Why & Correctness: Used during administrative bulk correction when a student was missing an initial record.
    // =========================================================================
    pub async fn insert_single_record(
        conn: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
        school_id: Uuid,
        session_id: Uuid,
        student_id: Uuid,
        status: &str,
        marked_by: Uuid,
        remarks: Option<&str>,
    ) -> Result<(), AppError> {
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
        .bind(session_id)
        .bind(student_id)
        .bind(status)
        .bind(marked_by)
        .bind(remarks)
        .execute(&mut **conn)
        .await?;

        Ok(())
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Retrieves attendance record details along with session metadata for permission & grace period verification.
    // 2. Joins & Foreign Keys:
    //    - attendance_records.attendance_session_id -> attendance_sessions.id
    //    - attendance_records.school_id -> schools.id
    // 3. Why & Correctness:
    //    - Enforces tenant isolation on school_id ($2)
    //    - Provides marked_by, is_locked, and session created_at to validate whether teacher can still edit.
    // =========================================================================
    pub async fn get_record_with_session(
        pool: &PgPool,
        school_id: Uuid,
        record_id: Uuid,
    ) -> Result<Option<RecordWithSessionInfo>, AppError> {
        let row = sqlx::query(
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
        .await?;

        let info = row.map(|r| RecordWithSessionInfo {
            record_id: r.get("record_id"),
            session_id: r.get("attendance_session_id"),
            student_id: r.get("student_id"),
            old_status: r.get("old_status"),
            marked_by: r.get("marked_by"),
            is_locked: r.get("is_locked"),
            created_at: r.get("created_at"),
            class_section_id: r.get("class_section_id"),
        });

        Ok(info)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Verifies if a given user is the designated class teacher of the section.
    // 2. Joins & Foreign Keys:
    //    - class_sections.class_id -> classes.id
    //    - class_sections.class_teacher_staff_id references staff.id
    //    - Subquery (SELECT id FROM staff WHERE user_id = $3 AND school_id = $2) resolves auth user_id to staff.id
    // 3. Why & Correctness:
    //    - CRITICAL: class_teacher_staff_id points to staff.id, NOT users.id directly.
    //    - Resolving user_id -> staff.id guarantees class teacher capabilities work flawlessly.
    // =========================================================================
    pub async fn check_is_class_teacher(
        pool: &PgPool,
        school_id: Uuid,
        class_section_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, AppError> {
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
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        Ok(is_class_teacher)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Deletes an attendance session and all its associated student records in a single atomic transaction.
    // 2. Joins & Foreign Keys:
    //    - attendance_records.attendance_session_id -> attendance_sessions.id
    //    - attendance_sessions.school_id -> schools.id
    // 3. Why & Correctness:
    //    - Ensures transactional integrity: both child records and the parent session are removed atomically.
    // =========================================================================
    pub async fn delete_session_and_records(
        pool: &PgPool,
        school_id: Uuid,
        session_id: Uuid,
    ) -> Result<(), AppError> {
        let mut tx = pool.begin().await?;

        sqlx::query(
            "DELETE FROM attendance_records WHERE attendance_session_id = $1 AND school_id = $2",
        )
        .bind(session_id)
        .bind(school_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM attendance_sessions WHERE id = $1 AND school_id = $2")
            .bind(session_id)
            .bind(school_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(())
    }

    /// Retrieve class-wise daily attendance heat map for a given date range
    pub async fn get_weekly_heatmap(
        pool: &PgPool,
        school_id: Uuid,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<WeeklyHeatmapResponse, AppError> {
        let today = Utc::now().date_naive();

        // 1. Generate Day Headers for the date range
        let mut day_headers = Vec::new();
        let mut curr = start_date;
        while curr <= end_date {
            let day_name = curr.format("%a").to_string();
            let formatted = curr.format("%b %-d").to_string();
            let is_today = curr == today;
            let is_future = curr > today;

            day_headers.push(HeatmapDayHeader {
                date: curr.format("%Y-%m-%d").to_string(),
                day_name,
                formatted,
                is_today,
                is_future,
            });

            curr = match curr.succ_opt() {
                Some(next) => next,
                None => break,
            };
        }

        // 2. Query all active class sections with their attendance on each day
        let rows = sqlx::query(
            r#"
            SELECT 
                cs.id as class_section_id,
                c.name as class_name,
                sec.name as section_name,
                c.code as class_code,
                sec.code as section_code,
                COALESCE(NULLIF(u.display_name, ''), u.username) as class_teacher_name,
                (SELECT COUNT(*) FROM enrollments e WHERE e.class_section_id = cs.id AND e.status = 'ACTIVE') as enrolled_count,
                d.date::date::text as slot_date,
                to_char(d.date, 'Dy') as slot_day_name,
                COUNT(r.id) as total_records,
                COUNT(CASE WHEN r.status = 'PRESENT' THEN 1 END) as present_count,
                COUNT(CASE WHEN r.status = 'ABSENT' THEN 1 END) as absent_count,
                COUNT(CASE WHEN r.status = 'LATE' THEN 1 END) as late_count,
                CASE 
                    WHEN COUNT(r.id) > 0 THEN ROUND((COUNT(CASE WHEN r.status = 'PRESENT' THEN 1 END)::numeric / COUNT(r.id)) * 100)::int
                    ELSE NULL 
                END as attendance_rate,
                MAX(s.id::text) as session_id
            FROM class_sections cs
            JOIN classes c ON cs.class_id = c.id
            JOIN sections sec ON cs.section_id = sec.id
            LEFT JOIN users u ON cs.class_teacher_staff_id = u.id
            CROSS JOIN generate_series($2::date, $3::date, '1 day'::interval) d(date)
            LEFT JOIN attendance_sessions s ON s.class_section_id = cs.id AND s.date = d.date::date AND s.school_id = c.school_id
            LEFT JOIN attendance_records r ON r.attendance_session_id = s.id
            WHERE c.school_id = $1 AND cs.status = 'ACTIVE'
            GROUP BY cs.id, c.name, sec.name, c.code, sec.code, u.display_name, u.username, d.date
            ORDER BY c.name ASC, sec.name ASC, d.date ASC;
            "#,
        )
        .bind(school_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(pool)
        .await?;

        // 3. Group by class_section_id
        use std::collections::BTreeMap;
        struct IntermediateRow {
            class_name: String,
            section_name: String,
            class_code: Option<String>,
            section_code: Option<String>,
            class_teacher_name: Option<String>,
            enrolled_count: i64,
            slots: Vec<ClassDayAttendanceSlot>,
        }

        let mut class_map: BTreeMap<Uuid, IntermediateRow> = BTreeMap::new();
        let mut class_order: Vec<Uuid> = Vec::new();

        for r in rows {
            let cs_id: Uuid = r.get("class_section_id");
            let class_name: String = r.get("class_name");
            let section_name: String = r.get("section_name");
            let class_code: Option<String> = r.try_get("class_code").ok();
            let section_code: Option<String> = r.try_get("section_code").ok();
            let class_teacher_name: Option<String> = r.try_get("class_teacher_name").ok();
            let enrolled_count: i64 = r.try_get("enrolled_count").unwrap_or(0);
            let slot_date: String = r.get("slot_date");
            let slot_day_name: String = r.get("slot_day_name");
            let total_records: i64 = r.try_get("total_records").unwrap_or(0);
            let present_count: i64 = r.try_get("present_count").unwrap_or(0);
            let absent_count: i64 = r.try_get("absent_count").unwrap_or(0);
            let late_count: i64 = r.try_get("late_count").unwrap_or(0);
            let attendance_rate: Option<i32> = r.try_get("attendance_rate").ok();
            let session_id_str: Option<String> = r.try_get("session_id").ok();
            let session_id = session_id_str.and_then(|s| Uuid::parse_str(&s).ok());

            let parsed_date = NaiveDate::parse_from_str(&slot_date, "%Y-%m-%d").unwrap_or(today);
            let status = if total_records > 0 {
                "RECORDED".to_string()
            } else if parsed_date > today {
                "FUTURE".to_string()
            } else if parsed_date == today {
                "PENDING".to_string()
            } else {
                "NOT_RECORDED".to_string()
            };

            let slot = ClassDayAttendanceSlot {
                date: slot_date,
                day_name: slot_day_name,
                rate: attendance_rate,
                status,
                present_count,
                absent_count,
                late_count,
                total_records,
                session_id,
            };

            if !class_map.contains_key(&cs_id) {
                class_order.push(cs_id);
                class_map.insert(
                    cs_id,
                    IntermediateRow {
                        class_name,
                        section_name,
                        class_code,
                        section_code,
                        class_teacher_name,
                        enrolled_count,
                        slots: vec![slot],
                    },
                );
            } else if let Some(entry) = class_map.get_mut(&cs_id) {
                entry.slots.push(slot);
            }
        }

        // 4. Build output rows & summary stats
        let mut result_rows = Vec::new();
        let mut total_rates = Vec::new();
        let mut total_recorded_sessions = 0i64;
        let mut best_class: Option<(String, i32)> = None;
        let mut lowest_class: Option<(String, i32)> = None;

        for cs_id in class_order {
            if let Some(entry) = class_map.remove(&cs_id) {
                let recorded_rates: Vec<i32> = entry
                    .slots
                    .iter()
                    .filter_map(|s| if s.status == "RECORDED" { s.rate } else { None })
                    .collect();

                let weekly_avg_rate = if !recorded_rates.is_empty() {
                    let sum: i32 = recorded_rates.iter().sum();
                    Some((sum as f64 / recorded_rates.len() as f64).round() as i32)
                } else {
                    None
                };

                let class_label = format!("{} - {}", entry.class_name, entry.section_name);
                if let Some(avg) = weekly_avg_rate {
                    total_rates.push(avg);
                    total_recorded_sessions += recorded_rates.len() as i64;

                    match &best_class {
                        None => best_class = Some((class_label.clone(), avg)),
                        Some((_, cur_best)) if avg > *cur_best => {
                            best_class = Some((class_label.clone(), avg))
                        }
                        _ => {}
                    }

                    match &lowest_class {
                        None => lowest_class = Some((class_label.clone(), avg)),
                        Some((_, cur_lowest)) if avg < *cur_lowest => {
                            lowest_class = Some((class_label.clone(), avg))
                        }
                        _ => {}
                    }
                }

                result_rows.push(ClassHeatmapRow {
                    class_section_id: cs_id,
                    class_name: entry.class_name,
                    section_name: entry.section_name,
                    class_code: entry.class_code,
                    section_code: entry.section_code,
                    class_teacher_name: entry.class_teacher_name,
                    enrolled_count: entry.enrolled_count,
                    days: entry.slots,
                    weekly_avg_rate,
                });
            }
        }

        let overall_attendance_rate = if !total_rates.is_empty() {
            let sum: i32 = total_rates.iter().sum();
            Some((sum as f64 / total_rates.len() as f64).round() as i32)
        } else {
            None
        };

        Ok(WeeklyHeatmapResponse {
            start_date: start_date.format("%Y-%m-%d").to_string(),
            end_date: end_date.format("%Y-%m-%d").to_string(),
            day_headers,
            rows: result_rows,
            summary: WeeklyHeatmapSummary {
                overall_attendance_rate,
                total_recorded_sessions,
                best_class_section: best_class.map(|(name, rate)| format!("{name} ({rate}%)")),
                lowest_class_section: lowest_class.map(|(name, rate)| format!("{name} ({rate}%)")),
            },
        })
    }
}
