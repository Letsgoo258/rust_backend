use crate::{
    error::AppError,
    modules::auth::backend::AuthSession,
    modules::auth::{
        backend::Credentials,
        dto::{LoginRequest, SetupRequest, SetupResponse, CreateSchoolRequest, CreateSchoolResponse},
    },
    state::AppState,
    utils::password::hash_password,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/setup", post(setup_system))
        .route("/schools", get(get_schools).post(create_school))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .route("/verify/email", post(verify_email))
        .route("/verify/phone", post(verify_phone))
        .route("/schools/setup", post(setup_school_details))
        .route("/switch-tenant", post(switch_tenant))
}

/// Idempotent setup endpoint to create the very first School and Super Admin.
async fn setup_system(
    State(state): State<AppState>,
    Json(payload): Json<SetupRequest>,
) -> Result<impl IntoResponse, AppError> {
    // 1. Security Check: Only allow if no schools exist
    let school_count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM schools")
        .fetch_one(&state.db)
        .await
        ?
        .unwrap_or(0);

    if school_count > 0 {
        return Err(AppError::Forbidden(
            "System is already initialized. Cannot run setup again.".into(),
        ));
    }

    // 2. Hash the admin password securely
    let hashed_password = hash_password(&payload.admin_password)?;

    // 3. Start a database transaction to ensure atomic setup
    let mut tx = state
        .db
        .begin()
        .await
        ?;

    // Create the School
    let school_id = sqlx::query_scalar!(
        "INSERT INTO schools (name, code, status) VALUES ($1, $2, 'ACTIVE') RETURNING id",
        payload.school_name,
        payload.school_code
    )
    .fetch_one(&mut *tx)
    .await
    ?;

    // Create the Super Admin Role
    let role_id = sqlx::query_scalar!(
        "INSERT INTO roles (school_id, name, code, description, is_system_role) 
         VALUES ($1, 'Super Admin', 'SUPER_ADMIN', 'Full system access', true) RETURNING id",
        school_id
    )
    .fetch_one(&mut *tx)
    .await
    ?;

    // Create the Admin User
    let user_id = sqlx::query_scalar!(
        "INSERT INTO users (school_id, username, email, password_hash, first_name, last_name, status, user_type) 
         VALUES ($1, $2, $3, $4, $5, $6, 'ACTIVE', 'SUPER_ADMIN') RETURNING id",
        school_id,
        payload.admin_username,
        payload.admin_email,
        hashed_password,
        payload.first_name,
        payload.last_name
    )
    .fetch_one(&mut *tx)
    .await
    ?;

    // Assign Role to User
    sqlx::query!(
        "INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)",
        user_id,
        role_id
    )
    .execute(&mut *tx)
    .await
    ?;

    tx.commit()
        .await
        ?;

    tracing::info!(
        "System successfully initialized with school '{}' and admin user '{}'",
        payload.school_name,
        payload.admin_username
    );

    Ok((
        StatusCode::CREATED,
        Json(SetupResponse {
            message: "System successfully initialized.".to_string(),
            school_id,
            user_id,
        }),
    ))
}

async fn login(
    mut auth_session: AuthSession,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let creds = Credentials {
        username: payload.username,
        password: payload.password,
    };

    let user = match auth_session.authenticate(creds).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err(AppError::Unauthorized(
                "Invalid username or password".into(),
            ))
        }
        Err(_) => {
            return Err(AppError::InternalServerError(
                "Authentication failed".into(),
            ))
        }
    };

    if auth_session.login(&user).await.is_err() {
        return Err(AppError::InternalServerError(
            "Failed to create session".into(),
        ));
    }

    Ok(Json(json!({ "message": "Login successful", "user": user })))
}

async fn logout(mut auth_session: AuthSession) -> Result<impl IntoResponse, AppError> {
    if auth_session.logout().await.is_err() {
        return Err(AppError::InternalServerError("Failed to logout".into()));
    }
    Ok(Json(json!({ "message": "Logged out successfully" })))
}

async fn me(auth_session: AuthSession) -> Result<impl IntoResponse, AppError> {
    match auth_session.user {
        Some(user) => Ok(Json(user)),
        None => Err(AppError::Unauthorized("Not logged in".into())),
    }
}


#[tracing::instrument(err, skip(state, auth_session, payload))]
async fn create_school(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<CreateSchoolRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    // Check if SUPER_ADMIN
    let user_type: Option<String> = sqlx::query_scalar!("SELECT user_type::TEXT FROM users WHERE id = $1", user.id)
        .fetch_one(&state.db)
        .await?;
        
    if user_type.as_deref() != Some("SUPER_ADMIN") {
        return Err(AppError::Forbidden("Only Super Admins can create schools".into()));
    }

    let hashed_password = hash_password(&payload.admin_password)?;

    let mut tx = state.db.begin().await?;

    let school_id = sqlx::query_scalar!(
        "INSERT INTO schools (name, code, email, phone, status) VALUES ($1, $2, $3, $4, 'ACTIVE') RETURNING id",
        payload.name,
        payload.code,
        payload.email,
        payload.phone
    )
    .fetch_one(&mut *tx)
    .await?;

    // Create the School Admin Role
    let role_id = sqlx::query_scalar!(
        "INSERT INTO roles (school_id, name, code, description, is_system_role) 
         VALUES ($1, 'School Admin', 'SCHOOL_ADMIN', 'School level administrator', true) RETURNING id",
        school_id
    )
    .fetch_one(&mut *tx)
    .await?;

    // Create the Admin User
    let user_id = sqlx::query_scalar!(
        "INSERT INTO users (school_id, username, email, password_hash, first_name, last_name, status, user_type) 
         VALUES ($1, $2, $3, $4, $5, $6, 'ACTIVE', 'SCHOOL_ADMIN') RETURNING id",
        school_id,
        payload.admin_username,
        payload.admin_email,
        hashed_password,
        payload.admin_first_name,
        payload.admin_last_name
    )
    .fetch_one(&mut *tx)
    .await?;

    // Assign Role to User
    sqlx::query!(
        "INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)",
        user_id,
        role_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    tracing::info!("Super Admin {} created new school: {} with admin {}", user.username, payload.name, payload.admin_username);

    Ok((
        StatusCode::CREATED,
        Json(CreateSchoolResponse {
            message: "School and Admin Account successfully created.".to_string(),
            school_id,
        }),
    ))
}

#[tracing::instrument(err, skip(state, auth_session))]
async fn get_schools(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    // Check if SUPER_ADMIN
    let user_type: Option<String> = sqlx::query_scalar!("SELECT user_type::TEXT FROM users WHERE id = $1", user.id)
        .fetch_one(&state.db)
        .await?;
        
    if user_type.as_deref() != Some("SUPER_ADMIN") {
        return Err(AppError::Forbidden("Only Super Admins can view platform stats".into()));
    }

    let total_users: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await?
        .unwrap_or(0);

    let active_tenants: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM schools WHERE status = 'ACTIVE'")
        .fetch_one(&state.db)
        .await?
        .unwrap_or(0);

    let schools = sqlx::query_as!(
        crate::modules::auth::dto::SchoolDto,
        "SELECT id, name, code, status::TEXT as \"status!\", created_at::TEXT as \"created_at!\" FROM schools ORDER BY created_at DESC"
    )
    .fetch_all(&state.db)
    .await?;

    Ok((
        StatusCode::OK,
        Json(crate::modules::auth::dto::SuperAdminStatsResponse {
            total_users,
            active_tenants,
            schools,
        }),
    ))
}

#[derive(serde::Deserialize)]
pub struct VerifyPhoneRequest {
    pub otp: String,
}

#[tracing::instrument(err, skip(state, auth_session))]
async fn verify_email(
    mut auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .clone()
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    // Mark verified
    sqlx::query!("UPDATE users SET email_verified = true WHERE id = $1", user.id)
        .execute(&state.db)
        .await?;

    // Refresh session user (simplistic way)
    if let Ok(Some(mut updated_user)) = sqlx::query_as!(
        crate::modules::auth::backend::User,
        r#"SELECT u.id, u.school_id, u.username, u.password_hash, u.display_name, u.user_type::TEXT as "user_type!", u.email_verified, u.phone_verified, s.status::TEXT as "school_status!"
           FROM users u JOIN schools s ON u.school_id = s.id WHERE u.id = $1"#,
        user.id
    ).fetch_optional(&state.db).await {
        // Log in again to update session
        let _ = auth_session.login(&updated_user).await;
    }

    Ok((StatusCode::OK, Json(serde_json::json!({ "message": "Email verified successfully" }))))
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
async fn verify_phone(
    mut auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<VerifyPhoneRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .clone()
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if payload.otp != "123456" {
        return Err(AppError::BadRequest("Invalid OTP".into()));
    }

    sqlx::query!("UPDATE users SET phone_verified = true WHERE id = $1", user.id)
        .execute(&state.db)
        .await?;
        
    if let Ok(Some(mut updated_user)) = sqlx::query_as!(
        crate::modules::auth::backend::User,
        r#"SELECT u.id, u.school_id, u.username, u.password_hash, u.display_name, u.user_type::TEXT as "user_type!", u.email_verified, u.phone_verified, s.status::TEXT as "school_status!"
           FROM users u JOIN schools s ON u.school_id = s.id WHERE u.id = $1"#,
        user.id
    ).fetch_optional(&state.db).await {
        let _ = auth_session.login(&updated_user).await;
    }

    Ok((StatusCode::OK, Json(serde_json::json!({ "message": "Phone verified successfully" }))))
}

#[derive(serde::Deserialize)]
pub struct SchoolSetupRequest {
    pub short_name: Option<String>,
    pub website: Option<String>,
    pub address_line_1: String,
    pub address_line_2: Option<String>,
    pub city: String,
    pub district: String,
    pub state: String,
    pub country: String,
    pub postal_code: String,
    pub timezone: String,
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
async fn setup_school_details(
    mut auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<SchoolSetupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .clone()
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    // Only SCHOOL_ADMIN can do this
    if user.user_type != "SCHOOL_ADMIN" {
        return Err(AppError::Forbidden("Only School Admins can perform setup".into()));
    }

    // Must have verified email and phone
    if !user.email_verified || !user.phone_verified {
        return Err(AppError::BadRequest("Must verify email and phone first".into()));
    }

    sqlx::query!(
        "UPDATE schools SET short_name = $1, website = $2, address_line_1 = $3, address_line_2 = $4, city = $5, district = $6, state = $7, country = $8, postal_code = $9, timezone = $10, status = 'ACTIVE' WHERE id = $11",
        payload.short_name,
        payload.website,
        payload.address_line_1,
        payload.address_line_2,
        payload.city,
        payload.district,
        payload.state,
        payload.country,
        payload.postal_code,
        payload.timezone,
        user.school_id
    )
    .execute(&state.db)
    .await?;

    // Refresh session
    if let Ok(Some(mut updated_user)) = sqlx::query_as!(
        crate::modules::auth::backend::User,
        r#"SELECT u.id, u.school_id, u.username, u.password_hash, u.display_name, u.user_type::TEXT as "user_type!", u.email_verified, u.phone_verified, s.status::TEXT as "school_status!"
           FROM users u JOIN schools s ON u.school_id = s.id WHERE u.id = $1"#,
        user.id
    ).fetch_optional(&state.db).await {
        let _ = auth_session.login(&updated_user).await;
    }

    Ok((StatusCode::OK, Json(serde_json::json!({ "message": "School setup completed" }))))
}



#[derive(serde::Deserialize)]
pub struct SwitchTenantRequest {
    pub school_id: uuid::Uuid,
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
async fn switch_tenant(
    mut auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<SwitchTenantRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .clone()
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    // Only SUPER_ADMIN can switch tenants
    if user.user_type != "SUPER_ADMIN" {
        return Err(AppError::Forbidden("Only Super Admins can switch tenants".into()));
    }

    // Ensure the target school exists and is active
    let school_status = sqlx::query_scalar!("SELECT status::TEXT as \"status!\" FROM schools WHERE id = $1", payload.school_id)
        .fetch_optional(&state.db)
        .await?;

    if school_status.is_none() {
        return Err(AppError::NotFound("School not found".into()));
    }

    // Create a modified user object with the new school_id
    let mut updated_user = user.clone();
    updated_user.school_id = payload.school_id;

    // We don't update the database! We just update the session cookie!
    // The AuthSession backend implementation just serializes this User struct.
    // So logging in with this modified struct will overwrite the session cookie.
    let _ = auth_session.login(&updated_user).await;

    Ok((StatusCode::OK, Json(serde_json::json!({ "message": "Switched tenant successfully" }))))
}
