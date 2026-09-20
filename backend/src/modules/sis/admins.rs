use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct AddAdminRequest {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: String,
}

pub async fn add_admin(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<AddAdminRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session.user.ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;
    
    // Check if email or phone already exists
    let existing = sqlx::query!("SELECT id FROM users WHERE email = $1 OR phone = $2", payload.email, payload.phone)
        .fetch_optional(&state.db).await?;
        
    if existing.is_some() {
        return Err(AppError::BadRequest("Email or phone already exists".into()));
    }
    
    // Generate a default username and password hash
    let username = format!("{}.{}", payload.first_name.to_lowercase(), payload.last_name.to_lowercase());
    let default_pass = crate::utils::password::hash_password("Welcome123!").map_err(|_| AppError::BadRequest("Hashing failed".into()))?;
    
    // Insert new admin (status INACTIVE until they verify)
    sqlx::query!(
        r#"
        INSERT INTO users (school_id, first_name, last_name, email, phone, username, password_hash, status, email_verified, phone_verified, roles, updated_by_user_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'INACTIVE', false, false, ARRAY['SCHOOL_ADMIN']::system_role_type[], $8)
        "#,
        user.school_id,
        payload.first_name,
        payload.last_name,
        payload.email,
        payload.phone,
        username,
        default_pass,
        user.id
    ).execute(&state.db).await?;
    
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "message": "Admin added successfully" }))))
}
