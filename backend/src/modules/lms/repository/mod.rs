use crate::error::AppError;
use crate::modules::lms::domain::{LmsTask, TaskStatus, TaskType};
use crate::modules::lms::dto::*;
use chrono::{Duration, Utc};
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

pub struct LmsRepository;

impl LmsRepository {
    /// Create a new LMS task record
    pub async fn create_task(
        pool: &PgPool,
        school_id: Uuid,
        creator_id: Uuid,
        req: &CreateLmsTaskRequest,
    ) -> Result<LmsTask, AppError> {
        let academic_year_id = match req.academic_year_id {
            Some(y) => y,
            None => {
                let yr: Option<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM academic_years WHERE school_id = $1 AND status = 'ACTIVE' LIMIT 1",
                )
                .bind(school_id)
                .fetch_optional(pool)
                .await?;

                let yr = match yr {
                    Some(id) => Some(id),
                    None => {
                        sqlx::query_scalar(
                            "SELECT id FROM academic_years WHERE school_id = $1 ORDER BY start_date DESC LIMIT 1",
                        )
                        .bind(school_id)
                        .fetch_optional(pool)
                        .await?
                    }
                };

                yr.ok_or_else(|| {
                    AppError::BadRequest("No academic year configured for this school".into())
                })?
            }
        };

        let t_type = req
            .task_type
            .as_deref()
            .map(|s| TaskType::from_str_loose(s).as_str())
            .unwrap_or("HOMEWORK");

        let assigned_at = req.assigned_at.unwrap_or_else(Utc::now);
        let add_to_calendar = req.add_to_calendar.unwrap_or(false);
        let notify_whatsapp = req.notify_whatsapp.unwrap_or(false);
        let notify_email = req.notify_email.unwrap_or(false);
        let is_evaluation = req.is_evaluation.unwrap_or(false);

        let row = sqlx::query(
            r#"
            INSERT INTO lms_tasks (
                school_id, academic_year_id, title, description, special_message,
                task_type, class_section_id, subject_id, teaching_group_id,
                attendance_session_id, created_by, assigned_at, deadline_at,
                status, add_to_calendar, notify_whatsapp, notify_email, is_evaluation
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, $9,
                $10, $11, $12, $13,
                'DRAFT', $14, $15, $16, $17
            )
            RETURNING
                id, school_id, academic_year_id, title, description, special_message,
                task_type, class_section_id, subject_id, teaching_group_id,
                attendance_session_id, created_by, assigned_at, deadline_at,
                status, add_to_calendar, calendar_event_id, notify_whatsapp,
                notify_email, is_evaluation, published_at, created_at, updated_at
            "#,
        )
        .bind(school_id)
        .bind(academic_year_id)
        .bind(req.title.trim())
        .bind(req.description.trim())
        .bind(req.special_message.as_deref().map(str::trim))
        .bind(t_type)
        .bind(req.class_section_id)
        .bind(req.subject_id)
        .bind(req.teaching_group_id)
        .bind(req.attendance_session_id)
        .bind(creator_id)
        .bind(assigned_at)
        .bind(req.deadline_at)
        .bind(add_to_calendar)
        .bind(notify_whatsapp)
        .bind(notify_email)
        .bind(is_evaluation)
        .fetch_one(pool)
        .await?;

        let task = Self::row_to_lms_task(&row);

        // Audit log
        Self::create_audit_log(
            pool,
            school_id,
            task.id,
            Some(creator_id),
            "CREATED",
            serde_json::json!({
                "title": task.title,
                "task_type": task.task_type,
                "class_section_id": task.class_section_id,
                "is_evaluation": task.is_evaluation
            }),
        )
        .await?;

        Ok(task)
    }

    /// Helper to map row to LmsTask
    fn row_to_lms_task(r: &sqlx::postgres::PgRow) -> LmsTask {
        LmsTask {
            id: r.get("id"),
            school_id: r.get("school_id"),
            academic_year_id: r.get("academic_year_id"),
            title: r.get("title"),
            description: r.get("description"),
            special_message: r.try_get("special_message").ok(),
            task_type: r.get("task_type"),
            class_section_id: r.get("class_section_id"),
            subject_id: r.try_get("subject_id").ok(),
            teaching_group_id: r.try_get("teaching_group_id").ok(),
            attendance_session_id: r.try_get("attendance_session_id").ok(),
            created_by: r.get("created_by"),
            assigned_at: r.get("assigned_at"),
            deadline_at: r.try_get("deadline_at").ok(),
            status: r.get("status"),
            add_to_calendar: r.get("add_to_calendar"),
            calendar_event_id: r.try_get("calendar_event_id").ok(),
            notify_whatsapp: r.get("notify_whatsapp"),
            notify_email: r.get("notify_email"),
            is_evaluation: r.get("is_evaluation"),
            published_at: r.try_get("published_at").ok(),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }
    }

    /// Update an existing draft or active task
    pub async fn update_task(
        pool: &PgPool,
        school_id: Uuid,
        task_id: Uuid,
        actor_id: Uuid,
        req: &UpdateLmsTaskRequest,
    ) -> Result<LmsTask, AppError> {
        let existing = Self::get_task_by_id(pool, school_id, task_id).await?;

        let title = req
            .title
            .as_deref()
            .map(str::trim)
            .unwrap_or(&existing.title);
        let description = req
            .description
            .as_deref()
            .map(str::trim)
            .unwrap_or(&existing.description);
        let special_message = req
            .special_message
            .as_deref()
            .map(str::trim)
            .or(existing.special_message.as_deref());
        let task_type = req
            .task_type
            .as_deref()
            .map(|s| TaskType::from_str_loose(s).as_str().to_string())
            .unwrap_or(existing.task_type);
        let class_section_id = req.class_section_id.unwrap_or(existing.class_section_id);
        let subject_id = if req.subject_id.is_some() {
            req.subject_id
        } else {
            existing.subject_id
        };
        let teaching_group_id = if req.teaching_group_id.is_some() {
            req.teaching_group_id
        } else {
            existing.teaching_group_id
        };
        let attendance_session_id = if req.attendance_session_id.is_some() {
            req.attendance_session_id
        } else {
            existing.attendance_session_id
        };
        let assigned_at = req.assigned_at.unwrap_or(existing.assigned_at);
        let deadline_at = if req.deadline_at.is_some() {
            req.deadline_at
        } else {
            existing.deadline_at
        };
        let add_to_calendar = req.add_to_calendar.unwrap_or(existing.add_to_calendar);
        let notify_whatsapp = req.notify_whatsapp.unwrap_or(existing.notify_whatsapp);
        let notify_email = req.notify_email.unwrap_or(existing.notify_email);
        let is_evaluation = req.is_evaluation.unwrap_or(existing.is_evaluation);

        let row = sqlx::query(
            r#"
            UPDATE lms_tasks
            SET
                title = $1,
                description = $2,
                special_message = $3,
                task_type = $4,
                class_section_id = $5,
                subject_id = $6,
                teaching_group_id = $7,
                attendance_session_id = $8,
                assigned_at = $9,
                deadline_at = $10,
                add_to_calendar = $11,
                notify_whatsapp = $12,
                notify_email = $13,
                is_evaluation = $14,
                updated_at = NOW()
            WHERE id = $15 AND school_id = $16
            RETURNING
                id, school_id, academic_year_id, title, description, special_message,
                task_type, class_section_id, subject_id, teaching_group_id,
                attendance_session_id, created_by, assigned_at, deadline_at,
                status, add_to_calendar, calendar_event_id, notify_whatsapp,
                notify_email, is_evaluation, published_at, created_at, updated_at
            "#,
        )
        .bind(title)
        .bind(description)
        .bind(special_message)
        .bind(task_type)
        .bind(class_section_id)
        .bind(subject_id)
        .bind(teaching_group_id)
        .bind(attendance_session_id)
        .bind(assigned_at)
        .bind(deadline_at)
        .bind(add_to_calendar)
        .bind(notify_whatsapp)
        .bind(notify_email)
        .bind(is_evaluation)
        .bind(task_id)
        .bind(school_id)
        .fetch_one(pool)
        .await?;

        let updated = Self::row_to_lms_task(&row);

        // Audit log
        Self::create_audit_log(
            pool,
            school_id,
            task_id,
            Some(actor_id),
            "UPDATED",
            serde_json::json!({
                "title": updated.title,
                "deadline_at": updated.deadline_at,
                "attendance_session_id": updated.attendance_session_id
            }),
        )
        .await?;

        Ok(updated)
    }

    /// Retrieve a task by ID
    pub async fn get_task_by_id(
        pool: &PgPool,
        school_id: Uuid,
        task_id: Uuid,
    ) -> Result<LmsTask, AppError> {
        let row = sqlx::query(
            r#"
            SELECT
                id, school_id, academic_year_id, title, description, special_message,
                task_type, class_section_id, subject_id, teaching_group_id,
                attendance_session_id, created_by, assigned_at, deadline_at,
                status, add_to_calendar, calendar_event_id, notify_whatsapp,
                notify_email, is_evaluation, published_at, created_at, updated_at
            FROM lms_tasks
            WHERE id = $1 AND school_id = $2
            "#,
        )
        .bind(task_id)
        .bind(school_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("LMS Task not found".into()))?;

        Ok(Self::row_to_lms_task(&row))
    }

    /// Add an attachment to a task
    pub async fn add_attachment(
        pool: &PgPool,
        school_id: Uuid,
        task_id: Uuid,
        uploaded_by: Uuid,
        input: &CreateAttachmentInput,
    ) -> Result<LmsAttachmentResponse, AppError> {
        let is_gdrive = input
            .attachment_type
            .trim()
            .eq_ignore_ascii_case("GOOGLE_DRIVE");
        let att_type = if is_gdrive {
            "GOOGLE_DRIVE"
        } else {
            "TEMPORARY_UPLOAD"
        };

        let expires_at = if !is_gdrive {
            Some(Utc::now() + Duration::days(7))
        } else {
            None
        };

        let file_url = if is_gdrive {
            input
                .google_drive_url
                .clone()
                .unwrap_or_else(|| input.file_url.clone())
        } else {
            input.file_url.clone()
        };

        let row = sqlx::query(
            r#"
            INSERT INTO lms_task_attachments (
                school_id, task_id, attachment_type, original_filename,
                mime_type, file_size, imagekit_file_id, file_url,
                google_drive_url, uploaded_by, expires_at
            ) VALUES (
                $1, $2, $3, $4,
                $5, $6, $7, $8,
                $9, $10, $11
            )
            RETURNING
                id, attachment_type, original_filename, mime_type, file_size,
                file_url, google_drive_url, expires_at, deletion_status, uploaded_at
            "#,
        )
        .bind(school_id)
        .bind(task_id)
        .bind(att_type)
        .bind(&input.original_filename)
        .bind(&input.mime_type)
        .bind(input.file_size)
        .bind(&input.imagekit_file_id)
        .bind(&file_url)
        .bind(&input.google_drive_url)
        .bind(uploaded_by)
        .bind(expires_at)
        .fetch_one(pool)
        .await?;

        // Audit log
        Self::create_audit_log(
            pool,
            school_id,
            task_id,
            Some(uploaded_by),
            "ATTACHMENT_ADDED",
            serde_json::json!({
                "attachment_id": row.get::<Uuid, _>("id"),
                "attachment_type": row.get::<String, _>("attachment_type"),
                "original_filename": row.try_get::<String, _>("original_filename").ok()
            }),
        )
        .await?;

        Ok(LmsAttachmentResponse {
            id: row.get("id"),
            attachment_type: row.get("attachment_type"),
            original_filename: row.try_get("original_filename").ok(),
            mime_type: row.try_get("mime_type").ok(),
            file_size: row.try_get("file_size").ok(),
            file_url: row.get("file_url"),
            google_drive_url: row.try_get("google_drive_url").ok(),
            expires_at: row.try_get("expires_at").ok(),
            deletion_status: row.get("deletion_status"),
            uploaded_at: row.get("uploaded_at"),
        })
    }

    /// List attachments for a task
    pub async fn list_attachments(
        pool: &PgPool,
        school_id: Uuid,
        task_id: Uuid,
    ) -> Result<Vec<LmsAttachmentResponse>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, attachment_type, original_filename, mime_type, file_size,
                file_url, google_drive_url, expires_at, deletion_status, uploaded_at
            FROM lms_task_attachments
            WHERE task_id = $1 AND school_id = $2 AND deletion_status != 'DELETED'
            ORDER BY uploaded_at ASC
            "#,
        )
        .bind(task_id)
        .bind(school_id)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| LmsAttachmentResponse {
                id: r.get("id"),
                attachment_type: r.get("attachment_type"),
                original_filename: r.try_get("original_filename").ok(),
                mime_type: r.try_get("mime_type").ok(),
                file_size: r.try_get("file_size").ok(),
                file_url: r.get("file_url"),
                google_drive_url: r.try_get("google_drive_url").ok(),
                expires_at: r.try_get("expires_at").ok(),
                deletion_status: r.get("deletion_status"),
                uploaded_at: r.get("uploaded_at"),
            })
            .collect())
    }

    /// Remove an attachment
    pub async fn remove_attachment(
        pool: &PgPool,
        school_id: Uuid,
        task_id: Uuid,
        attachment_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), AppError> {
        let res = sqlx::query(
            r#"
            UPDATE lms_task_attachments
            SET deletion_status = 'DELETED', deleted_at = NOW()
            WHERE id = $1 AND task_id = $2 AND school_id = $3
            "#,
        )
        .bind(attachment_id)
        .bind(task_id)
        .bind(school_id)
        .execute(pool)
        .await?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Attachment not found".into()));
        }

        Self::create_audit_log(
            pool,
            school_id,
            task_id,
            Some(actor_id),
            "ATTACHMENT_REMOVED",
            serde_json::json!({ "attachment_id": attachment_id }),
        )
        .await?;

        Ok(())
    }

    /// Upsert evaluation configuration for a task
    pub async fn configure_evaluation(
        pool: &PgPool,
        school_id: Uuid,
        task_id: Uuid,
        created_by: Uuid,
        config: &CreateEvaluationConfig,
    ) -> Result<LmsEvaluationResponse, AppError> {
        let max_marks = config.maximum_marks;
        let pass_marks = config.pass_marks.unwrap_or(35.0);
        let eval_title = config
            .evaluation_title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Task Evaluation");
        let inc_results = config.include_in_results.unwrap_or(false);

        let row = sqlx::query(
            r#"
            INSERT INTO lms_evaluations (
                school_id, task_id, evaluation_title, maximum_marks, pass_marks,
                evaluation_category, evaluation_date, include_in_results,
                result_component_id, status, created_by
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8,
                $9, 'PENDING', $10
            )
            ON CONFLICT (task_id) DO UPDATE SET
                evaluation_title = EXCLUDED.evaluation_title,
                maximum_marks = EXCLUDED.maximum_marks,
                pass_marks = EXCLUDED.pass_marks,
                evaluation_category = EXCLUDED.evaluation_category,
                evaluation_date = EXCLUDED.evaluation_date,
                include_in_results = EXCLUDED.include_in_results,
                result_component_id = EXCLUDED.result_component_id,
                updated_at = NOW()
            RETURNING
                id, task_id, evaluation_title, maximum_marks::FLOAT8 as maximum_marks,
                pass_marks::FLOAT8 as pass_marks, evaluation_category, evaluation_date,
                include_in_results, result_component_id, status, created_at, updated_at
            "#,
        )
        .bind(school_id)
        .bind(task_id)
        .bind(eval_title)
        .bind(max_marks)
        .bind(pass_marks)
        .bind(&config.evaluation_category)
        .bind(config.evaluation_date)
        .bind(inc_results)
        .bind(config.result_component_id)
        .bind(created_by)
        .fetch_one(pool)
        .await?;

        // Update task is_evaluation flag
        let _ = sqlx::query(
            "UPDATE lms_tasks SET is_evaluation = true, updated_at = NOW() WHERE id = $1",
        )
        .bind(task_id)
        .execute(pool)
        .await;

        let eval_id: Uuid = row.get("id");

        // Counts
        let total_students: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM lms_task_recipients WHERE task_id = $1")
                .bind(task_id)
                .fetch_one(pool)
                .await
                .unwrap_or(0);

        let evaluated_students: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM lms_evaluation_marks WHERE evaluation_id = $1 AND marks_obtained IS NOT NULL"
        )
        .bind(eval_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        Self::create_audit_log(
            pool,
            school_id,
            task_id,
            Some(created_by),
            "EVALUATION_CONFIGURED",
            serde_json::json!({
                "evaluation_id": eval_id,
                "maximum_marks": max_marks,
                "include_in_results": inc_results
            }),
        )
        .await?;

        Ok(LmsEvaluationResponse {
            id: eval_id,
            task_id: row.get("task_id"),
            evaluation_title: row.get("evaluation_title"),
            maximum_marks: row.get("maximum_marks"),
            pass_marks: row.get("pass_marks"),
            evaluation_category: row.try_get("evaluation_category").ok(),
            evaluation_date: row.try_get("evaluation_date").ok(),
            include_in_results: row.get("include_in_results"),
            result_component_id: row.try_get("result_component_id").ok(),
            status: row.get("status"),
            total_students,
            evaluated_students,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Retrieve evaluation detail for a task
    pub async fn get_evaluation_by_task_id(
        pool: &PgPool,
        school_id: Uuid,
        task_id: Uuid,
    ) -> Result<Option<LmsEvaluationResponse>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT
                id, task_id, evaluation_title, maximum_marks::FLOAT8 as maximum_marks,
                pass_marks::FLOAT8 as pass_marks, evaluation_category, evaluation_date,
                include_in_results, result_component_id, status, created_at, updated_at
            FROM lms_evaluations
            WHERE task_id = $1 AND school_id = $2
            "#,
        )
        .bind(task_id)
        .bind(school_id)
        .fetch_optional(pool)
        .await?;

        let r = match row {
            Some(x) => x,
            None => return Ok(None),
        };

        let eval_id: Uuid = r.get("id");

        let total_students: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM lms_task_recipients WHERE task_id = $1")
                .bind(task_id)
                .fetch_one(pool)
                .await
                .unwrap_or(0);

        let evaluated_students: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM lms_evaluation_marks WHERE evaluation_id = $1 AND marks_obtained IS NOT NULL"
        )
        .bind(eval_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        Ok(Some(LmsEvaluationResponse {
            id: eval_id,
            task_id: r.get("task_id"),
            evaluation_title: r.get("evaluation_title"),
            maximum_marks: r.get("maximum_marks"),
            pass_marks: r.get("pass_marks"),
            evaluation_category: r.try_get("evaluation_category").ok(),
            evaluation_date: r.try_get("evaluation_date").ok(),
            include_in_results: r.get("include_in_results"),
            result_component_id: r.try_get("result_component_id").ok(),
            status: r.get("status"),
            total_students,
            evaluated_students,
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }

    /// Retrieve students and entered marks for spreadsheet marks entry
    pub async fn get_evaluation_spreadsheet_rows(
        pool: &PgPool,
        school_id: Uuid,
        task_id: Uuid,
    ) -> Result<Vec<LmsEvaluationStudentMarksRow>, AppError> {
        let eval = Self::get_evaluation_by_task_id(pool, school_id, task_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("Task is not configured for evaluation".into()))?;

        let rows = sqlx::query(
            r#"
            SELECT
                r.student_id,
                COALESCE(NULLIF(u.display_name, ''), NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''), u.username) as student_name,
                u.admission_number,
                e.roll_number,
                em.marks_obtained::FLOAT8 as marks_obtained,
                COALESCE(em.is_absent, false) as is_absent,
                COALESCE(em.question_marks, '[]'::jsonb) as question_marks,
                em.remarks,
                em.entered_at,
                em.updated_at
            FROM lms_task_recipients r
            JOIN users u ON r.student_id = u.id
            LEFT JOIN enrollments e ON e.student_id = u.id AND e.status = 'ACTIVE'
            LEFT JOIN lms_evaluation_marks em ON em.evaluation_id = $1 AND em.student_id = r.student_id
            WHERE r.task_id = $2 AND r.school_id = $3
            ORDER BY COALESCE(NULLIF(e.roll_number, ''), '999999'), student_name ASC
            "#
        )
        .bind(eval.id)
        .bind(task_id)
        .bind(school_id)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| LmsEvaluationStudentMarksRow {
                student_id: r.get("student_id"),
                student_name: r.get("student_name"),
                admission_number: r.try_get("admission_number").ok(),
                roll_number: r.try_get("roll_number").ok(),
                marks_obtained: r.try_get("marks_obtained").ok(),
                is_absent: r.get("is_absent"),
                question_marks: r.get("question_marks"),
                remarks: r.try_get("remarks").ok(),
                entered_at: r.try_get("entered_at").ok(),
                updated_at: r.try_get("updated_at").ok(),
            })
            .collect())
    }

    /// Bulk save evaluation marks
    pub async fn save_evaluation_marks(
        pool: &PgPool,
        school_id: Uuid,
        task_id: Uuid,
        entered_by: Uuid,
        marks_input: &[StudentMarkEntry],
    ) -> Result<(), AppError> {
        let eval = Self::get_evaluation_by_task_id(pool, school_id, task_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("Task is not configured for evaluation".into()))?;

        let mut tx = pool.begin().await?;

        for entry in marks_input {
            let is_absent = entry.is_absent.unwrap_or(false);
            let q_marks = entry
                .question_marks
                .clone()
                .unwrap_or_else(|| serde_json::json!([]));

            sqlx::query(
                r#"
                INSERT INTO lms_evaluation_marks (
                    school_id, evaluation_id, student_id, marks_obtained,
                    is_absent, question_marks, remarks, entered_by, updated_by
                ) VALUES (
                    $1, $2, $3, $4,
                    $5, $6, $7, $8, $8
                )
                ON CONFLICT (evaluation_id, student_id) DO UPDATE SET
                    marks_obtained = EXCLUDED.marks_obtained,
                    is_absent = EXCLUDED.is_absent,
                    question_marks = EXCLUDED.question_marks,
                    remarks = EXCLUDED.remarks,
                    updated_by = EXCLUDED.updated_by,
                    updated_at = NOW()
                "#,
            )
            .bind(school_id)
            .bind(eval.id)
            .bind(entry.student_id)
            .bind(entry.marks_obtained)
            .bind(is_absent)
            .bind(q_marks)
            .bind(&entry.remarks)
            .bind(entered_by)
            .execute(&mut *tx)
            .await?;
        }

        // Update evaluation status
        let _ = sqlx::query(
            "UPDATE lms_evaluations SET status = 'IN_PROGRESS', updated_at = NOW() WHERE id = $1",
        )
        .bind(eval.id)
        .execute(&mut *tx)
        .await;

        tx.commit().await?;

        Self::create_audit_log(
            pool,
            school_id,
            task_id,
            Some(entered_by),
            "MARKS_UPDATED",
            serde_json::json!({
                "evaluation_id": eval.id,
                "records_saved": marks_input.len()
            }),
        )
        .await?;

        Ok(())
    }

    /// Snapshot recipients for a published task
    pub async fn snapshot_recipients(
        pool: &PgPool,
        school_id: Uuid,
        task_id: Uuid,
        class_section_id: Uuid,
    ) -> Result<i64, AppError> {
        let students = sqlx::query(
            r#"
            SELECT 
                u.id as student_id,
                (
                    SELECT e.id 
                    FROM enrollments e 
                    WHERE e.student_id = u.id 
                      AND (e.class_section_id = $2 OR e.class_section_id = u.class_section_id)
                    ORDER BY e.created_at DESC 
                    LIMIT 1
                ) as enrollment_id
            FROM users u
            WHERE u.school_id = $1
              AND u.status = 'ACTIVE'
              AND (
                  u.class_section_id = $2 
                  OR EXISTS (
                      SELECT 1 FROM enrollments e 
                      WHERE e.student_id = u.id AND e.class_section_id = $2 AND (e.status = 'ACTIVE' OR e.status IS NULL)
                  )
              )
            "#,
        )
        .bind(school_id)
        .bind(class_section_id)
        .fetch_all(pool)
        .await?;

        let mut count = 0i64;
        for s in students {
            let stu_id: Uuid = s.get("student_id");
            let enr_id: Option<Uuid> = s.try_get("enrollment_id").ok();

            let res = sqlx::query(
                r#"
                INSERT INTO lms_task_recipients (school_id, task_id, student_id, enrollment_id)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (task_id, student_id) DO NOTHING
                "#,
            )
            .bind(school_id)
            .bind(task_id)
            .bind(stu_id)
            .bind(enr_id)
            .execute(pool)
            .await?;

            if res.rows_affected() > 0 {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Publish a task
    pub async fn publish_task(
        pool: &PgPool,
        school_id: Uuid,
        task_id: Uuid,
        actor_id: Uuid,
    ) -> Result<LmsTask, AppError> {
        let task = Self::get_task_by_id(pool, school_id, task_id).await?;

        if task.attendance_session_id.is_none() {
            return Err(AppError::BadRequest(
                "A published LMS task must reference a valid attendance session.".into(),
            ));
        }

        let session_id = task.attendance_session_id.unwrap();
        let session_valid: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM attendance_sessions WHERE id = $1 AND school_id = $2 AND class_section_id = $3"
        )
        .bind(session_id)
        .bind(school_id)
        .bind(task.class_section_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        if session_valid == 0 {
            return Err(AppError::BadRequest(
                "The selected attendance session does not match this school and class section."
                    .into(),
            ));
        }

        let recipient_count =
            Self::snapshot_recipients(pool, school_id, task_id, task.class_section_id).await?;

        let row = sqlx::query(
            r#"
            UPDATE lms_tasks
            SET status = 'PUBLISHED', published_at = NOW(), updated_at = NOW()
            WHERE id = $1 AND school_id = $2
            RETURNING
                id, school_id, academic_year_id, title, description, special_message,
                task_type, class_section_id, subject_id, teaching_group_id,
                attendance_session_id, created_by, assigned_at, deadline_at,
                status, add_to_calendar, calendar_event_id, notify_whatsapp,
                notify_email, is_evaluation, published_at, created_at, updated_at
            "#,
        )
        .bind(task_id)
        .bind(school_id)
        .fetch_one(pool)
        .await?;

        let updated = Self::row_to_lms_task(&row);

        // Calendar event integration if requested
        if updated.add_to_calendar && updated.deadline_at.is_some() {
            let deadline_date = updated.deadline_at.unwrap().date_naive();
            let cal_res: Option<Uuid> = sqlx::query_scalar(
                r#"
                INSERT INTO academic_calendar (
                    school_id, academic_year_id, date, type, title, description,
                    is_working_day, applies_to_all_classes, class_ids, created_by
                ) VALUES (
                    $1, $2, $3, 'EVENT', $4, $5,
                    true, false, ARRAY[$6]::uuid[], $7
                )
                RETURNING id
                "#,
            )
            .bind(school_id)
            .bind(updated.academic_year_id)
            .bind(deadline_date)
            .bind(format!("Deadline: {}", updated.title))
            .bind(format!(
                "LMS Task: {}\nType: {}",
                updated.title, updated.task_type
            ))
            .bind(updated.class_section_id)
            .bind(actor_id)
            .fetch_optional(pool)
            .await?;

            if let Some(cid) = cal_res {
                let _ = sqlx::query("UPDATE lms_tasks SET calendar_event_id = $1 WHERE id = $2")
                    .bind(cid)
                    .bind(task_id)
                    .execute(pool)
                    .await;
            }
        }

        Self::create_audit_log(
            pool,
            school_id,
            task_id,
            Some(actor_id),
            "PUBLISHED",
            serde_json::json!({
                "recipient_count": recipient_count,
                "attendance_session_id": session_id,
                "deadline_at": updated.deadline_at
            }),
        )
        .await?;

        Ok(updated)
    }

    /// Cancel a task
    pub async fn cancel_task(
        pool: &PgPool,
        school_id: Uuid,
        task_id: Uuid,
        actor_id: Uuid,
    ) -> Result<LmsTask, AppError> {
        let task = Self::get_task_by_id(pool, school_id, task_id).await?;

        if let Some(cal_id) = task.calendar_event_id {
            let _ = sqlx::query("DELETE FROM academic_calendar WHERE id = $1 AND school_id = $2")
                .bind(cal_id)
                .bind(school_id)
                .execute(pool)
                .await;
        }

        let row = sqlx::query(
            r#"
            UPDATE lms_tasks
            SET status = 'CANCELLED', updated_at = NOW()
            WHERE id = $1 AND school_id = $2
            RETURNING
                id, school_id, academic_year_id, title, description, special_message,
                task_type, class_section_id, subject_id, teaching_group_id,
                attendance_session_id, created_by, assigned_at, deadline_at,
                status, add_to_calendar, calendar_event_id, notify_whatsapp,
                notify_email, is_evaluation, published_at, created_at, updated_at
            "#,
        )
        .bind(task_id)
        .bind(school_id)
        .fetch_one(pool)
        .await?;

        let updated = Self::row_to_lms_task(&row);

        Self::create_audit_log(
            pool,
            school_id,
            task_id,
            Some(actor_id),
            "CANCELLED",
            serde_json::json!({ "previous_status": task.status }),
        )
        .await?;

        Ok(updated)
    }

    /// Create an audit log record
    pub async fn create_audit_log(
        pool: &PgPool,
        school_id: Uuid,
        task_id: Uuid,
        actor_id: Option<Uuid>,
        action: &str,
        details: serde_json::Value,
    ) -> Result<(), AppError> {
        let _ = sqlx::query(
            r#"
            INSERT INTO lms_task_audit_logs (school_id, task_id, actor_id, action, details)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(school_id)
        .bind(task_id)
        .bind(actor_id)
        .bind(action)
        .bind(details)
        .execute(pool)
        .await;

        Ok(())
    }

    /// Filter and list LMS tasks
    pub async fn list_tasks(
        pool: &PgPool,
        school_id: Uuid,
        filter: &TaskFilterQuery,
        teacher_scope: Option<Uuid>,
        student_scope: Option<Uuid>,
    ) -> Result<Vec<LmsTaskResponse>, AppError> {
        let mut qb = QueryBuilder::new(
            r#"
            SELECT
                t.id, t.school_id, t.academic_year_id, t.title, t.description, t.special_message,
                t.task_type, t.class_section_id, t.subject_id, t.teaching_group_id,
                t.attendance_session_id, t.created_by, t.assigned_at, t.deadline_at,
                t.status, t.add_to_calendar, t.calendar_event_id, t.notify_whatsapp,
                t.notify_email, t.is_evaluation, t.published_at, t.created_at, t.updated_at,
                c.name as class_name, sec.name as section_name, s.name as subject_name,
                COALESCE(NULLIF(u.display_name, ''), NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''), u.username) as creator_name,
                (SELECT COUNT(*) FROM lms_task_recipients r WHERE r.task_id = t.id) as recipient_count,
                att.date as session_date, att.period_number as session_period, att.attendance_type as session_type,
                COALESCE(NULLIF(u_att.display_name, ''), NULLIF(TRIM(CONCAT(u_att.first_name, ' ', u_att.last_name)), ''), u_att.username) as session_marked_by_name
            FROM lms_tasks t
            JOIN class_sections cs ON t.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections sec ON cs.section_id = sec.id
            LEFT JOIN subjects s ON t.subject_id = s.id
            LEFT JOIN users u ON t.created_by = u.id
            LEFT JOIN attendance_sessions att ON t.attendance_session_id = att.id
            LEFT JOIN users u_att ON att.marked_by = u_att.id
            WHERE t.school_id = 
            "#,
        );
        qb.push_bind(school_id);

        if let Some(y) = filter.academic_year_id {
            qb.push(" AND t.academic_year_id = ");
            qb.push_bind(y);
        }

        if let Some(cs) = filter.class_section_id {
            qb.push(" AND t.class_section_id = ");
            qb.push_bind(cs);
        }

        if let Some(sub) = filter.subject_id {
            qb.push(" AND t.subject_id = ");
            qb.push_bind(sub);
        }

        if let Some(tt) = &filter.task_type {
            let normalized = TaskType::from_str_loose(tt).as_str();
            qb.push(" AND t.task_type = ");
            qb.push_bind(normalized);
        }

        if let Some(st) = &filter.status {
            let normalized = TaskStatus::from_str_loose(st).as_str();
            qb.push(" AND t.status = ");
            qb.push_bind(normalized);
        }

        if let Some(is_eval) = filter.is_evaluation {
            qb.push(" AND t.is_evaluation = ");
            qb.push_bind(is_eval);
        }

        if let Some(creator) = filter.teacher_id.or(teacher_scope) {
            qb.push(" AND t.created_by = ");
            qb.push_bind(creator);
        }

        if let Some(stu_id) = student_scope {
            qb.push(" AND t.status = 'PUBLISHED' AND EXISTS (SELECT 1 FROM lms_task_recipients r WHERE r.task_id = t.id AND r.student_id = ");
            qb.push_bind(stu_id);
            qb.push(")");
        }

        qb.push(" ORDER BY t.created_at DESC");

        let limit = filter.limit.unwrap_or(50).clamp(1, 100);
        let offset = filter.offset.unwrap_or(0).max(0);
        qb.push(" LIMIT ");
        qb.push_bind(limit);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        let rows = qb.build().fetch_all(pool).await?;

        let mut res = Vec::with_capacity(rows.len());
        for r in rows {
            let t_id: Uuid = r.get("id");
            let att_sess_id: Option<Uuid> = r.try_get("attendance_session_id").ok();
            let session_snippet = if let Some(asid) = att_sess_id {
                let s_date: Option<chrono::NaiveDate> = r.try_get("session_date").ok();
                if let Some(d) = s_date {
                    Some(AttendanceSessionSnippet {
                        id: asid,
                        date: d,
                        period_number: r.try_get("session_period").ok(),
                        attendance_type: r
                            .try_get("session_type")
                            .unwrap_or_else(|_| "DAILY".into()),
                        marked_by_name: r.try_get("session_marked_by_name").ok(),
                    })
                } else {
                    None
                }
            } else {
                None
            };

            let attachments = Self::list_attachments(pool, school_id, t_id)
                .await
                .unwrap_or_default();
            let evaluation = Self::get_evaluation_by_task_id(pool, school_id, t_id)
                .await
                .unwrap_or_default();

            res.push(LmsTaskResponse {
                id: t_id,
                school_id: r.get("school_id"),
                academic_year_id: r.get("academic_year_id"),
                title: r.get("title"),
                description: r.get("description"),
                special_message: r.try_get("special_message").ok(),
                task_type: r.get("task_type"),
                class_section_id: r.get("class_section_id"),
                class_name: r.try_get("class_name").ok(),
                section_name: r.try_get("section_name").ok(),
                subject_id: r.try_get("subject_id").ok(),
                subject_name: r.try_get("subject_name").ok(),
                teaching_group_id: r.try_get("teaching_group_id").ok(),
                attendance_session_id: att_sess_id,
                attendance_session_info: session_snippet,
                created_by: r.get("created_by"),
                creator_name: r.try_get("creator_name").ok(),
                assigned_at: r.get("assigned_at"),
                deadline_at: r.try_get("deadline_at").ok(),
                status: r.get("status"),
                add_to_calendar: r.get("add_to_calendar"),
                calendar_event_id: r.try_get("calendar_event_id").ok(),
                notify_whatsapp: r.get("notify_whatsapp"),
                notify_email: r.get("notify_email"),
                is_evaluation: r.get("is_evaluation"),
                published_at: r.try_get("published_at").ok(),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                attachments,
                evaluation,
                recipient_count: r.try_get("recipient_count").unwrap_or(0),
            });
        }

        Ok(res)
    }

    /// Retrieve LMS overview statistics
    pub async fn get_overview_stats(
        pool: &PgPool,
        school_id: Uuid,
        teacher_id: Option<Uuid>,
    ) -> Result<LmsOverviewStats, AppError> {
        let total_tasks: i64 = if let Some(t_id) = teacher_id {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM lms_tasks WHERE school_id = $1 AND created_by = $2",
            )
            .bind(school_id)
            .bind(t_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0)
        } else {
            sqlx::query_scalar("SELECT COUNT(*) FROM lms_tasks WHERE school_id = $1")
                .bind(school_id)
                .fetch_one(pool)
                .await
                .unwrap_or(0)
        };

        let active_tasks: i64 = if let Some(t_id) = teacher_id {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM lms_tasks WHERE school_id = $1 AND created_by = $2 AND status = 'PUBLISHED' AND (deadline_at IS NULL OR deadline_at >= NOW())"
            )
            .bind(school_id)
            .bind(t_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0)
        } else {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM lms_tasks WHERE school_id = $1 AND status = 'PUBLISHED' AND (deadline_at IS NULL OR deadline_at >= NOW())"
            )
            .bind(school_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0)
        };

        let upcoming_deadlines: i64 = if let Some(t_id) = teacher_id {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM lms_tasks WHERE school_id = $1 AND created_by = $2 AND status = 'PUBLISHED' AND deadline_at >= NOW() AND deadline_at <= (NOW() + INTERVAL '48 hours')"
            )
            .bind(school_id)
            .bind(t_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0)
        } else {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM lms_tasks WHERE school_id = $1 AND status = 'PUBLISHED' AND deadline_at >= NOW() AND deadline_at <= (NOW() + INTERVAL '48 hours')"
            )
            .bind(school_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0)
        };

        let completed_tasks: i64 = if let Some(t_id) = teacher_id {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM lms_tasks WHERE school_id = $1 AND created_by = $2 AND (status = 'CLOSED' OR (status = 'PUBLISHED' AND deadline_at < NOW()))"
            )
            .bind(school_id)
            .bind(t_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0)
        } else {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM lms_tasks WHERE school_id = $1 AND (status = 'CLOSED' OR (status = 'PUBLISHED' AND deadline_at < NOW()))"
            )
            .bind(school_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0)
        };

        let pending_evaluation: i64 = if let Some(t_id) = teacher_id {
            sqlx::query_scalar(
                r#"
                SELECT COUNT(*) FROM lms_tasks t
                JOIN lms_evaluations e ON t.id = e.task_id
                WHERE t.school_id = $1 AND t.created_by = $2 AND t.status = 'PUBLISHED'
                  AND e.status != 'COMPLETED'
                "#,
            )
            .bind(school_id)
            .bind(t_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0)
        } else {
            sqlx::query_scalar(
                r#"
                SELECT COUNT(*) FROM lms_tasks t
                JOIN lms_evaluations e ON t.id = e.task_id
                WHERE t.school_id = $1 AND t.status = 'PUBLISHED'
                  AND e.status != 'COMPLETED'
                "#,
            )
            .bind(school_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0)
        };

        let recent = Self::list_tasks(
            pool,
            school_id,
            &TaskFilterQuery {
                academic_year_id: None,
                class_section_id: None,
                subject_id: None,
                teacher_id: None,
                task_type: None,
                status: None,
                is_evaluation: None,
                start_date: None,
                end_date: None,
                limit: Some(6),
                offset: Some(0),
            },
            teacher_id,
            None,
        )
        .await
        .unwrap_or_default();

        let activity_rows = sqlx::query(
            r#"
            SELECT
                cs.id as class_section_id,
                c.name as class_name,
                sec.name as section_name,
                COUNT(t.id) as task_count,
                COUNT(CASE WHEN t.status = 'PUBLISHED' AND (t.deadline_at IS NULL OR t.deadline_at >= NOW()) THEN 1 END) as active_count
            FROM class_sections cs
            JOIN classes c ON cs.class_id = c.id
            JOIN sections sec ON cs.section_id = sec.id
            LEFT JOIN lms_tasks t ON cs.id = t.class_section_id
            WHERE c.school_id = $1
            GROUP BY cs.id, c.name, sec.name
            ORDER BY c.name, sec.name ASC
            "#
        )
        .bind(school_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let class_activity = activity_rows
            .into_iter()
            .map(|r| ClassActivitySummary {
                class_section_id: r.get("class_section_id"),
                class_name: r.get("class_name"),
                section_name: r.get("section_name"),
                task_count: r.get("task_count"),
                active_count: r.get("active_count"),
            })
            .collect();

        Ok(LmsOverviewStats {
            total_tasks,
            active_tasks,
            upcoming_deadlines,
            completed_tasks,
            pending_evaluation,
            recent_tasks: recent,
            class_activity,
        })
    }

    /// Retrieve daily teacher task report
    pub async fn get_daily_teacher_report(
        pool: &PgPool,
        school_id: Uuid,
        date: chrono::NaiveDate,
        teacher_id: Option<Uuid>,
    ) -> Result<Vec<DailyTeacherTaskReportItem>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                att.date,
                att.marked_by as teacher_id,
                COALESCE(NULLIF(u.display_name, ''), NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''), u.username) as teacher_name,
                att.class_section_id,
                c.name as class_name,
                sec.name as section_name,
                att.subject_id,
                s.name as subject_name,
                att.id as attendance_session_id,
                att.period_number,
                t.id as task_id,
                t.title as task_title,
                t.task_type,
                t.deadline_at,
                t.is_evaluation,
                eval.id as evaluation_id,
                eval.status as evaluation_status,
                (SELECT COUNT(*) FROM lms_evaluation_marks em WHERE em.evaluation_id = eval.id) as marks_count
            FROM attendance_sessions att
            JOIN users u ON att.marked_by = u.id
            JOIN class_sections cs ON att.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections sec ON cs.section_id = sec.id
            LEFT JOIN subjects s ON att.subject_id = s.id
            LEFT JOIN lms_tasks t ON t.attendance_session_id = att.id AND t.status != 'CANCELLED'
            LEFT JOIN lms_evaluations eval ON t.id = eval.task_id
            WHERE att.school_id = $1 AND att.date = $2
              AND ($3::uuid IS NULL OR att.marked_by = $3)
            ORDER BY teacher_name, att.period_number ASC
            "#
        )
        .bind(school_id)
        .bind(date)
        .bind(teacher_id)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| {
                let task_id: Option<Uuid> = r.try_get("task_id").ok();
                let is_eval: Option<bool> = r.try_get("is_evaluation").ok();
                let marks_cnt: i64 = r.try_get("marks_count").unwrap_or(0);
                let eval_status: Option<String> = r.try_get("evaluation_status").ok();

                DailyTeacherTaskReportItem {
                    date: r.get("date"),
                    teacher_id: r.get("teacher_id"),
                    teacher_name: r.get("teacher_name"),
                    class_section_id: r.get("class_section_id"),
                    class_name: r.get("class_name"),
                    section_name: r.get("section_name"),
                    subject_id: r.try_get("subject_id").ok(),
                    subject_name: r.try_get("subject_name").ok(),
                    attendance_session_id: r.try_get("attendance_session_id").ok(),
                    period_number: r.try_get("period_number").ok(),
                    task_assigned: task_id.is_some(),
                    task_id,
                    task_title: r.try_get("task_title").ok(),
                    task_type: r.try_get("task_type").ok(),
                    deadline_at: r.try_get("deadline_at").ok(),
                    evaluation_required: is_eval.unwrap_or(false),
                    marks_entered: marks_cnt > 0,
                    marks_completed: eval_status.as_deref() == Some("COMPLETED"),
                }
            })
            .collect())
    }

    /// Find the teacher's most recent relevant attendance session for defaulting
    pub async fn get_default_attendance_session(
        pool: &PgPool,
        school_id: Uuid,
        teacher_id: Uuid,
        class_section_id: Uuid,
        subject_id: Option<Uuid>,
    ) -> Result<Option<AttendanceSessionSnippet>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT
                att.id, att.date, att.period_number, att.attendance_type,
                COALESCE(NULLIF(u.display_name, ''), NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''), u.username) as marked_by_name
            FROM attendance_sessions att
            JOIN users u ON att.marked_by = u.id
            WHERE att.school_id = $1
              AND (att.marked_by = $2 OR $2 = ANY(att.co_marked_by))
              AND att.class_section_id = $3
              AND ($4::uuid IS NULL OR att.subject_id = $4 OR att.attendance_type = 'DAILY')
            ORDER BY att.date DESC, att.period_number DESC NULLS LAST
            LIMIT 1
            "#
        )
        .bind(school_id)
        .bind(teacher_id)
        .bind(class_section_id)
        .bind(subject_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| AttendanceSessionSnippet {
            id: r.get("id"),
            date: r.get("date"),
            period_number: r.try_get("period_number").ok(),
            attendance_type: r.get("attendance_type"),
            marked_by_name: r.try_get("marked_by_name").ok(),
        }))
    }

    /// Retrieve distinct classes where the user has taken attendance sessions, with the latest subject
    pub async fn get_attended_classes(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        is_admin: bool,
    ) -> Result<Vec<AttendedClassSnippet>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                DISTINCT ON (cs.id)
                cs.id as class_section_id,
                c.name as class_name,
                sec.name as section_name,
                COALESCE(s.subject_id, cdt.subject_id, class_sub.subject_id) as subject_id,
                COALESCE(subj.name, tt_sub.name, class_sub_name.name) as subject_name,
                s.date as last_session_date
            FROM attendance_sessions s
            JOIN class_sections cs ON s.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections sec ON cs.section_id = sec.id
            LEFT JOIN subjects subj ON s.subject_id = subj.id
            LEFT JOIN current_day_timetable cdt ON cdt.class_section_id = s.class_section_id 
                                                AND cdt.date = s.date 
                                                AND cdt.period_number = s.period_number
            LEFT JOIN subjects tt_sub ON cdt.subject_id = tt_sub.id
            LEFT JOIN LATERAL (
                SELECT sub_inner.id as subject_id
                FROM current_day_timetable cdt_inner
                JOIN subjects sub_inner ON cdt_inner.subject_id = sub_inner.id
                WHERE cdt_inner.class_section_id = cs.id
                LIMIT 1
            ) class_sub ON true
            LEFT JOIN subjects class_sub_name ON class_sub.subject_id = class_sub_name.id
            WHERE s.school_id = $1
              AND ($2 = true OR s.marked_by = $3 OR $3 = ANY(s.co_marked_by))
            ORDER BY cs.id, s.date DESC, s.period_number DESC NULLS LAST
            "#,
        )
        .bind(school_id)
        .bind(is_admin)
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| AttendedClassSnippet {
                class_section_id: r.get("class_section_id"),
                class_name: r.get("class_name"),
                section_name: r.get("section_name"),
                default_subject_id: r.try_get("subject_id").ok().flatten(),
                default_subject_name: r.try_get("subject_name").ok().flatten(),
                last_session_date: r
                    .get::<chrono::NaiveDate, _>("last_session_date")
                    .to_string(),
            })
            .collect())
    }
}

#[derive(Debug, serde::Serialize)]
pub struct AttendedClassSnippet {
    pub class_section_id: Uuid,
    pub class_name: String,
    pub section_name: String,
    pub default_subject_id: Option<Uuid>,
    pub default_subject_name: Option<String>,
    pub last_session_date: String,
}
