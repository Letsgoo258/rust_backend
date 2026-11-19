use crate::{
    error::AppError,
    modules::auth::backend::{AuthSession, User},
    state::AppState,
};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{delete, get, patch, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct PrintQueueItem {
    pub id: Uuid,
    pub school_id: Uuid,
    pub user_id: Option<Uuid>,
    pub uploaded_by: Uuid,
    pub document_type: String,
    pub title: String,
    pub recipient_name: String,
    pub recipient_phone: Option<String>,
    pub recipient_admission_number: Option<String>,
    pub file_url: Option<String>,
    pub file_name: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub copies: Option<i32>,
    pub status: String,
    pub options: serde_json::Value,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub printed_at: Option<DateTime<Utc>>,
    pub printed_by: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnqueuePrintJobPayload {
    pub document_type: String,
    pub title: String,
    pub recipient_name: String,
    pub recipient_phone: Option<String>,
    pub recipient_admission_number: Option<String>,
    pub user_id: Option<Uuid>,
    pub file_url: Option<String>,
    pub file_name: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub copies: Option<i32>,
    pub options: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePrintJobStatusPayload {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchPrintQueueQuery {
    pub q: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DbStudentWithDocs {
    pub id: Uuid,
    pub display_name: String,
    pub phone: Option<String>,
    pub admission_number: Option<String>,
    pub class_name: Option<String>,
    pub section_name: Option<String>,
    pub roll_number: Option<String>,
    pub issued_documents: Vec<serde_json::Value>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/access", get(check_access_handler))
        .route("/", post(enqueue_print_job))
        .route("/my-orders", get(get_my_orders))
        .route("/search", get(search_print_desk))
        .route("/:id/status", patch(update_job_status))
        .route("/:id", delete(cancel_print_job))
        .layer(axum::extract::DefaultBodyLimit::max(50 * 1024 * 1024))
}

/// Verify if the user has Print Center Desk privileges
async fn has_print_desk_access(pool: &PgPool, user: &User) -> Result<(bool, String), AppError> {
    if user
        .roles
        .iter()
        .any(|r| r.eq_ignore_ascii_case("SUPER_ADMIN"))
    {
        return Ok((true, "SUPER_ADMIN".into()));
    }

    if user.is_school_admin() {
        return Ok((true, "ADMIN".into()));
    }

    // Check modules_staff_assignments in school_settings
    let assignments_val = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = 'modules_staff_assignments'
        "#,
    )
    .bind(user.school_id)
    .fetch_optional(pool)
    .await?;

    if let Some(val) = assignments_val {
        if let Some(arr) = val.as_array() {
            for item in arr {
                let u_id = item.get("user_id").and_then(|v| v.as_str());
                let m_key = item.get("module_key").and_then(|v| v.as_str());
                let status = item
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("ACTIVE");
                let tier = item
                    .get("access_tier")
                    .and_then(|v| v.as_str())
                    .unwrap_or("OPERATOR");

                if let Some(uid_str) = u_id {
                    if uid_str == user.id.to_string()
                        && m_key == Some("print_studio")
                        && status == "ACTIVE"
                    {
                        return Ok((true, tier.to_string()));
                    }
                }
            }
        }
    }

    Ok((false, "NONE".into()))
}

/// GET /api/v1/print-queue/access
pub async fn check_access_handler(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?;

    let (has_access, tier) = has_print_desk_access(&state.db, &user).await?;

    // Also get active pending count for badge display
    let pending_count: i64 = if has_access {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*) 
            FROM print_queue 
            WHERE school_id = $1 AND status = 'PENDING' AND expires_at > now()
            "#,
        )
        .bind(user.school_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0)
    } else {
        0
    };

    Ok(Json(json!({
        "has_access": has_access,
        "tier": tier,
        "pending_count": pending_count
    })))
}

/// POST /api/v1/print-queue
pub async fn enqueue_print_job(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<EnqueuePrintJobPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?;

    let school_id = user.school_id;
    let copies = payload.copies.unwrap_or(1).clamp(1, 50);

    let norm_phone = payload
        .recipient_phone
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(crate::utils::phone::normalize_phone_number);

    let recipient_adm = payload
        .recipient_admission_number
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let options = payload.options.unwrap_or_else(|| {
        json!({
            "color": "BW",
            "paper_size": "A4",
            "orientation": "PORTRAIT",
            "double_sided": false
        })
    });

    let metadata = payload.metadata.unwrap_or_else(|| json!({}));

    let job = sqlx::query_as::<_, PrintQueueItem>(
        r#"
        INSERT INTO print_queue (
            school_id,
            user_id,
            uploaded_by,
            document_type,
            title,
            recipient_name,
            recipient_phone,
            recipient_admission_number,
            file_url,
            file_name,
            file_size_bytes,
            copies,
            status,
            options,
            metadata,
            created_at,
            expires_at
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 'PENDING',
            $13, $14, now(), now() + interval '24 hours'
        )
        RETURNING
            id, school_id, user_id, uploaded_by, document_type, title,
            recipient_name, recipient_phone, recipient_admission_number,
            file_url, file_name, file_size_bytes, copies, status,
            options, metadata, created_at, expires_at, printed_at, printed_by
        "#,
    )
    .bind(school_id)
    .bind(payload.user_id)
    .bind(user.id)
    .bind(&payload.document_type)
    .bind(&payload.title)
    .bind(&payload.recipient_name)
    .bind(norm_phone)
    .bind(recipient_adm)
    .bind(&payload.file_url)
    .bind(&payload.file_name)
    .bind(payload.file_size_bytes.unwrap_or(0))
    .bind(copies)
    .bind(options)
    .bind(metadata)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to enqueue print job into print_queue: {:?}", e);
        AppError::Database(e)
    })?;

    Ok(Json(json!({
        "success": true,
        "message": "Document successfully added to print queue (24h retention)",
        "job": job
    })))
}

/// GET /api/v1/print-queue/my-orders
pub async fn get_my_orders(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?;

    let items = sqlx::query_as::<_, PrintQueueItem>(
        r#"
        SELECT
            id, school_id, user_id, uploaded_by, document_type, title,
            recipient_name, recipient_phone, recipient_admission_number,
            file_url, file_name, file_size_bytes, copies, status,
            options, metadata, created_at, expires_at, printed_at, printed_by
        FROM print_queue
        WHERE uploaded_by = $1 AND (expires_at > now() OR status = 'PRINTED')
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Ok(Json(json!({
        "items": items,
        "count": items.len()
    })))
}

/// GET /api/v1/print-queue/search?q=...
/// Unified print desk search: active print queue orders + student documents from DB
pub async fn search_print_desk(
    auth_session: AuthSession,
    Query(query): Query<SearchPrintQueueQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?;

    let (has_access, tier) = has_print_desk_access(&state.db, &user).await?;
    if !has_access {
        return Err(AppError::Forbidden(
            "Print Center Desk access is restricted to assigned module staff and school administrators.".into(),
        ));
    }

    let raw_q = query.q.unwrap_or_default();
    let q = raw_q.trim();

    // 1. Fetch queued orders
    let queued_orders: Vec<PrintQueueItem> = if q.is_empty() {
        // Return latest 25 pending orders
        sqlx::query_as::<_, PrintQueueItem>(
            r#"
            SELECT
                id, school_id, user_id, uploaded_by, document_type, title,
                recipient_name, recipient_phone, recipient_admission_number,
                file_url, file_name, file_size_bytes, copies, status,
                options, metadata, created_at, expires_at, printed_at, printed_by
            FROM print_queue
            WHERE school_id = $1 AND (expires_at > now() OR status = 'PRINTED')
            ORDER BY created_at DESC
            LIMIT 25
            "#,
        )
        .bind(user.school_id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    } else {
        let pattern = format!("%{}%", q);
        sqlx::query_as::<_, PrintQueueItem>(
            r#"
            SELECT
                id, school_id, user_id, uploaded_by, document_type, title,
                recipient_name, recipient_phone, recipient_admission_number,
                file_url, file_name, file_size_bytes, copies, status,
                options, metadata, created_at, expires_at, printed_at, printed_by
            FROM print_queue
            WHERE school_id = $1 AND (expires_at > now() OR status = 'PRINTED')
              AND (
                recipient_phone ILIKE $2
                OR recipient_name ILIKE $2
                OR recipient_admission_number ILIKE $2
                OR title ILIKE $2
              )
            ORDER BY created_at DESC
            LIMIT 25
            "#,
        )
        .bind(user.school_id)
        .bind(&pattern)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    };

    // 2. If query given, search student documents from DB (users + issued_reports)
    let mut db_students: Vec<DbStudentWithDocs> = Vec::new();

    if !q.is_empty() {
        let pattern = format!("%{}%", q);
        let user_rows = sqlx::query_as::<
            _,
            (
                Uuid,
                Option<String>,
                Option<String>,
                Option<String>,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            ),
        >(
            r#"
            SELECT 
                u.id, 
                u.display_name, 
                u.first_name, 
                u.last_name, 
                u.username, 
                u.phone, 
                u.admission_number,
                c.name as class_name, 
                sec.name as section_name,
                e.roll_number
            FROM users u
            LEFT JOIN enrollments e ON e.student_id = u.id AND e.status = 'ACTIVE'
            LEFT JOIN class_sections cs ON cs.id = COALESCE(e.class_section_id, u.class_section_id)
            LEFT JOIN classes c ON cs.class_id = c.id
            LEFT JOIN sections sec ON cs.section_id = sec.id
            WHERE u.school_id = $1
              AND (
                u.phone ILIKE $2
                OR u.display_name ILIKE $2
                OR u.first_name ILIKE $2
                OR u.last_name ILIKE $2
                OR u.username ILIKE $2
                OR u.admission_number ILIKE $2
              )
            ORDER BY u.created_at DESC
            LIMIT 15
            "#,
        )
        .bind(user.school_id)
        .bind(&pattern)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        let student_ids: Vec<Uuid> = user_rows.iter().map(|r| r.0).collect();

        // Fetch issued_reports for these students
        let mut reports_map: std::collections::HashMap<Uuid, Vec<serde_json::Value>> =
            std::collections::HashMap::new();

        if !student_ids.is_empty() {
            let report_rows = sqlx::query_as::<
                _,
                (
                    Uuid,
                    Uuid,
                    String,
                    String,
                    i32,
                    String,
                    Option<String>,
                    chrono::NaiveDate,
                    String,
                    serde_json::Value,
                ),
            >(
                r#"
                SELECT 
                    id, student_id, report_type, document_number, serial_number, 
                    student_name, admission_number, issue_date, status, variables_snapshot
                FROM issued_reports
                WHERE school_id = $1 AND student_id = ANY($2)
                ORDER BY created_at DESC
                "#,
            )
            .bind(user.school_id)
            .bind(&student_ids)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

            for (
                id,
                student_id,
                report_type,
                document_number,
                serial_number,
                student_name,
                admission_number,
                issue_date,
                status,
                variables_snapshot,
            ) in report_rows
            {
                reports_map.entry(student_id).or_default().push(json!({
                    "id": id,
                    "report_type": report_type,
                    "document_number": document_number,
                    "serial_number": serial_number,
                    "student_name": student_name,
                    "admission_number": admission_number,
                    "issue_date": issue_date.to_string(),
                    "status": status,
                    "variables": variables_snapshot
                }));
            }
        }

        for (
            id,
            display_name,
            first_name,
            last_name,
            username,
            phone,
            admission_number,
            class_name,
            section_name,
            roll_number,
        ) in user_rows
        {
            let name = display_name
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| {
                    let f = first_name.as_deref().unwrap_or("").trim();
                    let l = last_name.as_deref().unwrap_or("").trim();
                    let full = format!("{} {}", f, l).trim().to_string();
                    if full.is_empty() {
                        username
                    } else {
                        full
                    }
                });

            let issued_docs = reports_map.remove(&id).unwrap_or_default();

            db_students.push(DbStudentWithDocs {
                id,
                display_name: name,
                phone,
                admission_number,
                class_name,
                section_name,
                roll_number,
                issued_documents: issued_docs,
            });
        }
    }

    Ok(Json(json!({
        "queued_orders": queued_orders,
        "db_students": db_students,
        "access_tier": tier,
        "query": q
    })))
}

/// PATCH /api/v1/print-queue/:id/status
pub async fn update_job_status(
    auth_session: AuthSession,
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    Json(payload): Json<UpdatePrintJobStatusPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?;

    let (has_access, _) = has_print_desk_access(&state.db, &user).await?;

    // Check if job exists
    let existing = sqlx::query_as::<_, PrintQueueItem>(
        r#"
        SELECT
            id, school_id, user_id, uploaded_by, document_type, title,
            recipient_name, recipient_phone, recipient_admission_number,
            file_url, file_name, file_size_bytes, copies, status,
            options, metadata, created_at, expires_at, printed_at, printed_by
        FROM print_queue
        WHERE id = $1 AND school_id = $2
        "#,
    )
    .bind(id)
    .bind(user.school_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Print job not found".into()))?;

    // If cancelling, allowed if user uploaded it OR has print desk access
    if payload.status == "CANCELLED" {
        if existing.uploaded_by != user.id && !has_access {
            return Err(AppError::Forbidden(
                "You cannot cancel another user's print job.".into(),
            ));
        }
    } else if !has_access {
        return Err(AppError::Forbidden(
            "Only assigned print desk staff or administrators can update job status.".into(),
        ));
    }

    let is_printed = payload.status == "PRINTED" || payload.status == "COMPLETED";

    let updated = sqlx::query_as::<_, PrintQueueItem>(
        r#"
        UPDATE print_queue
        SET status = $1,
            printed_at = CASE WHEN $2 THEN now() ELSE printed_at END,
            printed_by = CASE WHEN $2 THEN $3 ELSE printed_by END
        WHERE id = $4 AND school_id = $5
        RETURNING
            id, school_id, user_id, uploaded_by, document_type, title,
            recipient_name, recipient_phone, recipient_admission_number,
            file_url, file_name, file_size_bytes, copies, status,
            options, metadata, created_at, expires_at, printed_at, printed_by
        "#,
    )
    .bind(&payload.status)
    .bind(is_printed)
    .bind(user.id)
    .bind(id)
    .bind(user.school_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(json!({
        "success": true,
        "job": updated
    })))
}

/// DELETE /api/v1/print-queue/:id
pub async fn cancel_print_job(
    auth_session: AuthSession,
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?;

    let (has_access, _) = has_print_desk_access(&state.db, &user).await?;

    let existing = sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT id, uploaded_by FROM print_queue WHERE id = $1 AND school_id = $2",
    )
    .bind(id)
    .bind(user.school_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Print job not found".into()))?;

    if existing.1 != user.id && !has_access {
        return Err(AppError::Forbidden(
            "You cannot delete another user's print job.".into(),
        ));
    }

    sqlx::query("DELETE FROM print_queue WHERE id = $1 AND school_id = $2")
        .bind(id)
        .bind(user.school_id)
        .execute(&state.db)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Print job cancelled and removed"
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_minimal_enqueue_payload() {
        let json_str = r#"{
            "document_type": "DATA_EXPORT",
            "title": "Students Export.csv",
            "recipient_name": "Admin"
        }"#;

        let payload: Result<EnqueuePrintJobPayload, _> = serde_json::from_str(json_str);
        assert!(payload.is_ok());
        let p = payload.unwrap();
        assert_eq!(p.document_type, "DATA_EXPORT");
        assert_eq!(p.title, "Students Export.csv");
        assert_eq!(p.recipient_name, "Admin");
        assert!(p.recipient_phone.is_none());
        assert!(p.recipient_admission_number.is_none());
        assert!(p.file_url.is_none());
        assert!(p.file_size_bytes.is_none());
        assert!(p.copies.is_none());
        assert!(p.options.is_none());
        assert!(p.metadata.is_none());
    }

    #[test]
    fn test_deserialize_full_enqueue_payload() {
        let json_str = r#"{
            "document_type": "CUSTOM_UPLOAD",
            "title": "Report Card.pdf",
            "recipient_name": "John Doe",
            "recipient_phone": "+91 98765 43210",
            "recipient_admission_number": "ADM001",
            "file_url": "data:application/pdf;base64,JVBERi0xLjQK...",
            "file_name": "Report Card.pdf",
            "file_size_bytes": 1048576,
            "copies": 3,
            "options": {
                "color": "COLOR",
                "paper_size": "A4",
                "orientation": "PORTRAIT"
            },
            "metadata": {
                "converted_from_csv": true,
                "notes": "Urgent print"
            }
        }"#;

        let payload: EnqueuePrintJobPayload = serde_json::from_str(json_str).unwrap();
        assert_eq!(payload.document_type, "CUSTOM_UPLOAD");
        assert_eq!(payload.copies, Some(3));
        assert_eq!(payload.file_size_bytes, Some(1048576));
        assert!(payload.options.is_some());
        assert!(payload.metadata.is_some());
    }
}
