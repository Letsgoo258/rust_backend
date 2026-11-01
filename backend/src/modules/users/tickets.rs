use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateChangeTicketRequest {
    pub field_name: String,
    pub old_value: Option<String>,
    pub new_value: String,
    pub reason: String,
    pub priority: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ChangeTicketItem {
    pub id: Uuid,
    pub school_id: Uuid,
    pub user_id: Uuid,
    pub requested_by_user_id: Option<Uuid>,
    pub ticket_number: String,
    pub ticket_type: String,
    pub status: String,
    pub field_name: String,
    pub old_value: Option<String>,
    pub new_value: String,
    pub reason: String,
    pub priority: String,
    pub reviewer_user_id: Option<Uuid>,
    pub reviewer_notes: Option<String>,
    #[sqlx(default)]
    pub target_user_name: Option<String>,
    #[sqlx(default)]
    pub requester_name: Option<String>,
    #[sqlx(default)]
    pub reviewer_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CreateChangeTicketResponse {
    pub success: bool,
    pub message: String,
    pub ticket_id: Uuid,
    pub ticket_number: String,
}

/// Creates a new Change Request Ticket for critical login credentials or profile data
pub async fn create_change_ticket(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(target_user_id): Path<Uuid>,
    Json(payload): Json<CreateChangeTicketRequest>,
) -> Result<impl IntoResponse, AppError> {
    let auth_user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

    let school_id = auth_user.school_id;
    let requested_by = auth_user.id;

    let clean_field = payload.field_name.trim().to_lowercase();
    let clean_new_val = payload.new_value.trim().to_string();
    let clean_reason = payload.reason.trim().to_string();
    let clean_priority = payload
        .priority
        .as_deref()
        .map(|p| p.trim().to_uppercase())
        .filter(|p| p == "URGENT" || p == "HIGH")
        .unwrap_or_else(|| "STANDARD".to_string());

    if clean_field.is_empty() {
        return Err(AppError::BadRequest("Field name is required".into()));
    }
    if clean_new_val.is_empty() {
        return Err(AppError::BadRequest("New value cannot be empty".into()));
    }
    if clean_reason.is_empty() {
        return Err(AppError::BadRequest(
            "A reason or justification for changing this protected credential is required".into(),
        ));
    }

    // Verify target user belongs to same school
    let target_exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND school_id = $2)"#,
    )
    .bind(target_user_id)
    .bind(school_id)
    .fetch_one(&state.db)
    .await?;

    if !target_exists {
        return Err(AppError::NotFound(
            "Target user not found in this school".into(),
        ));
    }

    // Determine ticket type based on field
    let ticket_type = match clean_field.as_str() {
        "email" => "EMAIL_CHANGE",
        "phone" => "PHONE_CHANGE",
        "first_name" | "last_name" | "display_name" => "IDENTITY_CHANGE",
        _ => "CREDENTIAL_CHANGE",
    };

    let ticket_number = format!(
        "TKT-{}-{}",
        Utc::now().format("%Y%m%d"),
        &Uuid::new_v4().to_string()[..6].to_uppercase()
    );

    let ticket_id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO data_change_tickets (
            id,
            school_id,
            user_id,
            requested_by_user_id,
            ticket_number,
            ticket_type,
            status,
            field_name,
            old_value,
            new_value,
            reason,
            priority,
            created_at,
            updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, 'PENDING', $7, $8, $9, $10, $11, NOW(), NOW())"#,
    )
    .bind(ticket_id)
    .bind(school_id)
    .bind(target_user_id)
    .bind(requested_by)
    .bind(&ticket_number)
    .bind(ticket_type)
    .bind(&clean_field)
    .bind(payload.old_value.as_ref().map(|s| s.trim().to_string()))
    .bind(&clean_new_val)
    .bind(&clean_reason)
    .bind(&clean_priority)
    .execute(&state.db)
    .await?;

    // Record audit entry in entity_change_history
    let _ = sqlx::query(
        r#"INSERT INTO entity_change_history (
            school_id, entity_type, entity_id, action, updated_by_user_id, before_state, after_state
        ) VALUES ($1, 'DATA_CHANGE_TICKET', $2, 'CREATE', $3, $4, $5)"#,
    )
    .bind(school_id)
    .bind(ticket_id)
    .bind(requested_by)
    .bind(serde_json::json!({
        "target_user_id": target_user_id,
        "field_name": clean_field,
        "old_value": payload.old_value,
    }))
    .bind(serde_json::json!({
        "ticket_number": ticket_number,
        "new_value": clean_new_val,
        "reason": clean_reason,
        "priority": clean_priority,
    }))
    .execute(&state.db)
    .await;

    Ok((
        StatusCode::CREATED,
        Json(CreateChangeTicketResponse {
            success: true,
            message: format!(
                "Change Request Ticket #{} created successfully. Pending institutional verification.",
                ticket_number
            ),
            ticket_id,
            ticket_number,
        }),
    ))
}

/// Lists all change request tickets for a specific user
pub async fn list_user_tickets(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(target_user_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let auth_user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

    let school_id = auth_user.school_id;

    let tickets = sqlx::query_as::<_, ChangeTicketItem>(
        r#"SELECT
            id, school_id, user_id, requested_by_user_id, ticket_number,
            ticket_type, status, field_name, old_value, new_value,
            reason, priority, reviewer_user_id, reviewer_notes,
            created_at, updated_at
        FROM data_change_tickets
        WHERE school_id = $1 AND user_id = $2
        ORDER BY created_at DESC"#,
    )
    .bind(school_id)
    .bind(target_user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "tickets": tickets,
    })))
}

/// Lists all change request tickets for the school (admin view)
pub async fn list_school_tickets(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let auth_user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

    let is_authorized =
        auth_user.is_school_admin() || auth_user.has_capability("attendance:resolve_tickets");
    if !is_authorized {
        return Err(AppError::Forbidden(
            "Only school administrators can access institutional support tickets".into(),
        ));
    }

    let school_id = auth_user.school_id;

    let tickets = sqlx::query_as::<_, ChangeTicketItem>(
        r#"SELECT
            t.id, t.school_id, t.user_id, t.requested_by_user_id, t.ticket_number,
            t.ticket_type, t.status, t.field_name, t.old_value, t.new_value,
            t.reason, t.priority, t.reviewer_user_id, t.reviewer_notes,
            TRIM(CONCAT(u.first_name, ' ', COALESCE(u.last_name, ''))) as target_user_name,
            TRIM(CONCAT(req.first_name, ' ', COALESCE(req.last_name, ''))) as requester_name,
            TRIM(CONCAT(rev.first_name, ' ', COALESCE(rev.last_name, ''))) as reviewer_name,
            t.created_at, t.updated_at
        FROM data_change_tickets t
        LEFT JOIN users u ON t.user_id = u.id
        LEFT JOIN users req ON t.requested_by_user_id = req.id
        LEFT JOIN users rev ON t.reviewer_user_id = rev.id
        WHERE t.school_id = $1
        ORDER BY t.created_at DESC
        LIMIT 200"#,
    )
    .bind(school_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "tickets": tickets,
    })))
}

#[derive(Debug, Deserialize)]
pub struct ResolveTicketRequest {
    pub action: String, // "APPROVE" | "RESOLVE" | "REJECT"
    pub notes: String,
}

/// Resolves or rejects a data change ticket
pub async fn resolve_school_ticket(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(ticket_id): Path<Uuid>,
    Json(payload): Json<ResolveTicketRequest>,
) -> Result<impl IntoResponse, AppError> {
    let auth_user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

    let is_authorized =
        auth_user.is_school_admin() || auth_user.has_capability("attendance:resolve_tickets");
    if !is_authorized {
        return Err(AppError::Forbidden(
            "Only school administrators can resolve institutional tickets".into(),
        ));
    }

    let school_id = auth_user.school_id;
    let reviewer_id = auth_user.id;
    let clean_action = payload.action.trim().to_uppercase();
    let clean_notes = payload.notes.trim().to_string();

    let new_status = match clean_action.as_str() {
        "APPROVE" | "RESOLVE" => "RESOLVED",
        "REJECT" => "REJECTED",
        _ => {
            return Err(AppError::BadRequest(
                "Invalid resolution action. Use APPROVE, RESOLVE, or REJECT".into(),
            ))
        }
    };

    // Fetch existing ticket
    let ticket_row = sqlx::query(
        "SELECT id, user_id, ticket_number, ticket_type, field_name, new_value, status FROM data_change_tickets WHERE id = $1 AND school_id = $2",
    )
    .bind(ticket_id)
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Ticket not found".into()))?;

    use sqlx::Row;
    let target_user_id: Uuid = ticket_row.get("user_id");
    let ticket_number: String = ticket_row.get("ticket_number");
    let ticket_type: String = ticket_row.get("ticket_type");
    let field_name: String = ticket_row.get("field_name");
    let new_value: String = ticket_row.get("new_value");
    let previous_status: String = ticket_row.get("status");

    // Update ticket
    sqlx::query(
        "UPDATE data_change_tickets SET status = $1, reviewer_user_id = $2, reviewer_notes = $3, updated_at = NOW() WHERE id = $4",
    )
    .bind(&new_status)
    .bind(reviewer_id)
    .bind(&clean_notes)
    .bind(ticket_id)
    .execute(&state.db)
    .await?;

    // If ticket was an ATTENDANCE_COLLISION, sync attendance_collisions status
    if ticket_type == "ATTENDANCE_COLLISION" {
        let collision_status = if new_status == "RESOLVED" {
            "RESOLVED"
        } else {
            "DISMISSED"
        };
        let _ = sqlx::query(
            "UPDATE attendance_collisions SET status = $1, resolved_by_staff_id = $2, resolution_notes = $3, resolved_at = NOW(), updated_at = NOW() WHERE ticket_id = $4",
        )
        .bind(collision_status)
        .bind(reviewer_id)
        .bind(&clean_notes)
        .bind(ticket_id)
        .execute(&state.db)
        .await;
    }

    // If credential change ticket was approved, apply change to users table
    if new_status == "RESOLVED" && previous_status != "RESOLVED" {
        match field_name.as_str() {
            "email" => {
                let _ = sqlx::query(
                    "UPDATE users SET email = $1, updated_at = NOW() WHERE id = $2 AND school_id = $3",
                )
                .bind(&new_value)
                .bind(target_user_id)
                .bind(school_id)
                .execute(&state.db)
                .await;
            }
            "phone" => {
                let _ = sqlx::query(
                    "UPDATE users SET phone = $1, updated_at = NOW() WHERE id = $2 AND school_id = $3",
                )
                .bind(&new_value)
                .bind(target_user_id)
                .bind(school_id)
                .execute(&state.db)
                .await;
            }
            "first_name" => {
                let _ = sqlx::query(
                    "UPDATE users SET first_name = $1, updated_at = NOW() WHERE id = $2 AND school_id = $3",
                )
                .bind(&new_value)
                .bind(target_user_id)
                .bind(school_id)
                .execute(&state.db)
                .await;
            }
            "last_name" => {
                let _ = sqlx::query(
                    "UPDATE users SET last_name = $1, updated_at = NOW() WHERE id = $2 AND school_id = $3",
                )
                .bind(&new_value)
                .bind(target_user_id)
                .bind(school_id)
                .execute(&state.db)
                .await;
            }
            _ => {}
        }
    }

    // Log to entity_change_history
    let _ = sqlx::query(
        r#"INSERT INTO entity_change_history (
            school_id, entity_type, entity_id, action, updated_by_user_id, before_state, after_state
        ) VALUES ($1, 'DATA_CHANGE_TICKET', $2, 'RESOLVE', $3, $4, $5)"#,
    )
    .bind(school_id)
    .bind(ticket_id)
    .bind(reviewer_id)
    .bind(serde_json::json!({ "previous_status": previous_status }))
    .bind(serde_json::json!({
        "new_status": new_status,
        "notes": clean_notes,
        "ticket_number": ticket_number
    }))
    .execute(&state.db)
    .await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Ticket #{} {} successfully", ticket_number, new_status.to_lowercase()),
        "ticket_id": ticket_id,
        "status": new_status,
    })))
}
