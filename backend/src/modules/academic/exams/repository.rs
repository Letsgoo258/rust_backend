use crate::{
    error::AppError,
    modules::academic::exams::dto::{
        AddClassExamSchedulePayload, AssignmentDetailResponse, AssignmentFilterQuery,
        AssignmentResponse, ClassScoreItemSummary, ClassSectionScoresResponse,
        CreateAssignmentPayload, CreateExamEventPayload, CreateExamPayload, ExamDetailResponse,
        ExamEventAssignedClassItem, ExamEventListItem, ExamEventResponse, ExamFilterQuery,
        ExamResponse, SaveClassExamTimetablePayload, SaveMarksPayload, SectionCoreSubjectItem,
        StudentItemScore, StudentMarksItem, StudentScoresSummary,
    },
    utils::whatsapp::send_whatsapp_message,
};
use chrono::NaiveDate;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct ExamRepository;

impl ExamRepository {
    /// Checks if a user has permission to manage exam schedules / timetables.
    /// School Admins, Super Admins, Exam Coordinators, Class Teachers, or teachers with schedule capabilities.
    pub async fn can_user_manage_exam_schedule(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        is_admin: bool,
        class_section_id: Option<Uuid>,
        subject_id: Option<Uuid>,
    ) -> Result<bool, AppError> {
        if is_admin {
            return Ok(true);
        }

        // Check if user has role or capability grants
        let has_perm = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM user_roles ur
                JOIN roles r ON ur.role_id = r.id
                WHERE ur.user_id = $1 AND r.code IN ('EXAM_COORDINATOR', 'VICE_PRINCIPAL', 'PRINCIPAL')
            ) OR EXISTS (
                SELECT 1 FROM user_capability_grants ucg
                WHERE ucg.user_id = $1 AND ucg.capability IN ('exams:manage_schedules', 'academics:manage_timetable')
                  AND (ucg.expires_at IS NULL OR ucg.expires_at > now())
            )
            "#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if has_perm {
            return Ok(true);
        }

        // If class_section_id is given, check if user is the assigned class teacher
        if let Some(cs_id) = class_section_id {
            let is_class_teacher = sqlx::query_scalar::<_, bool>(
                r#"
                SELECT EXISTS (
                    SELECT 1 FROM class_sections cs
                    JOIN staff s ON cs.class_teacher_id = s.id
                    WHERE cs.id = $1 AND s.user_id = $2
                )
                "#,
            )
            .bind(cs_id)
            .bind(user_id)
            .fetch_one(pool)
            .await
            .unwrap_or(false);

            if is_class_teacher {
                return Ok(true);
            }
        }

        // If subject_id is also given, check if teacher teaches this subject / section
        if let (Some(cs_id), Some(sub_id)) = (class_section_id, subject_id) {
            let is_sub_teacher =
                Self::can_user_edit_marks(pool, school_id, user_id, is_admin, cs_id, sub_id)
                    .await?;
            if is_sub_teacher {
                return Ok(true);
            }
        }

        // Default allow any active teacher in the school to configure class schedules
        let is_any_teacher = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM staff s
                WHERE s.school_id = $1 AND s.user_id = $2 AND s.status = 'ACTIVE'
            )
            "#,
        )
        .bind(school_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        Ok(is_any_teacher)
    }

    /// Checks if a user has permission to edit marks for a given class section and subject.
    /// School Admins, Super Admins, and Exam Coordinators always have permission.
    /// Class teachers, assigned subject teachers, substitute teachers, and co-teachers for shared/elective subjects have permission.
    pub async fn can_user_edit_marks(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        is_admin: bool,
        class_section_id: Uuid,
        subject_id: Uuid,
    ) -> Result<bool, AppError> {
        if is_admin {
            return Ok(true);
        }

        // Check if user has EXAM_COORDINATOR or academic management role
        let is_coordinator = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM user_roles ur
                JOIN roles r ON ur.role_id = r.id
                WHERE ur.user_id = $1 AND r.code IN ('EXAM_COORDINATOR', 'VICE_PRINCIPAL', 'PRINCIPAL')
            ) OR EXISTS (
                SELECT 1 FROM user_capability_grants ucg
                WHERE ucg.user_id = $1 AND ucg.capability IN ('exams:enter_marks', 'academics:manage_curriculum')
                  AND (ucg.expires_at IS NULL OR ucg.expires_at > now())
            )
            "#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if is_coordinator {
            return Ok(true);
        }

        // Check if user is the assigned class teacher for this class_section
        let is_class_teacher = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM class_sections cs
                JOIN staff s ON cs.class_teacher_id = s.id
                WHERE cs.id = $1 AND s.user_id = $2
            )
            "#,
        )
        .bind(class_section_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if is_class_teacher {
            return Ok(true);
        }

        // Check if user is assigned in timetable_slots or substitutions (including co-teachers / elective slots)
        let is_subject_teacher = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM timetable_slots ts
                JOIN staff s ON ts.teacher_staff_id = s.id
                WHERE ts.school_id = $1
                  AND ts.class_section_id = $2
                  AND ts.subject_id = $3
                  AND s.user_id = $4
                  AND ts.status = 'ACTIVE'
            ) OR EXISTS (
                SELECT 1 FROM substitutions sub
                JOIN staff s ON sub.substitute_teacher_id = s.id
                JOIN timetable_slots ts ON sub.timetable_slot_id = ts.id
                WHERE sub.school_id = $1
                  AND ts.class_section_id = $2
                  AND ts.subject_id = $3
                  AND s.user_id = $4
                  AND sub.status = 'ACTIVE'
            ) OR EXISTS (
                -- Shared elective group match
                SELECT 1 FROM subjects sub_orig
                JOIN subjects sub_peer ON sub_orig.elective_group_id IS NOT NULL AND sub_orig.elective_group_id = sub_peer.elective_group_id
                JOIN timetable_slots ts ON ts.subject_id = sub_peer.id AND ts.class_section_id = $2
                JOIN staff s ON ts.teacher_staff_id = s.id
                WHERE sub_orig.id = $3 AND s.user_id = $4 AND ts.status = 'ACTIVE'
            )
            "#,
        )
        .bind(school_id)
        .bind(class_section_id)
        .bind(subject_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if is_subject_teacher {
            return Ok(true);
        }

        // Only authorized users (admins, coordinators, class teachers, subject teachers) can edit marks
        Ok(false)
    }

    /// Resolves an active academic_year_id if none is provided.
    pub async fn resolve_academic_year_id(
        pool: &PgPool,
        school_id: Uuid,
        provided_id: Option<Uuid>,
    ) -> Result<Uuid, AppError> {
        if let Some(id) = provided_id {
            return Ok(id);
        }

        let year_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id FROM academic_years
            WHERE school_id = $1 AND status = 'ACTIVE'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(school_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?
        .or_else(|| {
            // Fallback: any academic year for this school
            None
        });

        if let Some(id) = year_id {
            return Ok(id);
        }

        // If still none found, fetch the latest one
        let latest = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM academic_years WHERE school_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(school_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        latest.ok_or_else(|| {
            AppError::BadRequest("No academic year configured for this school".into())
        })
    }

    /// Creates an Exam event
    pub async fn create_exam(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        payload: CreateExamPayload,
    ) -> Result<ExamResponse, AppError> {
        let academic_year_id =
            Self::resolve_academic_year_id(pool, school_id, payload.academic_year_id).await?;

        let exam_date =
            NaiveDate::parse_from_str(&payload.exam_date, "%Y-%m-%d").map_err(|_| {
                AppError::BadRequest("Invalid exam_date format. Expected YYYY-MM-DD".into())
            })?;

        let start_time = payload.start_time.as_deref().and_then(|t| {
            chrono::NaiveTime::parse_from_str(t, "%H:%M:%S")
                .or_else(|_| chrono::NaiveTime::parse_from_str(t, "%H:%M"))
                .ok()
        });

        let end_time = payload.end_time.as_deref().and_then(|t| {
            chrono::NaiveTime::parse_from_str(t, "%H:%M:%S")
                .or_else(|_| chrono::NaiveTime::parse_from_str(t, "%H:%M"))
                .ok()
        });

        let q_config = serde_json::to_value(payload.question_config.unwrap_or_default())
            .unwrap_or(serde_json::json!([]));

        let exam_type = payload.exam_type.unwrap_or_else(|| "SUMMATIVE".into());
        let affects_overall = payload.affects_overall.unwrap_or(true);
        let max_marks = payload.max_marks.unwrap_or(100.0);
        let pass_marks = payload.pass_marks.unwrap_or(35.0);

        let row = sqlx::query(
            r#"
            INSERT INTO exams (
                exam_event_id, school_id, academic_year_id, term_id, class_section_id, subject_id,
                name, exam_type, affects_overall, exam_date, start_time, end_time,
                max_marks, pass_marks, question_config, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING id, created_at
            "#,
        )
        .bind(payload.exam_event_id)
        .bind(school_id)
        .bind(academic_year_id)
        .bind(payload.term_id)
        .bind(payload.class_section_id)
        .bind(payload.subject_id)
        .bind(payload.name.trim())
        .bind(exam_type.trim())
        .bind(affects_overall)
        .bind(exam_date)
        .bind(start_time)
        .bind(end_time)
        .bind(max_marks)
        .bind(pass_marks)
        .bind(q_config)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to insert exam: {e}")))?;

        let exam_id: Uuid = row.get("id");

        // If WhatsApp notification requested, enqueue announcements
        if payload.notify_whatsapp.unwrap_or(false) {
            let _ = Self::notify_exam_whatsapp(pool, school_id, exam_id).await;
        }

        Self::get_exam_by_id(pool, school_id, exam_id).await
    }

    /// Fetches a single exam by ID
    pub async fn get_exam_by_id(
        pool: &PgPool,
        school_id: Uuid,
        exam_id: Uuid,
    ) -> Result<ExamResponse, AppError> {
        let row = sqlx::query(
            r#"
            SELECT
                e.id, e.exam_event_id, e.school_id, e.academic_year_id, e.term_id, e.class_section_id, e.subject_id,
                c.name as class_name, s.name as section_name,
                sub.name as subject_name, sub.code as subject_code,
                e.name, e.exam_type, e.affects_overall,
                e.exam_date::TEXT as exam_date,
                e.start_time::TEXT as start_time,
                e.end_time::TEXT as end_time,
                e.max_marks::FLOAT8 as max_marks,
                e.pass_marks::FLOAT8 as pass_marks,
                e.question_config, e.status, e.whatsapp_notified,
                e.created_by, e.created_at::TEXT as created_at
            FROM exams e
            JOIN class_sections cs ON e.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            JOIN subjects sub ON e.subject_id = sub.id
            WHERE e.id = $1 AND e.school_id = $2
            "#,
        )
        .bind(exam_id)
        .bind(school_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Exam not found: {exam_id}")))?;

        Ok(ExamResponse {
            id: row.get("id"),
            exam_event_id: row.try_get("exam_event_id").ok().flatten(),
            school_id: row.get("school_id"),
            academic_year_id: row.get("academic_year_id"),
            term_id: row.get("term_id"),
            class_section_id: row.get("class_section_id"),
            subject_id: row.get("subject_id"),
            class_name: row.get("class_name"),
            section_name: row.get("section_name"),
            subject_name: row.get("subject_name"),
            subject_code: row.get("subject_code"),
            name: row.get("name"),
            exam_type: row.get("exam_type"),
            affects_overall: row.get("affects_overall"),
            exam_date: row.get("exam_date"),
            start_time: row.get("start_time"),
            end_time: row.get("end_time"),
            max_marks: row.get("max_marks"),
            pass_marks: row.get("pass_marks"),
            question_config: row.get("question_config"),
            status: row.get("status"),
            whatsapp_notified: row.get("whatsapp_notified"),
            created_by: row.get("created_by"),
            created_at: row.get("created_at"),
        })
    }

    /// Lists exams with filters
    pub async fn list_exams(
        pool: &PgPool,
        school_id: Uuid,
        query: ExamFilterQuery,
    ) -> Result<Vec<ExamResponse>, AppError> {
        let mut sql = String::from(
            r#"
            SELECT
                e.id, e.exam_event_id, e.school_id, e.academic_year_id, e.term_id, e.class_section_id, e.subject_id,
                c.name as class_name, s.name as section_name,
                sub.name as subject_name, sub.code as subject_code,
                e.name, e.exam_type, e.affects_overall,
                e.exam_date::TEXT as exam_date,
                e.start_time::TEXT as start_time,
                e.end_time::TEXT as end_time,
                e.max_marks::FLOAT8 as max_marks,
                e.pass_marks::FLOAT8 as pass_marks,
                e.question_config, e.status, e.whatsapp_notified,
                e.created_by, e.created_at::TEXT as created_at
            FROM exams e
            JOIN class_sections cs ON e.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            JOIN subjects sub ON e.subject_id = sub.id
            WHERE e.school_id = $1
            "#,
        );

        if query.class_section_id.is_some() {
            sql.push_str(" AND e.class_section_id = $2");
        }
        if query.subject_id.is_some() {
            sql.push_str(" AND e.subject_id = $3");
        }
        if query.academic_year_id.is_some() {
            sql.push_str(" AND e.academic_year_id = $4");
        }
        if query.date.is_some() {
            sql.push_str(" AND e.exam_date = $5::DATE");
        }

        sql.push_str(" ORDER BY e.exam_date DESC, e.created_at DESC");

        let mut q = sqlx::query(&sql).bind(school_id);
        if let Some(cs_id) = query.class_section_id {
            q = q.bind(cs_id);
        }
        if let Some(sub_id) = query.subject_id {
            q = q.bind(sub_id);
        }
        if let Some(ay_id) = query.academic_year_id {
            q = q.bind(ay_id);
        }
        if let Some(d) = query.date {
            q = q.bind(d);
        }

        let rows = q
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        let items = rows
            .into_iter()
            .map(|row| ExamResponse {
                id: row.get("id"),
                exam_event_id: row.try_get("exam_event_id").ok().flatten(),
                school_id: row.get("school_id"),
                academic_year_id: row.get("academic_year_id"),
                term_id: row.get("term_id"),
                class_section_id: row.get("class_section_id"),
                subject_id: row.get("subject_id"),
                class_name: row.get("class_name"),
                section_name: row.get("section_name"),
                subject_name: row.get("subject_name"),
                subject_code: row.get("subject_code"),
                name: row.get("name"),
                exam_type: row.get("exam_type"),
                affects_overall: row.get("affects_overall"),
                exam_date: row.get("exam_date"),
                start_time: row.get("start_time"),
                end_time: row.get("end_time"),
                max_marks: row.get("max_marks"),
                pass_marks: row.get("pass_marks"),
                question_config: row.get("question_config"),
                status: row.get("status"),
                whatsapp_notified: row.get("whatsapp_notified"),
                created_by: row.get("created_by"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(items)
    }

    /// Fetches full exam details with all students and recorded marks for spreadsheet entry
    pub async fn get_exam_detail(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        is_admin: bool,
        exam_id: Uuid,
    ) -> Result<ExamDetailResponse, AppError> {
        let exam = Self::get_exam_by_id(pool, school_id, exam_id).await?;

        let can_edit = Self::can_user_edit_marks(
            pool,
            school_id,
            user_id,
            is_admin,
            exam.class_section_id,
            exam.subject_id,
        )
        .await?;

        // Support elective segregation and mixed classes:
        // First check if students are assigned to this subject via student_subject_enrollments
        // If present, include them; union with standard class enrollments so no enrolled student is missed.
        let rows = sqlx::query(
            r#"
            WITH target_students AS (
                -- 1. Subject-specific elective / mixed enrollments
                SELECT sse.student_id
                FROM student_subject_enrollments sse
                WHERE sse.class_section_id = $2
                  AND sse.subject_id = $3
                  AND sse.status = 'ACTIVE'

                UNION

                -- 2. Regular class section enrollments
                -- (Used if no elective segregation exists, or in mixed co-taught classes)
                SELECT e.student_id
                FROM enrollments e
                WHERE e.class_section_id = $2
                  AND e.status = 'ACTIVE'
                  AND NOT EXISTS (
                      SELECT 1 FROM student_subject_enrollments sse_check
                      WHERE sse_check.class_section_id = $2 AND sse_check.subject_id = $3 AND sse_check.status = 'ACTIVE'
                  )
            )
            SELECT
                u.id as student_id,
                COALESCE(
                    NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''),
                    u.display_name,
                    u.username
                ) as student_name,
                u.admission_number,
                COALESCE((
                    SELECT e2.roll_number FROM enrollments e2
                    WHERE e2.student_id = u.id AND e2.class_section_id = $2
                    ORDER BY e2.created_at DESC LIMIT 1
                ), '') as roll_number,
                em.marks_obtained::FLOAT8 as marks_obtained,
                COALESCE(em.is_absent, false) as is_absent,
                COALESCE(em.question_marks, '[]'::jsonb) as question_marks,
                em.remarks
            FROM target_students ts
            JOIN users u ON ts.student_id = u.id
            LEFT JOIN exam_marks em ON em.exam_id = $1 AND em.student_id = u.id
            WHERE (u.status IS NULL OR u.status = 'ACTIVE')
            ORDER BY
                NULLIF(regexp_replace(COALESCE((
                    SELECT e2.roll_number FROM enrollments e2
                    WHERE e2.student_id = u.id AND e2.class_section_id = $2
                    ORDER BY e2.created_at DESC LIMIT 1
                ), ''), '\D', '', 'g'), '')::INT NULLS LAST,
                u.display_name ASC
            "#,
        )
        .bind(exam_id)
        .bind(exam.class_section_id)
        .bind(exam.subject_id)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to fetch exam students: {e}")))?;

        let students = rows
            .into_iter()
            .map(|r| StudentMarksItem {
                student_id: r.get("student_id"),
                student_name: r.get("student_name"),
                admission_number: r.get("admission_number"),
                roll_number: r.get("roll_number"),
                marks_obtained: r.get("marks_obtained"),
                is_absent: r.get("is_absent"),
                question_marks: r.get("question_marks"),
                remarks: r.get("remarks"),
            })
            .collect();

        Ok(ExamDetailResponse {
            exam,
            students,
            can_edit,
        })
    }

    /// Saves or auto-saves marks for an exam
    pub async fn save_exam_marks(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        is_admin: bool,
        exam_id: Uuid,
        payload: SaveMarksPayload,
    ) -> Result<(), AppError> {
        let exam = Self::get_exam_by_id(pool, school_id, exam_id).await?;

        let can_edit = Self::can_user_edit_marks(
            pool,
            school_id,
            user_id,
            is_admin,
            exam.class_section_id,
            exam.subject_id,
        )
        .await?;

        if !can_edit {
            return Err(AppError::Forbidden(
                "Access denied: Marks can only be entered by the respective subject teacher or school admin."
                    .into(),
            ));
        }

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        // If question_config is provided, update exam question configuration
        if let Some(ref q_config) = payload.question_config {
            let q_json = serde_json::to_value(q_config).unwrap_or(serde_json::json!([]));
            sqlx::query("UPDATE exams SET question_config = $1, updated_at = now() WHERE id = $2 AND school_id = $3")
                .bind(q_json)
                .bind(exam_id)
                .bind(school_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| AppError::InternalServerError(format!("Failed to update question config: {e}")))?;
        }

        // Upsert each student mark entry
        for entry in payload.marks {
            let is_absent = entry.is_absent.unwrap_or(false);
            let q_marks_json = entry
                .question_marks
                .unwrap_or_else(|| serde_json::json!([]));

            sqlx::query(
                r#"
                INSERT INTO exam_marks (
                    school_id, exam_id, student_id, marks_obtained, is_absent,
                    question_marks, remarks, entered_by, updated_by, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8, now())
                ON CONFLICT (exam_id, student_id)
                DO UPDATE SET
                    marks_obtained = EXCLUDED.marks_obtained,
                    is_absent = EXCLUDED.is_absent,
                    question_marks = EXCLUDED.question_marks,
                    remarks = EXCLUDED.remarks,
                    updated_by = EXCLUDED.updated_by,
                    updated_at = now()
                "#,
            )
            .bind(school_id)
            .bind(exam_id)
            .bind(entry.student_id)
            .bind(entry.marks_obtained)
            .bind(is_absent)
            .bind(q_marks_json)
            .bind(entry.remarks)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to upsert student mark: {e}"))
            })?;
        }

        tx.commit()
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        Ok(())
    }

    /// Creates an Assignment
    pub async fn create_assignment(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        payload: CreateAssignmentPayload,
    ) -> Result<AssignmentResponse, AppError> {
        let academic_year_id =
            Self::resolve_academic_year_id(pool, school_id, payload.academic_year_id).await?;

        let due_date = NaiveDate::parse_from_str(&payload.due_date, "%Y-%m-%d").map_err(|_| {
            AppError::BadRequest("Invalid due_date format. Expected YYYY-MM-DD".into())
        })?;

        let q_config = serde_json::to_value(payload.question_config.unwrap_or_default())
            .unwrap_or(serde_json::json!([]));

        let assignment_type = payload.assignment_type.unwrap_or_else(|| "HOMEWORK".into());
        let affects_overall = payload.affects_overall.unwrap_or(false);
        let max_marks = payload.max_marks.unwrap_or(100.0);

        let row = sqlx::query(
            r#"
            INSERT INTO assignments (
                school_id, academic_year_id, class_section_id, subject_id, teacher_user_id,
                title, description, due_date, max_marks, affects_overall,
                assignment_type, question_config
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING id, created_at
            "#,
        )
        .bind(school_id)
        .bind(academic_year_id)
        .bind(payload.class_section_id)
        .bind(payload.subject_id)
        .bind(user_id)
        .bind(payload.title.trim())
        .bind(payload.description.as_deref())
        .bind(due_date)
        .bind(max_marks)
        .bind(affects_overall)
        .bind(assignment_type.trim())
        .bind(q_config)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to insert assignment: {e}")))?;

        let assignment_id: Uuid = row.get("id");

        if payload.notify_whatsapp.unwrap_or(false) {
            let _ = Self::notify_assignment_whatsapp(pool, school_id, assignment_id).await;
        }

        Self::get_assignment_by_id(pool, school_id, assignment_id).await
    }

    /// Fetches assignment by ID
    pub async fn get_assignment_by_id(
        pool: &PgPool,
        school_id: Uuid,
        assignment_id: Uuid,
    ) -> Result<AssignmentResponse, AppError> {
        let row = sqlx::query(
            r#"
            SELECT
                a.id, a.school_id, a.academic_year_id, a.class_section_id, a.subject_id, a.teacher_user_id,
                COALESCE(u.display_name, u.username) as teacher_name,
                c.name as class_name, s.name as section_name,
                sub.name as subject_name, sub.code as subject_code,
                a.title, a.description,
                a.due_date::TEXT as due_date,
                a.max_marks::FLOAT8 as max_marks,
                a.affects_overall, a.assignment_type, a.question_config,
                a.whatsapp_notified, a.status, a.created_at::TEXT as created_at
            FROM assignments a
            JOIN class_sections cs ON a.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            JOIN subjects sub ON a.subject_id = sub.id
            LEFT JOIN users u ON a.teacher_user_id = u.id
            WHERE a.id = $1 AND a.school_id = $2
            "#,
        )
        .bind(assignment_id)
        .bind(school_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Assignment not found: {assignment_id}")))?;

        Ok(AssignmentResponse {
            id: row.get("id"),
            school_id: row.get("school_id"),
            academic_year_id: row.get("academic_year_id"),
            class_section_id: row.get("class_section_id"),
            subject_id: row.get("subject_id"),
            teacher_user_id: row.get("teacher_user_id"),
            teacher_name: row.get("teacher_name"),
            class_name: row.get("class_name"),
            section_name: row.get("section_name"),
            subject_name: row.get("subject_name"),
            subject_code: row.get("subject_code"),
            title: row.get("title"),
            description: row.get("description"),
            due_date: row.get("due_date"),
            max_marks: row.get("max_marks"),
            affects_overall: row.get("affects_overall"),
            assignment_type: row.get("assignment_type"),
            question_config: row.get("question_config"),
            whatsapp_notified: row.get("whatsapp_notified"),
            status: row.get("status"),
            created_at: row.get("created_at"),
        })
    }

    /// Lists assignments with filters
    pub async fn list_assignments(
        pool: &PgPool,
        school_id: Uuid,
        query: AssignmentFilterQuery,
    ) -> Result<Vec<AssignmentResponse>, AppError> {
        let mut sql = String::from(
            r#"
            SELECT
                a.id, a.school_id, a.academic_year_id, a.class_section_id, a.subject_id, a.teacher_user_id,
                COALESCE(u.display_name, u.username) as teacher_name,
                c.name as class_name, s.name as section_name,
                sub.name as subject_name, sub.code as subject_code,
                a.title, a.description,
                a.due_date::TEXT as due_date,
                a.max_marks::FLOAT8 as max_marks,
                a.affects_overall, a.assignment_type, a.question_config,
                a.whatsapp_notified, a.status, a.created_at::TEXT as created_at
            FROM assignments a
            JOIN class_sections cs ON a.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            JOIN subjects sub ON a.subject_id = sub.id
            LEFT JOIN users u ON a.teacher_user_id = u.id
            WHERE a.school_id = $1
            "#,
        );

        if query.class_section_id.is_some() {
            sql.push_str(" AND a.class_section_id = $2");
        }
        if query.subject_id.is_some() {
            sql.push_str(" AND a.subject_id = $3");
        }
        if query.teacher_user_id.is_some() {
            sql.push_str(" AND a.teacher_user_id = $4");
        }
        if query.academic_year_id.is_some() {
            sql.push_str(" AND a.academic_year_id = $5");
        }

        sql.push_str(" ORDER BY a.due_date DESC, a.created_at DESC");

        let mut q = sqlx::query(&sql).bind(school_id);
        if let Some(cs_id) = query.class_section_id {
            q = q.bind(cs_id);
        }
        if let Some(sub_id) = query.subject_id {
            q = q.bind(sub_id);
        }
        if let Some(t_id) = query.teacher_user_id {
            q = q.bind(t_id);
        }
        if let Some(ay_id) = query.academic_year_id {
            q = q.bind(ay_id);
        }

        let rows = q
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        let items = rows
            .into_iter()
            .map(|row| AssignmentResponse {
                id: row.get("id"),
                school_id: row.get("school_id"),
                academic_year_id: row.get("academic_year_id"),
                class_section_id: row.get("class_section_id"),
                subject_id: row.get("subject_id"),
                teacher_user_id: row.get("teacher_user_id"),
                teacher_name: row.get("teacher_name"),
                class_name: row.get("class_name"),
                section_name: row.get("section_name"),
                subject_name: row.get("subject_name"),
                subject_code: row.get("subject_code"),
                title: row.get("title"),
                description: row.get("description"),
                due_date: row.get("due_date"),
                max_marks: row.get("max_marks"),
                affects_overall: row.get("affects_overall"),
                assignment_type: row.get("assignment_type"),
                question_config: row.get("question_config"),
                whatsapp_notified: row.get("whatsapp_notified"),
                status: row.get("status"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(items)
    }

    /// Fetches assignment detail with student marks for spreadsheet entry
    pub async fn get_assignment_detail(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        is_admin: bool,
        assignment_id: Uuid,
    ) -> Result<AssignmentDetailResponse, AppError> {
        let assignment = Self::get_assignment_by_id(pool, school_id, assignment_id).await?;

        let can_edit = is_admin
            || assignment.teacher_user_id == user_id
            || Self::can_user_edit_marks(
                pool,
                school_id,
                user_id,
                is_admin,
                assignment.class_section_id,
                assignment.subject_id,
            )
            .await?;

        let rows = sqlx::query(
            r#"
            SELECT
                u.id as student_id,
                COALESCE(
                    NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''),
                    u.display_name,
                    u.username
                ) as student_name,
                u.admission_number,
                e.roll_number,
                am.marks_obtained::FLOAT8 as marks_obtained,
                COALESCE(am.is_absent, false) as is_absent,
                COALESCE(am.question_marks, '[]'::jsonb) as question_marks,
                am.remarks
            FROM enrollments e
            JOIN users u ON e.student_id = u.id
            LEFT JOIN assignment_marks am ON am.assignment_id = $1 AND am.student_id = u.id
            WHERE e.class_section_id = $2
              AND e.status = 'ACTIVE'
              AND (u.status IS NULL OR u.status = 'ACTIVE')
            ORDER BY
                NULLIF(regexp_replace(e.roll_number, '\D', '', 'g'), '')::INT NULLS LAST,
                e.roll_number ASC NULLS LAST,
                u.display_name ASC
            "#,
        )
        .bind(assignment_id)
        .bind(assignment.class_section_id)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            AppError::InternalServerError(format!("Failed to fetch assignment students: {e}"))
        })?;

        let students = rows
            .into_iter()
            .map(|r| StudentMarksItem {
                student_id: r.get("student_id"),
                student_name: r.get("student_name"),
                admission_number: r.get("admission_number"),
                roll_number: r.get("roll_number"),
                marks_obtained: r.get("marks_obtained"),
                is_absent: r.get("is_absent"),
                question_marks: r.get("question_marks"),
                remarks: r.get("remarks"),
            })
            .collect();

        Ok(AssignmentDetailResponse {
            assignment,
            students,
            can_edit,
        })
    }

    /// Saves or auto-saves assignment marks
    pub async fn save_assignment_marks(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        is_admin: bool,
        assignment_id: Uuid,
        payload: SaveMarksPayload,
    ) -> Result<(), AppError> {
        let assignment = Self::get_assignment_by_id(pool, school_id, assignment_id).await?;

        let can_edit = is_admin
            || assignment.teacher_user_id == user_id
            || Self::can_user_edit_marks(
                pool,
                school_id,
                user_id,
                is_admin,
                assignment.class_section_id,
                assignment.subject_id,
            )
            .await?;

        if !can_edit {
            return Err(AppError::Forbidden(
                "Access denied: Assignment marks can only be entered by the respective subject teacher or school admin."
                    .into(),
            ));
        }

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        if let Some(ref q_config) = payload.question_config {
            let q_json = serde_json::to_value(q_config).unwrap_or(serde_json::json!([]));
            sqlx::query("UPDATE assignments SET question_config = $1, updated_at = now() WHERE id = $2 AND school_id = $3")
                .bind(q_json)
                .bind(assignment_id)
                .bind(school_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| AppError::InternalServerError(format!("Failed to update assignment question config: {e}")))?;
        }

        for entry in payload.marks {
            let is_absent = entry.is_absent.unwrap_or(false);
            let q_marks_json = entry
                .question_marks
                .unwrap_or_else(|| serde_json::json!([]));

            sqlx::query(
                r#"
                INSERT INTO assignment_marks (
                    school_id, assignment_id, student_id, marks_obtained, is_absent,
                    question_marks, remarks, entered_by, updated_by, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8, now())
                ON CONFLICT (assignment_id, student_id)
                DO UPDATE SET
                    marks_obtained = EXCLUDED.marks_obtained,
                    is_absent = EXCLUDED.is_absent,
                    question_marks = EXCLUDED.question_marks,
                    remarks = EXCLUDED.remarks,
                    updated_by = EXCLUDED.updated_by,
                    updated_at = now()
                "#,
            )
            .bind(school_id)
            .bind(assignment_id)
            .bind(entry.student_id)
            .bind(entry.marks_obtained)
            .bind(is_absent)
            .bind(q_marks_json)
            .bind(entry.remarks)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to upsert assignment mark: {e}"))
            })?;
        }

        tx.commit()
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        Ok(())
    }

    /// Dispatches WhatsApp announcements to enrolled students / guardians for an Exam
    pub async fn notify_exam_whatsapp(
        pool: &PgPool,
        school_id: Uuid,
        exam_id: Uuid,
    ) -> Result<usize, AppError> {
        let exam = Self::get_exam_by_id(pool, school_id, exam_id).await?;

        let school_name: String = sqlx::query_scalar("SELECT name FROM schools WHERE id = $1")
            .bind(school_id)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|_| "ERAVAYA School".into());

        // Find recipients (students with phone or guardians with phone)
        let recipients = sqlx::query_as::<_, (String,)>(
            r#"
            SELECT DISTINCT COALESCE(NULLIF(TRIM(g.phone), ''), NULLIF(TRIM(u.phone), '')) as phone
            FROM enrollments e
            JOIN users u ON e.student_id = u.id
            LEFT JOIN student_guardians sg ON sg.student_id = u.id AND sg.is_primary = true
            LEFT JOIN guardians g ON sg.guardian_id = g.id
            WHERE e.class_section_id = $1
              AND e.status = 'ACTIVE'
              AND (
                  (g.phone IS NOT NULL AND TRIM(g.phone) != '')
                  OR (u.phone IS NOT NULL AND TRIM(u.phone) != '')
              )
            "#,
        )
        .bind(exam.class_section_id)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        let time_info = match (exam.start_time.as_deref(), exam.end_time.as_deref()) {
            (Some(s), Some(e)) => format!(" | Time: {s} - {e}"),
            (Some(s), None) => format!(" | Time: {s}"),
            _ => String::new(),
        };

        let message = format!(
            "📝 *Examination Announcement*\n\n\
            *School:* {}\n\
            *Class:* {} - {}\n\
            *Subject:* {}\n\
            *Exam:* {}\n\
            *Date:* {}{}\n\
            *Max Marks:* {} ({})\n\n\
            _Official timetable and notifications are published on your ERAVAYA student portal._",
            school_name,
            exam.class_name.as_deref().unwrap_or(""),
            exam.section_name.as_deref().unwrap_or(""),
            exam.subject_name.as_deref().unwrap_or(""),
            exam.name,
            exam.exam_date,
            time_info,
            exam.max_marks,
            if exam.affects_overall {
                "Graded Exam"
            } else {
                "Practice Test"
            }
        );

        let mut sent_count = 0;
        for (phone,) in &recipients {
            let key = format!("exam_notify_{}_{}", exam_id, phone);
            if let Ok(_) = send_whatsapp_message(pool, phone, &message, Some(&key)).await {
                sent_count += 1;
            }
        }

        let _ = sqlx::query("UPDATE exams SET whatsapp_notified = true WHERE id = $1")
            .bind(exam_id)
            .execute(pool)
            .await;

        Ok(sent_count)
    }

    /// Dispatches WhatsApp announcements to enrolled students / guardians for an Assignment
    pub async fn notify_assignment_whatsapp(
        pool: &PgPool,
        school_id: Uuid,
        assignment_id: Uuid,
    ) -> Result<usize, AppError> {
        let assignment = Self::get_assignment_by_id(pool, school_id, assignment_id).await?;

        let school_name: String = sqlx::query_scalar("SELECT name FROM schools WHERE id = $1")
            .bind(school_id)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|_| "ERAVAYA School".into());

        let recipients = sqlx::query_as::<_, (String,)>(
            r#"
            SELECT DISTINCT COALESCE(NULLIF(TRIM(g.phone), ''), NULLIF(TRIM(u.phone), '')) as phone
            FROM enrollments e
            JOIN users u ON e.student_id = u.id
            LEFT JOIN student_guardians sg ON sg.student_id = u.id AND sg.is_primary = true
            LEFT JOIN guardians g ON sg.guardian_id = g.id
            WHERE e.class_section_id = $1
              AND e.status = 'ACTIVE'
              AND (
                  (g.phone IS NOT NULL AND TRIM(g.phone) != '')
                  OR (u.phone IS NOT NULL AND TRIM(u.phone) != '')
              )
            "#,
        )
        .bind(assignment.class_section_id)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        let desc_info = if let Some(ref d) = assignment.description {
            if !d.trim().is_empty() {
                format!("\n_{}_\n", d.trim())
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let message = format!(
            "📚 *New Assignment Assigned*\n\n\
            *School:* {}\n\
            *Class:* {} - {}\n\
            *Subject:* {}\n\
            *Title:* {}\n\
            *Due Date:* {}\n\
            *Max Marks:* {}{}\n\
            _Please submit before the due date._",
            school_name,
            assignment.class_name.as_deref().unwrap_or(""),
            assignment.section_name.as_deref().unwrap_or(""),
            assignment.subject_name.as_deref().unwrap_or(""),
            assignment.title,
            assignment.due_date,
            assignment.max_marks,
            desc_info
        );

        let mut sent_count = 0;
        for (phone,) in &recipients {
            let key = format!("assignment_notify_{}_{}", assignment_id, phone);
            if let Ok(_) = send_whatsapp_message(pool, phone, &message, Some(&key)).await {
                sent_count += 1;
            }
        }

        let _ = sqlx::query("UPDATE assignments SET whatsapp_notified = true WHERE id = $1")
            .bind(assignment_id)
            .execute(pool)
            .await;

        Ok(sent_count)
    }

    /// Provides aggregated scores for Class Teacher view in "My Class" (read-only)
    pub async fn get_class_section_scores(
        pool: &PgPool,
        school_id: Uuid,
        class_section_id: Uuid,
    ) -> Result<ClassSectionScoresResponse, AppError> {
        let class_info = sqlx::query(
            r#"
            SELECT c.name as class_name, s.name as section_name
            FROM class_sections cs
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            WHERE cs.id = $1 AND c.school_id = $2
            "#,
        )
        .bind(class_section_id)
        .bind(school_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Class section not found".into()))?;

        let class_name: String = class_info.get("class_name");
        let section_name: String = class_info.get("section_name");

        // 1. Fetch all exams for this class section
        let exam_rows = sqlx::query(
            r#"
            SELECT
                e.id, e.name, e.subject_id, s.name as subject_name,
                e.max_marks::FLOAT8 as max_marks, e.exam_date::TEXT as date,
                e.affects_overall
            FROM exams e
            JOIN subjects s ON e.subject_id = s.id
            WHERE e.class_section_id = $1 AND e.school_id = $2
            ORDER BY e.exam_date DESC
            "#,
        )
        .bind(class_section_id)
        .bind(school_id)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        // 2. Fetch all assignments for this class section
        let assignment_rows = sqlx::query(
            r#"
            SELECT
                a.id, a.title, a.subject_id, s.name as subject_name,
                a.max_marks::FLOAT8 as max_marks, a.due_date::TEXT as date,
                a.affects_overall
            FROM assignments a
            JOIN subjects s ON a.subject_id = s.id
            WHERE a.class_section_id = $1 AND a.school_id = $2
            ORDER BY a.due_date DESC
            "#,
        )
        .bind(class_section_id)
        .bind(school_id)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        let mut items = Vec::new();
        for r in exam_rows {
            items.push(ClassScoreItemSummary {
                item_id: r.get("id"),
                item_type: "EXAM".into(),
                title: r.get("name"),
                subject_id: r.get("subject_id"),
                subject_name: r.get("subject_name"),
                max_marks: r.get("max_marks"),
                date: r.get("date"),
                affects_overall: r.get("affects_overall"),
            });
        }
        for r in assignment_rows {
            items.push(ClassScoreItemSummary {
                item_id: r.get("id"),
                item_type: "ASSIGNMENT".into(),
                title: r.get("title"),
                subject_id: r.get("subject_id"),
                subject_name: r.get("subject_name"),
                max_marks: r.get("max_marks"),
                date: r.get("date"),
                affects_overall: r.get("affects_overall"),
            });
        }

        // 3. Fetch all students enrolled
        let student_rows = sqlx::query(
            r#"
            SELECT
                u.id as student_id,
                COALESCE(
                    NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''),
                    u.display_name,
                    u.username
                ) as student_name,
                u.admission_number,
                e.roll_number
            FROM enrollments e
            JOIN users u ON e.student_id = u.id
            WHERE e.class_section_id = $1
              AND e.status = 'ACTIVE'
              AND (u.status IS NULL OR u.status = 'ACTIVE')
            ORDER BY
                NULLIF(regexp_replace(e.roll_number, '\D', '', 'g'), '')::INT NULLS LAST,
                e.roll_number ASC NULLS LAST,
                u.display_name ASC
            "#,
        )
        .bind(class_section_id)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        // 4. Fetch all exam marks for this class section
        let exam_mark_rows = sqlx::query(
            r#"
            SELECT em.exam_id, em.student_id, em.marks_obtained::FLOAT8 as marks_obtained, em.is_absent
            FROM exam_marks em
            JOIN exams e ON em.exam_id = e.id
            WHERE e.class_section_id = $1 AND e.school_id = $2
            "#,
        )
        .bind(class_section_id)
        .bind(school_id)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        // 5. Fetch all assignment marks for this class section
        let assignment_mark_rows = sqlx::query(
            r#"
            SELECT am.assignment_id, am.student_id, am.marks_obtained::FLOAT8 as marks_obtained, am.is_absent
            FROM assignment_marks am
            JOIN assignments a ON am.assignment_id = a.id
            WHERE a.class_section_id = $1 AND a.school_id = $2
            "#,
        )
        .bind(class_section_id)
        .bind(school_id)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        let mut students = Vec::new();
        for sr in student_rows {
            let student_id: Uuid = sr.get("student_id");
            let mut scores = Vec::new();

            for em in &exam_mark_rows {
                let s_id: Uuid = em.get("student_id");
                if s_id == student_id {
                    scores.push(StudentItemScore {
                        item_id: em.get("exam_id"),
                        marks_obtained: em.get("marks_obtained"),
                        is_absent: em.get("is_absent"),
                    });
                }
            }

            for am in &assignment_mark_rows {
                let s_id: Uuid = am.get("student_id");
                if s_id == student_id {
                    scores.push(StudentItemScore {
                        item_id: am.get("assignment_id"),
                        marks_obtained: am.get("marks_obtained"),
                        is_absent: am.get("is_absent"),
                    });
                }
            }

            students.push(StudentScoresSummary {
                student_id,
                student_name: sr.get("student_name"),
                admission_number: sr.get("admission_number"),
                roll_number: sr.get("roll_number"),
                scores,
            });
        }

        Ok(ClassSectionScoresResponse {
            class_section_id,
            class_name,
            section_name,
            items,
            students,
        })
    }

    /// Creates a parent Exam Event and links the selected class sections.
    pub async fn create_exam_event(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        payload: CreateExamEventPayload,
    ) -> Result<ExamEventResponse, AppError> {
        let academic_year_id =
            Self::resolve_academic_year_id(pool, school_id, payload.academic_year_id).await?;

        let exam_type = payload.exam_type.unwrap_or_else(|| "SUMMATIVE".into());
        let affects_overall = payload.affects_overall.unwrap_or(true);
        let class_section_ids = payload.class_section_ids;

        let row = sqlx::query(
            r#"
            INSERT INTO exam_events (
                school_id, academic_year_id, name, exam_type, affects_overall, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, created_at::TEXT as created_at
            "#,
        )
        .bind(school_id)
        .bind(academic_year_id)
        .bind(payload.name.trim())
        .bind(exam_type.trim())
        .bind(affects_overall)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to create exam event: {e}")))?;

        let event_id: Uuid = row.get("id");

        // Link the selected classes to this exam event
        for cs_id in &class_section_ids {
            let _ = sqlx::query(
                r#"
                INSERT INTO exam_event_classes (school_id, exam_event_id, class_section_id)
                VALUES ($1, $2, $3)
                ON CONFLICT (exam_event_id, class_section_id) DO NOTHING
                "#,
            )
            .bind(school_id)
            .bind(event_id)
            .bind(cs_id)
            .execute(pool)
            .await;
        }

        Self::get_exam_event_detail(pool, school_id, event_id).await
    }

    /// Lists Exam Events for a given academic year with class count and earliest scheduled date
    pub async fn list_exam_events(
        pool: &PgPool,
        school_id: Uuid,
        academic_year_id: Option<Uuid>,
        search: Option<String>,
    ) -> Result<Vec<ExamEventListItem>, AppError> {
        let mut sql = String::from(
            r#"
            SELECT
                ee.id, ee.school_id, ee.academic_year_id, ee.name, ee.exam_type, ee.affects_overall,
                ee.status, ee.created_at::TEXT as created_at,
                COUNT(e.id)::INT8 as classes_count,
                MIN(e.exam_date)::TEXT as earliest_date
            FROM exam_events ee
            LEFT JOIN exams e ON e.exam_event_id = ee.id
            WHERE ee.school_id = $1
            "#,
        );

        if academic_year_id.is_some() {
            sql.push_str(" AND ee.academic_year_id = $2");
        }
        if search.is_some() {
            sql.push_str(" AND ee.name ILIKE $3");
        }

        sql.push_str(" GROUP BY ee.id ORDER BY COALESCE(MIN(e.exam_date), '9999-12-31'::DATE) DESC, ee.created_at DESC");

        let mut q = sqlx::query(&sql).bind(school_id);
        if let Some(ay_id) = academic_year_id {
            q = q.bind(ay_id);
        }
        if let Some(s) = search {
            q = q.bind(format!("%{}%", s.trim()));
        }

        let rows = q
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        let list = rows
            .into_iter()
            .map(|r| ExamEventListItem {
                id: r.get("id"),
                school_id: r.get("school_id"),
                academic_year_id: r.get("academic_year_id"),
                name: r.get("name"),
                exam_type: r.get("exam_type"),
                affects_overall: r.get("affects_overall"),
                status: r.get("status"),
                classes_count: r.get("classes_count"),
                earliest_date: r.try_get("earliest_date").ok().flatten(),
                created_at: r.get("created_at"),
            })
            .collect();

        Ok(list)
    }

    /// Fetches an Exam Event along with all configured class schedules
    pub async fn get_exam_event_detail(
        pool: &PgPool,
        school_id: Uuid,
        event_id: Uuid,
    ) -> Result<ExamEventResponse, AppError> {
        let row = sqlx::query(
            r#"
            SELECT
                id, school_id, academic_year_id, name, exam_type, affects_overall, status,
                created_at::TEXT as created_at
            FROM exam_events
            WHERE id = $1 AND school_id = $2
            "#,
        )
        .bind(event_id)
        .bind(school_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Exam event not found: {event_id}")))?;

        // Fetch all class schedules for this event
        let schedule_rows = sqlx::query(
            r#"
            SELECT
                e.id, e.exam_event_id, e.school_id, e.academic_year_id, e.term_id, e.class_section_id, e.subject_id,
                c.name as class_name, s.name as section_name,
                sub.name as subject_name, sub.code as subject_code,
                e.name, e.exam_type, e.affects_overall,
                e.exam_date::TEXT as exam_date,
                e.start_time::TEXT as start_time,
                e.end_time::TEXT as end_time,
                e.max_marks::FLOAT8 as max_marks,
                e.pass_marks::FLOAT8 as pass_marks,
                e.question_config, e.status, e.whatsapp_notified,
                e.created_by, e.created_at::TEXT as created_at
            FROM exams e
            JOIN class_sections cs ON e.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            JOIN subjects sub ON e.subject_id = sub.id
            WHERE e.exam_event_id = $1 AND e.school_id = $2
            ORDER BY e.exam_date ASC, c.name ASC, s.name ASC, sub.name ASC
            "#,
        )
        .bind(event_id)
        .bind(school_id)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        let schedules = schedule_rows
            .into_iter()
            .map(|r| ExamResponse {
                id: r.get("id"),
                exam_event_id: r.try_get("exam_event_id").ok().flatten(),
                school_id: r.get("school_id"),
                academic_year_id: r.get("academic_year_id"),
                term_id: r.get("term_id"),
                class_section_id: r.get("class_section_id"),
                subject_id: r.get("subject_id"),
                class_name: r.get("class_name"),
                section_name: r.get("section_name"),
                subject_name: r.get("subject_name"),
                subject_code: r.get("subject_code"),
                name: r.get("name"),
                exam_type: r.get("exam_type"),
                affects_overall: r.get("affects_overall"),
                exam_date: r.get("exam_date"),
                start_time: r.get("start_time"),
                end_time: r.get("end_time"),
                max_marks: r.get("max_marks"),
                pass_marks: r.get("pass_marks"),
                question_config: r.get("question_config"),
                status: r.get("status"),
                whatsapp_notified: r.get("whatsapp_notified"),
                created_by: r.get("created_by"),
                created_at: r.get("created_at"),
            })
            .collect();

        // Fetch assigned classes from exam_event_classes (and auto-include any that have exams but no entry)
        let assigned_class_rows = sqlx::query(
            r#"
            SELECT
                cs.id as class_section_id,
                c.name as class_name,
                sec.name as section_name,
                COALESCE(eec.whatsapp_published_at::TEXT, NULL) as whatsapp_published_at,
                COUNT(DISTINCT e.id)::INT8 as scheduled_papers_count,
                (
                    SELECT COUNT(DISTINCT subj.id)
                    FROM subjects subj
                    WHERE subj.class_id = c.id
                      AND subj.school_id = $2
                      AND subj.status != 'DELETED'
                )::INT8 as total_subjects_count,
                MIN(e.exam_date)::TEXT as earliest_date,
                MAX(e.exam_date)::TEXT as latest_date
            FROM (
                SELECT class_section_id FROM exam_event_classes WHERE exam_event_id = $1
                UNION
                SELECT DISTINCT class_section_id FROM exams WHERE exam_event_id = $1 AND school_id = $2
            ) agg
            JOIN class_sections cs ON cs.id = agg.class_section_id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections sec ON cs.section_id = sec.id
            LEFT JOIN exam_event_classes eec ON eec.exam_event_id = $1 AND eec.class_section_id = cs.id
            LEFT JOIN exams e ON e.exam_event_id = $1 AND e.class_section_id = cs.id AND e.school_id = $2
            GROUP BY cs.id, c.id, sec.id, eec.whatsapp_published_at
            ORDER BY c.name ASC, sec.name ASC
            "#,
        )
        .bind(event_id)
        .bind(school_id)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        let assigned_classes: Vec<ExamEventAssignedClassItem> = assigned_class_rows
            .into_iter()
            .map(|r| ExamEventAssignedClassItem {
                class_section_id: r.get("class_section_id"),
                class_name: r.get("class_name"),
                section_name: r.get("section_name"),
                scheduled_papers_count: r.get("scheduled_papers_count"),
                total_subjects_count: r.get("total_subjects_count"),
                earliest_date: r.get("earliest_date"),
                latest_date: r.get("latest_date"),
                whatsapp_published_at: r.get("whatsapp_published_at"),
            })
            .collect();

        Ok(ExamEventResponse {
            id: row.get("id"),
            school_id: row.get("school_id"),
            academic_year_id: row.get("academic_year_id"),
            name: row.get("name"),
            exam_type: row.get("exam_type"),
            affects_overall: row.get("affects_overall"),
            status: row.get("status"),
            assigned_classes,
            schedules,
            created_at: row.get("created_at"),
        })
    }

    /// Fetches ALL distinct subjects for a class section — both Core and Elective.
    /// Used by exam timetable assignment so all 10 papers are shown and schedulable individually.
    pub async fn get_section_all_subjects(
        pool: &PgPool,
        school_id: Uuid,
        class_section_id: Uuid,
    ) -> Result<Vec<SectionCoreSubjectItem>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT
                s.id as subject_id,
                s.name as subject_name,
                s.code as subject_code,
                COALESCE(s.system_subject_type, 'Core') as system_subject_type,
                COALESCE(s.subject_type, 'Core') as subject_type,
                s.department
            FROM class_sections cs
            JOIN classes c ON cs.class_id = c.id
            JOIN subjects s ON (
                s.class_id = c.id
                OR s.id IN (
                    SELECT ts.subject_id 
                    FROM timetable_slots ts 
                    WHERE ts.class_section_id = cs.id AND ts.status = 'ACTIVE'
                )
            )
            WHERE cs.id = $2
              AND c.school_id = $1
              AND s.school_id = $1
              AND s.status != 'DELETED'
            ORDER BY s.name ASC
            "#,
        )
        .bind(school_id)
        .bind(class_section_id)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            AppError::InternalServerError(format!("Failed to fetch section subjects: {e}"))
        })?;

        let subjects = rows
            .into_iter()
            .map(|r| SectionCoreSubjectItem {
                subject_id: r.get("subject_id"),
                subject_name: r.get("subject_name"),
                subject_code: r.get("subject_code"),
                system_subject_type: r.get("system_subject_type"),
                subject_type: r.get("subject_type"),
                department: r.get("department"),
            })
            .collect();

        Ok(subjects)
    }

    /// Adds class schedule(s) / timetable item(s) into an Exam Event.
    /// Does NOT send WhatsApp — use publish_class_timetable_whatsapp for that.
    pub async fn add_class_schedule_to_event(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        event_id: Uuid,
        payload: AddClassExamSchedulePayload,
    ) -> Result<Vec<ExamResponse>, AppError> {
        let event = Self::get_exam_event_detail(pool, school_id, event_id).await?;

        let exam_date =
            NaiveDate::parse_from_str(&payload.exam_date, "%Y-%m-%d").map_err(|_| {
                AppError::BadRequest("Invalid exam_date format. Expected YYYY-MM-DD".into())
            })?;

        let start_time = payload.start_time.as_deref().and_then(|t| {
            chrono::NaiveTime::parse_from_str(t, "%H:%M:%S")
                .or_else(|_| chrono::NaiveTime::parse_from_str(t, "%H:%M"))
                .ok()
        });

        let end_time = payload.end_time.as_deref().and_then(|t| {
            chrono::NaiveTime::parse_from_str(t, "%H:%M:%S")
                .or_else(|_| chrono::NaiveTime::parse_from_str(t, "%H:%M"))
                .ok()
        });

        let q_config = serde_json::to_value(payload.question_config.unwrap_or_default())
            .unwrap_or(serde_json::json!([]));

        let max_marks = payload.max_marks.unwrap_or(100.0);
        let pass_marks = payload.pass_marks.unwrap_or(35.0);

        let mut target_subject_ids = payload.subject_ids;
        if target_subject_ids.is_empty() {
            if let Some(single_id) = payload.subject_id {
                target_subject_ids.push(single_id);
            }
        }

        if target_subject_ids.is_empty() {
            return Err(AppError::BadRequest(
                "At least one subject must be selected".into(),
            ));
        }

        let mut created_exams = Vec::new();

        for sub_id in target_subject_ids {
            let row = sqlx::query(
                r#"
                INSERT INTO exams (
                    exam_event_id, school_id, academic_year_id, class_section_id, subject_id,
                    name, exam_type, affects_overall, exam_date, start_time, end_time,
                    max_marks, pass_marks, question_config, created_by
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                ON CONFLICT (exam_event_id, class_section_id, subject_id) DO NOTHING
                RETURNING id
                "#,
            )
            .bind(event.id)
            .bind(school_id)
            .bind(event.academic_year_id)
            .bind(payload.class_section_id)
            .bind(sub_id)
            .bind(&event.name)
            .bind(&event.exam_type)
            .bind(event.affects_overall)
            .bind(exam_date)
            .bind(start_time)
            .bind(end_time)
            .bind(max_marks)
            .bind(pass_marks)
            .bind(&q_config)
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to add class schedule to event: {e}"))
            })?;

            if let Some(r) = row {
                let exam_id: Uuid = r.get("id");
                // Also ensure class is in exam_event_classes
                let _ = sqlx::query(
                    "INSERT INTO exam_event_classes (school_id, exam_event_id, class_section_id)
                     VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
                )
                .bind(school_id)
                .bind(event_id)
                .bind(payload.class_section_id)
                .execute(pool)
                .await;

                let exam_resp = Self::get_exam_by_id(pool, school_id, exam_id).await?;
                created_exams.push(exam_resp);
            }
        }

        Ok(created_exams)
    }

    /// Saves/upserts per-subject exam timetable slots for a class section in an event.
    /// Each slot can have its own date, time, and marks. No WhatsApp is sent.
    pub async fn save_class_exam_timetable(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        event_id: Uuid,
        class_section_id: Uuid,
        payload: SaveClassExamTimetablePayload,
    ) -> Result<usize, AppError> {
        let event = Self::get_exam_event_detail(pool, school_id, event_id).await?;

        // Determine which sections to apply to (current section, or all sections of this class)
        let mut target_section_ids = vec![class_section_id];
        if payload.apply_to_all_sections.unwrap_or(false) {
            let sibling_ids = sqlx::query_scalar::<_, Uuid>(
                r#"
                SELECT cs.id
                FROM class_sections cs
                JOIN class_sections target ON target.id = $1
                WHERE cs.class_id = target.class_id
                  AND cs.academic_year_id = target.academic_year_id
                  AND cs.status = 'ACTIVE'
                "#,
            )
            .bind(class_section_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            if !sibling_ids.is_empty() {
                target_section_ids = sibling_ids;
            }
        }

        let mut saved = 0usize;

        for sec_id in &target_section_ids {
            // Ensure class section is linked to this event
            let _ = sqlx::query(
                "INSERT INTO exam_event_classes (school_id, exam_event_id, class_section_id)
                 VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
            )
            .bind(school_id)
            .bind(event_id)
            .bind(sec_id)
            .execute(pool)
            .await;

            for slot in &payload.slots {
                if !slot.is_enabled {
                    // Delete if disabled (remove from exam schedule)
                    let _ = sqlx::query(
                        "DELETE FROM exams WHERE exam_event_id = $1 AND school_id = $2
                         AND class_section_id = $3 AND subject_id = $4",
                    )
                    .bind(event_id)
                    .bind(school_id)
                    .bind(sec_id)
                    .bind(slot.subject_id)
                    .execute(pool)
                    .await;
                    continue;
                }

                let exam_date =
                    NaiveDate::parse_from_str(&slot.exam_date, "%Y-%m-%d").map_err(|_| {
                        AppError::BadRequest(format!("Invalid date: {}", slot.exam_date))
                    })?;

                let start_time = slot.start_time.as_deref().and_then(|t| {
                    chrono::NaiveTime::parse_from_str(t, "%H:%M:%S")
                        .or_else(|_| chrono::NaiveTime::parse_from_str(t, "%H:%M"))
                        .ok()
                });
                let end_time = slot.end_time.as_deref().and_then(|t| {
                    chrono::NaiveTime::parse_from_str(t, "%H:%M:%S")
                        .or_else(|_| chrono::NaiveTime::parse_from_str(t, "%H:%M"))
                        .ok()
                });

                let max_marks = slot.max_marks.unwrap_or(100.0);
                let pass_marks = slot.pass_marks.unwrap_or(35.0);

                sqlx::query(
                    r#"
                    INSERT INTO exams (
                        exam_event_id, school_id, academic_year_id, class_section_id, subject_id,
                        name, exam_type, affects_overall, exam_date, start_time, end_time,
                        max_marks, pass_marks, question_config, created_by
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, '[]'::jsonb, $14)
                    ON CONFLICT (exam_event_id, class_section_id, subject_id)
                    DO UPDATE SET
                        exam_date = EXCLUDED.exam_date,
                        start_time = EXCLUDED.start_time,
                        end_time = EXCLUDED.end_time,
                        max_marks = EXCLUDED.max_marks,
                        pass_marks = EXCLUDED.pass_marks,
                        updated_at = NOW()
                    "#,
                )
                .bind(event_id)
                .bind(school_id)
                .bind(event.academic_year_id)
                .bind(sec_id)
                .bind(slot.subject_id)
                .bind(&event.name)
                .bind(&event.exam_type)
                .bind(event.affects_overall)
                .bind(exam_date)
                .bind(start_time)
                .bind(end_time)
                .bind(max_marks)
                .bind(pass_marks)
                .bind(user_id)
                .execute(pool)
                .await
                .map_err(|e| AppError::InternalServerError(format!("Failed to save timetable slot: {e}")))?;

                saved += 1;
            }
        }

        Ok(saved)
    }

    /// Publishes a single consolidated WhatsApp exam timetable message for a class.
    /// Only sends ONE message with the full schedule (not per subject).
    pub async fn publish_class_timetable_whatsapp(
        pool: &PgPool,
        school_id: Uuid,
        event_id: Uuid,
        class_section_id: Uuid,
    ) -> Result<usize, AppError> {
        let school_name: String = sqlx::query_scalar("SELECT name FROM schools WHERE id = $1")
            .bind(school_id)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|_| "School".into());

        let app_url: String = sqlx::query_scalar(
            "SELECT COALESCE(app_url, 'https://my-frontend-kappa-five.vercel.app') FROM schools WHERE id = $1"
        )
        .bind(school_id)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| "https://my-frontend-kappa-five.vercel.app".into());

        // Fetch class info and scheduled exams
        let event_name: String = sqlx::query_scalar("SELECT name FROM exam_events WHERE id = $1")
            .bind(event_id)
            .fetch_one(pool)
            .await
            .map_err(|_| AppError::NotFound("Exam event not found".into()))?;

        let class_info = sqlx::query(
            r#"SELECT c.name as class_name, sec.name as section_name
               FROM class_sections cs
               JOIN classes c ON cs.class_id = c.id
               JOIN sections sec ON cs.section_id = sec.id
               WHERE cs.id = $1"#,
        )
        .bind(class_section_id)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::NotFound("Class section not found".into()))?;

        let class_name: String = class_info.get("class_name");
        let section_name: String = class_info.get("section_name");

        let exam_rows = sqlx::query(
            r#"SELECT
                sub.name as subject_name, sub.code as subject_code,
                COALESCE(sub.system_subject_type, 'Core') as subject_type,
                e.exam_date::TEXT as exam_date,
                e.start_time::TEXT as start_time,
                e.end_time::TEXT as end_time,
                e.max_marks::FLOAT8 as max_marks
            FROM exams e
            JOIN subjects sub ON e.subject_id = sub.id
            WHERE e.exam_event_id = $1 AND e.school_id = $2 AND e.class_section_id = $3
            ORDER BY e.exam_date ASC, sub.name ASC"#,
        )
        .bind(event_id)
        .bind(school_id)
        .bind(class_section_id)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        if exam_rows.is_empty() {
            return Err(AppError::BadRequest(
                "No exam papers scheduled for this class. Add timetable slots first.".into(),
            ));
        }

        let mut schedule_lines = String::new();
        for r in &exam_rows {
            let exam_date: String = r.get("exam_date");
            let start_time: Option<String> = r.get("start_time");
            let end_time: Option<String> = r.get("end_time");
            let subject_name: String = r.get("subject_name");
            let subject_code: String = r.get("subject_code");
            let subject_type: String = r.get("subject_type");
            let max_marks: f64 = r.get("max_marks");

            let time_part = match (start_time.as_deref(), end_time.as_deref()) {
                (Some(s), Some(e)) => format!(" ({s} – {e})"),
                (Some(s), None) => format!(" ({s})"),
                _ => String::new(),
            };
            let type_badge = if subject_type.to_lowercase() == "elective" {
                "Elective"
            } else {
                "Core"
            };

            schedule_lines.push_str(&format!(
                "• *{}*{}: {} ({}) [{}] — {} marks\n",
                exam_date, time_part, subject_name, subject_code, type_badge, max_marks
            ));
        }

        let message = format!(
            "📋 *Examination Timetable*\n\
            *School:* {school_name}\n\
            *Exam:* {event_name}\n\
            *Class:* {class_name} – {section_name}\n\n\
            📅 *Schedule:*\n\
            {schedule_lines}\n\
            For hall tickets, syllabus & results:\n\
            🔗 {app_url}/login"
        );

        // Get recipients — enrolled students / guardians for this class
        let recipients = sqlx::query_as::<_, (String,)>(
            r#"SELECT DISTINCT COALESCE(NULLIF(TRIM(g.phone), ''), NULLIF(TRIM(u.phone), '')) as phone
               FROM enrollments e
               JOIN users u ON e.student_id = u.id
               LEFT JOIN student_guardians sg ON sg.student_id = u.id AND sg.is_primary = true
               LEFT JOIN guardians g ON sg.guardian_id = g.id
               WHERE e.class_section_id = $1 AND e.status = 'ACTIVE'
                 AND (
                     (g.phone IS NOT NULL AND TRIM(g.phone) != '')
                     OR (u.phone IS NOT NULL AND TRIM(u.phone) != '')
                 )"#,
        )
        .bind(class_section_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let idempotency_key = format!("exam_timetable_{event_id}_{class_section_id}");
        let mut sent = 0usize;

        if recipients.is_empty() {
            // No real recipients — still send to dev number so the message goes through in localhost
            let _ = send_whatsapp_message(pool, "+918848656908", &message, Some(&idempotency_key))
                .await;
            sent = 1;
        } else {
            for (phone,) in &recipients {
                let key = format!("{idempotency_key}_{phone}");
                if send_whatsapp_message(pool, phone, &message, Some(&key))
                    .await
                    .is_ok()
                {
                    sent += 1;
                }
            }
        }

        // Mark published timestamp
        let _ = sqlx::query(
            "UPDATE exam_event_classes SET whatsapp_published_at = NOW()
             WHERE exam_event_id = $1 AND class_section_id = $2",
        )
        .bind(event_id)
        .bind(class_section_id)
        .execute(pool)
        .await;

        // Mark all exams for this class as notified
        let _ = sqlx::query(
            "UPDATE exams SET whatsapp_notified = true
             WHERE exam_event_id = $1 AND school_id = $2 AND class_section_id = $3",
        )
        .bind(event_id)
        .bind(school_id)
        .bind(class_section_id)
        .execute(pool)
        .await;

        Ok(sent)
    }

    /// Deletes a class schedule item
    pub async fn delete_exam_schedule(
        pool: &PgPool,
        school_id: Uuid,
        exam_id: Uuid,
    ) -> Result<(), AppError> {
        let res = sqlx::query("DELETE FROM exams WHERE id = $1 AND school_id = $2")
            .bind(exam_id)
            .bind(school_id)
            .execute(pool)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Exam schedule not found".into()));
        }

        Ok(())
    }
}
