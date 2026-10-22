use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CustomFieldItem {
    pub id: Uuid,
    pub school_id: Uuid,
    pub entity_type: String,
    pub field_name: String,
    pub field_key: String,
    pub field_type: String,
    pub is_required: Option<bool>,
    pub options_json: Option<serde_json::Value>,
    pub display_order: Option<i32>,
    pub created_at: time::OffsetDateTime,
}

#[derive(Debug, Deserialize)]
pub struct CreateCustomFieldRequest {
    pub entity_type: Option<String>,
    pub field_name: String,
    pub field_key: String,
    pub field_type: String,
    pub is_required: Option<bool>,
    pub options_json: Option<serde_json::Value>,
    pub display_order: Option<i32>,
}

/// GET /api/v1/users/custom-fields
pub async fn get_custom_fields(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let auth_user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let entity_type_filter = params.get("entity_type").map(|s| s.as_str()).unwrap_or("");

    let fields = if entity_type_filter.is_empty() {
        sqlx::query_as::<_, CustomFieldItem>(
            r#"SELECT id, school_id, entity_type, field_name, field_key, field_type,
                      is_required, options_json, display_order, created_at
               FROM custom_fields
               WHERE school_id = $1
               ORDER BY display_order ASC, created_at ASC"#,
        )
        .bind(auth_user.school_id)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as::<_, CustomFieldItem>(
            r#"SELECT id, school_id, entity_type, field_name, field_key, field_type,
                      is_required, options_json, display_order, created_at
               FROM custom_fields
               WHERE school_id = $1 AND (entity_type = $2 OR entity_type = 'USER')
               ORDER BY display_order ASC, created_at ASC"#,
        )
        .bind(auth_user.school_id)
        .bind(entity_type_filter)
        .fetch_all(&state.db)
        .await?
    };

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "custom_fields": fields })),
    ))
}

/// POST /api/v1/users/custom-fields
pub async fn create_custom_field(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<CreateCustomFieldRequest>,
) -> Result<impl IntoResponse, AppError> {
    let auth_user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !auth_user.is_school_admin() && !auth_user.roles.contains(&"SUPER_ADMIN".to_string()) {
        return Err(AppError::Forbidden(
            "Only School Admins can manage custom fields".into(),
        ));
    }

    let field_name = payload.field_name.trim();
    if field_name.is_empty() {
        return Err(AppError::BadRequest("Field name is required".into()));
    }

    let field_key = payload
        .field_key
        .trim()
        .to_lowercase()
        .replace([' ', '-'], "_");

    if field_key.is_empty() || !field_key.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(AppError::BadRequest(
            "Field key must contain only letters, numbers, and underscores".into(),
        ));
    }

    // Disallow overwriting strict core field keys
    const STRICT_CORE_KEYS: &[&str] = &[
        "id",
        "display_name",
        "first_name",
        "last_name",
        "username",
        "email",
        "phone",
        "date_of_birth",
        "gender",
        "blood_group",
        "admission_number",
        "employee_id",
        "class_id",
        "section_id",
        "class_section_id",
        "roll_number",
        "status",
        "roles",
    ];
    if STRICT_CORE_KEYS.contains(&field_key.as_str()) {
        return Err(AppError::BadRequest(format!(
            "'{}' is a reserved strict core field key and cannot be reused",
            field_key
        )));
    }

    let raw_type = payload.field_type.trim().to_uppercase();
    let field_type =
        match raw_type.as_str() {
            "TEXT" | "NUMBER" | "DATE" | "SELECT" | "BOOLEAN" | "EMAIL" | "PHONE" => raw_type,
            _ => return Err(AppError::BadRequest(
                "Invalid field type. Allowed: TEXT, NUMBER, DATE, SELECT, BOOLEAN, EMAIL, PHONE"
                    .into(),
            )),
        };

    let entity_type = payload
        .entity_type
        .as_deref()
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "USER".to_string());

    // Check duplicate key
    let existing_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM custom_fields
           WHERE school_id = $1 AND entity_type = $2 AND field_key = $3"#,
    )
    .bind(auth_user.school_id)
    .bind(&entity_type)
    .bind(&field_key)
    .fetch_one(&state.db)
    .await?;

    if existing_count > 0 {
        return Err(AppError::BadRequest(format!(
            "A field with key '{}' already exists for {}",
            field_key, entity_type
        )));
    }

    let new_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO custom_fields (school_id, entity_type, field_name, field_key, field_type, is_required, options_json, display_order)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id"#
    )
    .bind(auth_user.school_id)
    .bind(&entity_type)
    .bind(field_name)
    .bind(&field_key)
    .bind(&field_type)
    .bind(payload.is_required.unwrap_or(false))
    .bind(payload.options_json)
    .bind(payload.display_order.unwrap_or(0))
    .fetch_one(&state.db)
    .await?;

    let _ = crate::utils::audit::log_action(
        &state.db,
        Some(auth_user.school_id),
        Some(auth_user.id),
        "CREATE_CUSTOM_FIELD",
        Some("SCHOOL_SETTINGS"),
        Some(new_id),
        Some(serde_json::json!({
            "field_id": new_id,
            "field_key": field_key,
            "field_name": field_name,
            "field_type": field_type,
            "entity_type": entity_type
        })),
        None,
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "message": "Custom field created successfully",
            "field_id": new_id,
            "field_key": field_key
        })),
    ))
}

/// DELETE /api/v1/users/custom-fields/:id
pub async fn delete_custom_field(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(field_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let auth_user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !auth_user.is_school_admin() && !auth_user.roles.contains(&"SUPER_ADMIN".to_string()) {
        return Err(AppError::Forbidden(
            "Only School Admins can manage custom fields".into(),
        ));
    }

    let rows_affected = sqlx::query("DELETE FROM custom_fields WHERE id = $1 AND school_id = $2")
        .bind(field_id)
        .bind(auth_user.school_id)
        .execute(&state.db)
        .await?
        .rows_affected();

    if rows_affected == 0 {
        return Err(AppError::NotFound(
            "Custom field not found or already deleted".into(),
        ));
    }

    let _ = crate::utils::audit::log_action(
        &state.db,
        Some(auth_user.school_id),
        Some(auth_user.id),
        "DELETE_CUSTOM_FIELD",
        Some("SCHOOL_SETTINGS"),
        Some(field_id),
        None,
        None,
    )
    .await;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "Custom field deleted successfully"
        })),
    ))
}
