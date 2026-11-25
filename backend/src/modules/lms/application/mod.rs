use crate::error::AppError;
use crate::modules::auth::backend::User;
use crate::modules::lms::domain::LmsTask;
use crate::modules::lms::dto::*;
use crate::modules::lms::repository::LmsRepository;
use sqlx::{PgPool, Row};
use tracing::info;
use uuid::Uuid;

pub struct LmsService;

impl LmsService {
    /// Create a task draft or publish directly
    pub async fn create_task(
        pool: &PgPool,
        user: &User,
        req: CreateLmsTaskRequest,
    ) -> Result<LmsTaskResponse, AppError> {
        let school_id = user.school_id;

        // Create main task record
        let task = LmsRepository::create_task(pool, school_id, user.id, &req).await?;

        // Attachments if provided
        if let Some(att_list) = &req.attachments {
            for att in att_list {
                LmsRepository::add_attachment(pool, school_id, task.id, user.id, att).await?;
            }
        }

        // Evaluation config if enabled
        if req.is_evaluation.unwrap_or(false) {
            if let Some(eval_cfg) = &req.evaluation_config {
                LmsRepository::configure_evaluation(pool, school_id, task.id, user.id, eval_cfg)
                    .await?;
            }
        }

        // Publish now if requested or if status == "PUBLISHED" and attendance session is provided
        let should_publish = req.publish_now.unwrap_or(false)
            || req
                .status
                .as_deref()
                .map(|s| s.eq_ignore_ascii_case("PUBLISHED"))
                .unwrap_or(false);

        let final_task = if should_publish && task.attendance_session_id.is_some() {
            let pub_task = LmsRepository::publish_task(pool, school_id, task.id, user.id).await?;

            // Trigger notification delivery asynchronously if enabled
            if pub_task.notify_whatsapp || pub_task.notify_email {
                let pool_clone = pool.clone();
                let task_clone = pub_task.clone();
                tokio::spawn(async move {
                    let _ = Self::dispatch_task_notifications(&pool_clone, &task_clone).await;
                });
            }

            pub_task
        } else {
            task
        };

        Self::get_task_details(pool, user, final_task.id).await
    }

    /// Update a task
    pub async fn update_task(
        pool: &PgPool,
        user: &User,
        task_id: Uuid,
        req: UpdateLmsTaskRequest,
    ) -> Result<LmsTaskResponse, AppError> {
        let school_id = user.school_id;
        let existing = LmsRepository::get_task_by_id(pool, school_id, task_id).await?;

        // Authorization check: only creator or users with lms:edit_all can edit
        let can_edit_all = user.capabilities.contains(&"lms:edit_all".to_string())
            || user
                .roles
                .iter()
                .any(|r| r == "SUPER_ADMIN" || r == "SCHOOL_ADMIN" || r == "PRINCIPAL");

        if existing.created_by != user.id && !can_edit_all {
            return Err(AppError::Forbidden(
                "You do not have permission to edit this task".into(),
            ));
        }

        let task = LmsRepository::update_task(pool, school_id, task_id, user.id, &req).await?;

        // Update evaluation config if provided
        if req.is_evaluation.unwrap_or(false) {
            if let Some(eval_cfg) = &req.evaluation_config {
                LmsRepository::configure_evaluation(pool, school_id, task.id, user.id, eval_cfg)
                    .await?;
            }
        }

        Self::get_task_details(pool, user, task.id).await
    }

    /// Publish a draft task
    pub async fn publish_task(
        pool: &PgPool,
        user: &User,
        task_id: Uuid,
    ) -> Result<LmsTaskResponse, AppError> {
        let school_id = user.school_id;
        let task = LmsRepository::publish_task(pool, school_id, task_id, user.id).await?;

        // Dispatch notifications if configured
        if task.notify_whatsapp || task.notify_email {
            let pool_clone = pool.clone();
            let task_clone = task.clone();
            tokio::spawn(async move {
                let _ = Self::dispatch_task_notifications(&pool_clone, &task_clone).await;
            });
        }

        Self::get_task_details(pool, user, task.id).await
    }

    /// Cancel a task
    pub async fn cancel_task(
        pool: &PgPool,
        user: &User,
        task_id: Uuid,
    ) -> Result<LmsTaskResponse, AppError> {
        let school_id = user.school_id;
        let existing = LmsRepository::get_task_by_id(pool, school_id, task_id).await?;

        let can_cancel = existing.created_by == user.id
            || user.capabilities.contains(&"lms:cancel".to_string())
            || user
                .roles
                .iter()
                .any(|r| r == "SUPER_ADMIN" || r == "SCHOOL_ADMIN" || r == "PRINCIPAL");

        if !can_cancel {
            return Err(AppError::Forbidden(
                "Permission denied to cancel this task".into(),
            ));
        }

        let task = LmsRepository::cancel_task(pool, school_id, task_id, user.id).await?;
        Self::get_task_details(pool, user, task.id).await
    }

    /// Delete a task permanently (supports DRAFT, PUBLISHED, or CANCELLED)
    pub async fn delete_task(pool: &PgPool, user: &User, task_id: Uuid) -> Result<(), AppError> {
        let school_id = user.school_id;
        let existing = LmsRepository::get_task_by_id(pool, school_id, task_id).await?;

        let can_delete = existing.created_by == user.id
            || user.capabilities.contains(&"lms:delete".to_string())
            || user.capabilities.contains(&"lms:cancel".to_string())
            || user
                .roles
                .iter()
                .any(|r| r == "SUPER_ADMIN" || r == "SCHOOL_ADMIN" || r == "PRINCIPAL");

        if !can_delete {
            return Err(AppError::Forbidden(
                "Permission denied to delete this task".into(),
            ));
        }

        LmsRepository::delete_task(pool, school_id, task_id, user.id).await
    }

    /// Retrieve task details by ID
    pub async fn get_task_details(
        pool: &PgPool,
        user: &User,
        task_id: Uuid,
    ) -> Result<LmsTaskResponse, AppError> {
        let school_id = user.school_id;
        let filter = TaskFilterQuery {
            academic_year_id: None,
            class_section_id: None,
            subject_id: None,
            teacher_id: None,
            task_type: None,
            status: None,
            is_evaluation: None,
            start_date: None,
            end_date: None,
            limit: Some(1),
            offset: Some(0),
        };

        let is_student = user
            .roles
            .iter()
            .any(|r| r == "STUDENT" || r == "PARENT" || r == "GUARDIAN");
        let student_scope = if is_student { Some(user.id) } else { None };

        let list = LmsRepository::list_tasks(pool, school_id, &filter, None, student_scope).await?;
        let item = list
            .into_iter()
            .find(|t| t.id == task_id)
            .ok_or_else(|| AppError::NotFound("LMS Task not found or not accessible".into()))?;

        Ok(item)
    }

    /// List tasks with role-based scoping
    pub async fn list_tasks(
        pool: &PgPool,
        user: &User,
        filter: TaskFilterQuery,
    ) -> Result<Vec<LmsTaskResponse>, AppError> {
        let school_id = user.school_id;
        let is_admin = user
            .roles
            .iter()
            .any(|r| r == "SUPER_ADMIN" || r == "SCHOOL_ADMIN" || r == "PRINCIPAL");
        let is_student = user
            .roles
            .iter()
            .any(|r| r == "STUDENT" || r == "PARENT" || r == "GUARDIAN");

        let teacher_scope = if !is_admin && !is_student {
            // For teachers, optionally allow them to view only their tasks unless specific permission
            if user.capabilities.contains(&"lms:edit_all".to_string()) {
                None
            } else {
                Some(user.id)
            }
        } else {
            None
        };

        let student_scope = if is_student { Some(user.id) } else { None };

        LmsRepository::list_tasks(pool, school_id, &filter, teacher_scope, student_scope).await
    }

    /// Retrieve overview stats
    pub async fn get_overview_stats(
        pool: &PgPool,
        user: &User,
    ) -> Result<LmsOverviewStats, AppError> {
        let is_admin = user
            .roles
            .iter()
            .any(|r| r == "SUPER_ADMIN" || r == "SCHOOL_ADMIN" || r == "PRINCIPAL");
        let teacher_id = if is_admin { None } else { Some(user.id) };

        LmsRepository::get_overview_stats(pool, user.school_id, teacher_id).await
    }

    /// Dispatch notifications for published tasks
    async fn dispatch_task_notifications(pool: &PgPool, task: &LmsTask) -> Result<(), AppError> {
        info!(task_id = %task.id, "Dispatching LMS task notifications");

        // Fetch student contact numbers and parent emails
        let recipients = sqlx::query(
            r#"
            SELECT
                u.id as student_id,
                u.phone as student_phone,
                u.email as student_email,
                g.phone as guardian_phone,
                g.email as guardian_email
            FROM lms_task_recipients r
            JOIN users u ON r.student_id = u.id
            LEFT JOIN student_guardians sg ON sg.student_id = u.id
            LEFT JOIN users g ON sg.guardian_id = g.id
            WHERE r.task_id = $1
            "#,
        )
        .bind(task.id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let deadline_str = task
            .deadline_at
            .map(|d| d.format("%d %b %Y, %I:%M %p").to_string())
            .unwrap_or_else(|| "No deadline specified".into());

        let wa_text = format!(
            "📚 *New Academic Task Assigned*\n\n\
            *Title*: {}\n\
            *Type*: {}\n\
            *Deadline*: {}\n\n\
            *Instructions*:\n{}\n\n\
            {}",
            task.title,
            task.task_type,
            deadline_str,
            task.description,
            task.special_message
                .as_deref()
                .map(|m| format!("⚠️ *Special Note*: {}\n\n", m))
                .unwrap_or_default()
        );

        for rec in recipients {
            let guardian_phone: Option<String> = rec.try_get("guardian_phone").ok();
            let student_phone: Option<String> = rec.try_get("student_phone").ok();
            let guardian_email: Option<String> = rec.try_get("guardian_email").ok();
            let student_email: Option<String> = rec.try_get("student_email").ok();

            // WhatsApp delivery
            if task.notify_whatsapp {
                let target_phone = guardian_phone.as_deref().or(student_phone.as_deref());
                if let Some(phone) = target_phone {
                    let _ =
                        crate::utils::whatsapp::send_whatsapp_message(pool, phone, &wa_text, None)
                            .await;
                }
            }

            // Email delivery
            if task.notify_email {
                let target_email = guardian_email.as_deref().or(student_email.as_deref());
                if let Some(email) = target_email {
                    let email_html = format!(
                        "<h2>New Academic Task: {}</h2>\
                        <p><strong>Type:</strong> {}</p>\
                        <p><strong>Deadline:</strong> {}</p>\
                        <div style='margin:16px 0;padding:12px;background:#f9fafb;border-left:4px solid #4f46e5;'>\
                        <p>{}</p>\
                        </div>",
                        task.title, task.task_type, deadline_str, task.description
                    );

                    let _ = crate::utils::email::send_email(
                        email,
                        &format!("New Task: {}", task.title),
                        &email_html,
                    )
                    .await;
                }
            }
        }

        Ok(())
    }
}
