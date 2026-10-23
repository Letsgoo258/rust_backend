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

    let school_id = auth_user.school_id;

    let tickets = sqlx::query_as::<_, ChangeTicketItem>(
        r#"SELECT
            id, school_id, user_id, requested_by_user_id, ticket_number,
            ticket_type, status, field_name, old_value, new_value,
            reason, priority, reviewer_user_id, reviewer_notes,
            created_at, updated_at
        FROM data_change_tickets
        WHERE school_id = $1
        ORDER BY created_at DESC
        LIMIT 100"#,
    )
    .bind(school_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "tickets": tickets,
    })))
}
