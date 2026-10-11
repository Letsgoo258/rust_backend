use super::repository::UserRepository;
use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;

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
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SUPER_ADMIN".to_string()) {
        crate::utils::telemetry::dispatch_security_alert(
            &state.db,
            user.school_id,
            user.id,
            "ADD_SCHOOL_ADMIN_ATTEMPT",
            "users_admins_add",
            "Only Super Admins can add new School Admins",
            &user.roles,
            None,
        )
        .await;
        return Err(AppError::Forbidden(
            "Only Super Admins can add new School Admins".into(),
        ));
    }

    let norm_phone = crate::utils::phone::normalize_phone_number(&payload.phone);

    // Check if email or phone already exists
    let exists =
        UserRepository::check_user_exists_by_email_or_phone(&state.db, &payload.email, &norm_phone)
            .await?;
    if exists {
        return Err(AppError::BadRequest("Email or phone already exists".into()));
    }

    // Generate a default username and password hash
    let username = format!(
        "{}.{}",
        payload.first_name.to_lowercase(),
        payload.last_name.to_lowercase()
    );
    let default_pass = crate::utils::password::hash_password("Welcome123!")
        .map_err(|_| AppError::BadRequest("Hashing failed".into()))?;

    let new_admin_id = UserRepository::create_admin_user(
        &state.db,
        user.school_id,
        &payload.first_name,
        &payload.last_name,
        &payload.email,
        &norm_phone,
        &username,
        &default_pass,
        user.id,
    )
    .await?;

    // 1. Dispatch Telemetry Event to all configured adapters
    let telemetry_payload = crate::utils::telemetry::TelemetryPayload {
        school_id: Some(user.school_id),
        user_id: Some(user.id),
        session_id: format!("admin-create-{}", uuid::Uuid::new_v4()),
        event_type: "ADMIN_USER_CREATED".into(),
        severity: "INFO".into(),
        url: None,
        component: Some("users_admins_add".into()),
        error_message: None,
        error_stack: None,
        duration_ms: None,
        metadata: Some(serde_json::json!({
            "action": "CREATE_SCHOOL_ADMIN",
            "new_admin_id": new_admin_id,
            "new_admin_username": username,
            "new_admin_email": payload.email,
            "created_by_user_id": user.id,
            "school_id": user.school_id,
            "timestamp": time::OffsetDateTime::now_utc().to_string()
        })),
        timestamp: time::OffsetDateTime::now_utc().to_string(),
    };

    let dispatcher =
        crate::utils::telemetry::TelemetryDispatcher::with_default_adapters(Some(state.db.clone()));
    dispatcher.dispatch(telemetry_payload).await;

    // 2. Structured Tracing Log
    tracing::info!(
        target: "eravaya::admin_lifecycle",
        event = "ADMIN_USER_CREATED",
        school_id = %user.school_id,
        created_by_user_id = %user.id,
        new_admin_id = %new_admin_id,
        new_admin_username = %username,
        new_admin_email = %payload.email,
        "New School Admin user '{}' ({}) successfully created by Super Admin {}",
        username,
        payload.email,
        user.id
    );

    // 3. Record in Audit Subsystem
    let _ = crate::utils::audit::log_action(
        &state.db,
        Some(user.school_id),
        Some(user.id),
        "CREATE_SCHOOL_ADMIN",
        Some("users"),
        Some(new_admin_id),
        Some(serde_json::json!({
            "username": username,
            "email": payload.email,
            "phone": norm_phone
        })),
        None,
    )
    .await;

    // 4. Dispatch System Notification Alert
    let notif_msg = format!(
        "New School Admin account provisioned: {} {} ({}) by Super Admin.",
        payload.first_name, payload.last_name, payload.email
    );
    let _ = sqlx::query(
        r#"
        INSERT INTO notifications (user_id, message, notification_type, status, priority)
        VALUES ($1, $2, 'EMAIL'::notification_type, 'PENDING'::notification_status, 2)
        "#,
    )
    .bind(user.id)
    .bind(&notif_msg)
    .execute(&state.db)
    .await;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "message": "Admin added successfully",
            "admin_id": new_admin_id,
            "username": username
        })),
    ))
}
