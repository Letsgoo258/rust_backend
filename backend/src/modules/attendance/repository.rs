use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{
    error::AppError,
    modules::attendance::dto::{
        AttendanceConfigDto, AttendanceCorrectionLog, AttendanceSessionDetailResponse,
        AttendanceSessionSummary, CollisionItem, StudentAttendanceDetail, StudentAttendanceInput,
        StudentRosterItem, TodayAttendanceStats,
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
                ), u.admission_number, '') as roll_number,
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
            &Uuid::new_v4().to_string()[..6].to_uppercase()
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
}
