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
        .route("/add-admin", post(invite_admin))
        .route("/setup", post(setup_system))
        .route("/schools", post(create_school))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .route("/verify/request-otp", post(request_verify_otp))
        .route("/verify/submit-otp", post(submit_verify_otp))
        .route("/password-reset/request", post(request_password_reset))
        .route("/password-reset/confirm", post(confirm_password_reset))
        .route("/schools/setup", post(setup_school_details))
        .route("/switch-tenant", post(switch_tenant))
        .route("/super-admin/stats", axum::routing::get(get_super_admin_stats))
        .route("/super-admin/schools/:id/status", axum::routing::patch(update_school_status))
        .route("/super-admin/schools/:id/request-delete-otp", axum::routing::post(request_delete_otp))
        .route("/super-admin/schools/:id/delete", axum::routing::post(delete_school))
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
        "INSERT INTO users (school_id, username, email, password_hash, first_name, last_name, status, roles) 
         VALUES ($1, $2, $3, $4, $5, $6, 'ACTIVE', '{SUPER_ADMIN}') RETURNING id",
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
    axum::extract::State(state): axum::extract::State<crate::state::AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
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

    crate::utils::audit::log_action(&state.db, Some(user.school_id), Some(user.id), "LOGIN", None, None, None, Some(addr.to_string())).await.ok();
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
    let roles = sqlx::query_scalar!("SELECT roles::TEXT[] as \"roles!\" FROM users WHERE id = $1", user.id)
        .fetch_one(&state.db)
        .await?;
        
    if !roles.contains(&"SUPER_ADMIN".to_string()) {
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
         VALUES ($1, 'School Admin', '{SCHOOL_ADMIN}', 'School level administrator', true) RETURNING id",
        school_id
    )
    .fetch_one(&mut *tx)
    .await?;

    // Create the Admin User
    let user_id = sqlx::query_scalar!(
        "INSERT INTO users (school_id, username, email, password_hash, first_name, last_name, status, roles) 
         VALUES ($1, $2, $3, $4, $5, $6, 'ACTIVE', '{SCHOOL_ADMIN}') RETURNING id",
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
async fn get_super_admin_stats(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    // Check if SUPER_ADMIN
    let roles = sqlx::query_scalar!("SELECT roles::TEXT[] as \"roles!\" FROM users WHERE id = $1", user.id)
        .fetch_one(&state.db)
        .await?;
        
    if !roles.contains(&"SUPER_ADMIN".to_string()) {
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
    if !user.roles.contains(&"SCHOOL_ADMIN".to_string()) {
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
    if let Ok(Some(updated_user)) = sqlx::query_as!(
        crate::modules::auth::backend::User,
        r#"SELECT u.id, u.school_id, u.username, u.password_hash, u.display_name, u.roles::TEXT[] as "roles!", u.email_verified, u.phone_verified, s.status::TEXT as "school_status!"
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
    if !user.roles.contains(&"SUPER_ADMIN".to_string()) {
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


#[derive(serde::Deserialize, Debug)]
pub struct InviteAdminRequest {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
async fn invite_admin(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<InviteAdminRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SCHOOL_ADMIN".to_string()) {
        return Err(AppError::Forbidden("Only School Admins can invite other admins.".into()));
    }

    if user.school_status != "ACTIVE" {
        return Err(AppError::Forbidden("School setup must be completed first.".into()));
    }

    let password_hash = crate::utils::password::hash_password("password")
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let new_user_id = sqlx::query_scalar!(
        r#"
        INSERT INTO users (school_id, email, password_hash, first_name, last_name, phone, roles, email_verified, phone_verified, status)
        VALUES ($1, $2, $3, $4, $5, $6, '{SCHOOL_ADMIN}', false, false, 'ACTIVE')
        RETURNING id
        "#,
        user.school_id,
        payload.email,
        password_hash,
        payload.first_name,
        payload.last_name,
        payload.phone
    )
    .fetch_one(&state.db)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "message": "School Admin invited successfully. Default password is 'password'. They must verify email and phone on first login.",
            "user_id": new_user_id
        })),
    ))
}


pub async fn update_school_status(
    auth_session: AuthSession,
    axum::extract::State(state): axum::extract::State<crate::state::AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
    axum::Json(payload): axum::Json<crate::modules::auth::dto::UpdateSchoolStatusRequest>,
) -> Result<impl axum::response::IntoResponse, crate::error::AppError> {
    let user = auth_session.user.ok_or_else(|| crate::error::AppError::Unauthorized("Not logged in".into()))?;
    
    let roles = sqlx::query_scalar!("SELECT roles::TEXT[] as \"roles!\" FROM users WHERE id = $1", user.id)
        .fetch_one(&state.db).await?;
        
    if !roles.contains(&"SUPER_ADMIN".to_string()) {
        return Err(crate::error::AppError::Forbidden("Only Super Admins can update school status".into()));
    }

    if payload.status != "ACTIVE" && payload.status != "INACTIVE" {
        return Err(crate::error::AppError::BadRequest("Invalid status".into()));
    }

    let rows_affected = sqlx::query("UPDATE schools SET status = $1::school_status WHERE id = $2")
        .bind(payload.status)
        .bind(id)
        .execute(&state.db).await?.rows_affected();
        
    if rows_affected == 0 {
        return Err(crate::error::AppError::NotFound("School not found".into()));
    }

    Ok((axum::http::StatusCode::OK, axum::Json(serde_json::json!({ "message": "Status updated successfully" }))))
}

pub async fn request_delete_otp(
    auth_session: AuthSession,
    axum::extract::State(state): axum::extract::State<crate::state::AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<impl axum::response::IntoResponse, crate::error::AppError> {
    let user = auth_session.user.ok_or_else(|| crate::error::AppError::Unauthorized("Not logged in".into()))?;
    
    let roles = sqlx::query_scalar!("SELECT roles::TEXT[] as \"roles!\" FROM users WHERE id = $1", user.id)
        .fetch_one(&state.db).await?;
        
    if !roles.contains(&"SUPER_ADMIN".to_string()) {
        return Err(crate::error::AppError::Forbidden("Only Super Admins can request deletion".into()));
    }

    // Verify school exists
    let exists = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM schools WHERE id = $1)", id).fetch_one(&state.db).await?.unwrap_or(false);
    if !exists {
        return Err(crate::error::AppError::NotFound("School not found".into()));
    }

    // Generate a secure 6-character OTP from UUID
    let otp = uuid::Uuid::new_v4().to_string().chars().filter(|c| c.is_ascii_alphanumeric()).take(6).collect::<String>().to_uppercase();
    
    // Invalidate existing OTPs for this action and target
    sqlx::query!("DELETE FROM admin_action_otps WHERE user_id = $1 AND action_type = 'DELETE_SCHOOL' AND target_id = $2", user.id, id)
        .execute(&state.db).await?;

    // Save OTP
    sqlx::query!("INSERT INTO admin_action_otps (user_id, action_type, target_id, otp_code, expires_at) VALUES ($1, 'DELETE_SCHOOL', $2, $3, NOW() + INTERVAL '10 minutes')", user.id, id, otp)
        .execute(&state.db).await?;

    let email_body = format!(
        "<h1>Action Required</h1>\n\
        <p>You have requested to <b>permanently delete</b> school tenant <code>{}</code>.</p>\n\
        <p>If you initiated this action, please enter the following OTP to confirm:</p>\n\
        <h2 style='color:red; letter-spacing:4px'>{}</h2>\n\
        <p>This code will expire in 10 minutes.</p>\n\
        <br>\n\
        <p><i>If you did not request this action, please ignore this email.</i></p>",
        id, otp
    );

    if let Err(e) = crate::utils::email::send_email(&user.username, "Eravaya: Confirm Tenant Deletion", &email_body).await {
        tracing::error!("Failed to send OTP email to {}: {}", user.username, e);
        // Fallback for dev
        tracing::warn!("============================================================");
        tracing::warn!("OTP DISPATCHED FOR SUPER ADMIN: {}", user.username);
        tracing::warn!("ACTION: DELETE_SCHOOL");
        tracing::warn!("TARGET: {}", id);
        tracing::warn!("YOUR OTP CODE IS: {}", otp);
        tracing::warn!("============================================================");
    } else {
        tracing::info!("Sent OTP email to {}", user.username);
    }

    Ok((axum::http::StatusCode::OK, axum::Json(serde_json::json!({ "message": "OTP generated and printed to server console" }))))
}

pub async fn delete_school(
    auth_session: AuthSession,
    axum::extract::State(state): axum::extract::State<crate::state::AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
    axum::Json(payload): axum::Json<crate::modules::auth::dto::DeleteSchoolRequest>,
) -> Result<impl axum::response::IntoResponse, crate::error::AppError> {
    let user = auth_session.user.ok_or_else(|| crate::error::AppError::Unauthorized("Not logged in".into()))?;
    
    let roles = sqlx::query_scalar!("SELECT roles::TEXT[] as \"roles!\" FROM users WHERE id = $1", user.id)
        .fetch_one(&state.db).await?;
        
    if !roles.contains(&"SUPER_ADMIN".to_string()) {
        return Err(crate::error::AppError::Forbidden("Only Super Admins can delete schools".into()));
    }

    // Verify OTP
    let stored_otp = sqlx::query!(
        "SELECT id, otp_code, expires_at FROM admin_action_otps WHERE user_id = $1 AND action_type = 'DELETE_SCHOOL' AND target_id = $2 ORDER BY created_at DESC LIMIT 1",
        user.id, id
    ).fetch_optional(&state.db).await?;

    if let Some(record) = stored_otp {
        if record.expires_at < time::OffsetDateTime::now_utc() {
            return Err(crate::error::AppError::BadRequest("OTP has expired".into()));
        }
        if record.otp_code != payload.otp {
            return Err(crate::error::AppError::BadRequest("Invalid OTP".into()));
        }
        
        // OTP is valid. Delete it so it can't be reused.
        sqlx::query!("DELETE FROM admin_action_otps WHERE id = $1", record.id).execute(&state.db).await?;
    } else {
        return Err(crate::error::AppError::BadRequest("No valid OTP requested for this action".into()));
    }

    // Nuke the school
    let rows_affected = sqlx::query!("DELETE FROM schools WHERE id = $1", id)
        .execute(&state.db).await?.rows_affected();
        
    if rows_affected == 0 {
        return Err(crate::error::AppError::NotFound("School not found".into()));
    }

    crate::utils::audit::log_action(&state.db, Some(id), Some(user.id), "DELETE_SCHOOL", None, None, None, None).await.ok();

    Ok((axum::http::StatusCode::OK, axum::Json(serde_json::json!({ "message": "School permanently deleted" }))))
}

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn request_verify_otp(
    auth_session: AuthSession,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    let user = auth_session.user.clone().ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let email_otp = uuid::Uuid::new_v4().to_string().chars().filter(|c| c.is_ascii_alphanumeric()).take(6).collect::<String>().to_uppercase();
    let phone_otp = uuid::Uuid::new_v4().to_string().chars().filter(|c| c.is_ascii_alphanumeric()).take(6).collect::<String>().to_uppercase();
    
    sqlx::query!("DELETE FROM admin_action_otps WHERE user_id = $1 AND action_type IN ('VERIFY_EMAIL', 'VERIFY_PHONE')", user.id)
        .execute(&state.db).await?;

    sqlx::query!("INSERT INTO admin_action_otps (user_id, action_type, otp_code, expires_at) VALUES ($1, 'VERIFY_EMAIL', $2, NOW() + INTERVAL '15 minutes')", user.id, email_otp)
        .execute(&state.db).await?;
        
    sqlx::query!("INSERT INTO admin_action_otps (user_id, action_type, otp_code, expires_at) VALUES ($1, 'VERIFY_PHONE', $2, NOW() + INTERVAL '15 minutes')", user.id, phone_otp)
        .execute(&state.db).await?;

    let email_body = format!(
        "<h1>Verify your Eravaya Account</h1>
\
        <p>Your Email Verification Code is:</p>
\
        <h2 style='color:blue; letter-spacing:4px'>{}</h2>
\
        <p>This code will expire in 15 minutes.</p>",
        email_otp
    );

    if let Err(e) = crate::utils::email::send_email(&user.username, "Eravaya: Email Verification Code", &email_body).await {
        tracing::error!("Failed to send verification email to {}: {}", user.username, e);
    } else {
        tracing::info!("Verification email sent successfully to {}", user.username);
    }
    
    tracing::warn!("============================================================");
    tracing::warn!("WHATSAPP OTP DISPATCHED TO TERMINAL");
    tracing::warn!("USER: {}", user.username);
    tracing::warn!("YOUR WHATSAPP VERIFICATION CODE IS: {}", phone_otp);
    
    // For safety, also log the email one to the terminal in case email fails completely in dev
    tracing::warn!("YOUR EMAIL VERIFICATION CODE IS: {}", email_otp);
    tracing::warn!("============================================================");

    Ok((axum::http::StatusCode::OK, axum::Json(serde_json::json!({ "message": "OTPs sent to email and WhatsApp" }))))
}

#[derive(Debug, serde::Deserialize)]
pub struct SubmitVerifyOtpRequest {
    pub email_otp: String,
    pub phone_otp: String,
}

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn submit_verify_otp(
    auth_session: AuthSession,
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::Json(payload): axum::Json<SubmitVerifyOtpRequest>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    let user = auth_session.user.clone().ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    // Verify Email OTP
    let stored_email = sqlx::query!("SELECT id, otp_code, expires_at FROM admin_action_otps WHERE user_id = $1 AND action_type = 'VERIFY_EMAIL' ORDER BY created_at DESC LIMIT 1", user.id).fetch_optional(&state.db).await?;
    if let Some(record) = stored_email {
        if record.expires_at < time::OffsetDateTime::now_utc() { return Err(AppError::BadRequest("Email OTP expired".into())); }
        if record.otp_code != payload.email_otp { return Err(AppError::BadRequest("Invalid Email OTP".into())); }
        sqlx::query!("DELETE FROM admin_action_otps WHERE id = $1", record.id).execute(&state.db).await?;
    } else {
        return Err(AppError::BadRequest("No valid Email OTP requested".into()));
    }
    
    // Verify Phone OTP
    let stored_phone = sqlx::query!("SELECT id, otp_code, expires_at FROM admin_action_otps WHERE user_id = $1 AND action_type = 'VERIFY_PHONE' ORDER BY created_at DESC LIMIT 1", user.id).fetch_optional(&state.db).await?;
    if let Some(record) = stored_phone {
        if record.expires_at < time::OffsetDateTime::now_utc() { return Err(AppError::BadRequest("Phone OTP expired".into())); }
        if record.otp_code != payload.phone_otp { return Err(AppError::BadRequest("Invalid WhatsApp OTP".into())); }
        sqlx::query!("DELETE FROM admin_action_otps WHERE id = $1", record.id).execute(&state.db).await?;
    } else {
        return Err(AppError::BadRequest("No valid WhatsApp OTP requested".into()));
    }

    sqlx::query!("UPDATE users SET email_verified = true, phone_verified = true, status = 'ACTIVE' WHERE id = $1", user.id).execute(&state.db).await?;

    if let Ok(Some(updated_user)) = sqlx::query_as!(
        crate::modules::auth::backend::User,
        r#"SELECT u.id, u.school_id, u.username, u.password_hash, u.display_name, u.roles::TEXT[] as "roles!", u.email_verified, u.phone_verified, s.status::TEXT as "school_status!"
           FROM users u JOIN schools s ON u.school_id = s.id WHERE u.id = $1"#,
        user.id
    ).fetch_optional(&state.db).await {
        let mut session = auth_session;
        session.login(&updated_user).await.map_err(|_| AppError::BadRequest("Failed to update session".into()))?;
    }

    Ok((axum::http::StatusCode::OK, axum::Json(serde_json::json!({ "message": "Account verified" }))))
}

#[derive(Debug, serde::Deserialize)]
pub struct RequestPasswordResetPayload {
    pub email: String,
}

#[tracing::instrument(err, skip(state))]
pub async fn request_password_reset(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::Json(payload): axum::Json<RequestPasswordResetPayload>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    let email = payload.email.trim().to_lowercase();
    
    let user = sqlx::query!("SELECT id, username FROM users WHERE LOWER(email) = LOWER($1) AND status = 'ACTIVE' LIMIT 1", email)
        .fetch_optional(&state.db)
        .await?;

    if let Some(user_record) = user {
        use argon2::password_hash::rand_core::{OsRng, RngCore};
        let code = format!("{:06}", OsRng.next_u32() % 1_000_000);
        let token = uuid::Uuid::new_v4().to_string();
        let token_hash = crate::utils::password::hash_password(&token).unwrap_or(token.clone());
        let expires_at = time::OffsetDateTime::now_utc() + time::Duration::minutes(15);

        sqlx::query!(
            "INSERT INTO password_resets (user_id, email, token_hash, code, expires_at) VALUES ($1, $2, $3, $4, $5)",
            user_record.id,
            email,
            token_hash,
            code,
            expires_at
        )
        .execute(&state.db)
        .await?;

        let reset_url = format!("http://localhost:3000/reset-password?token={}&email={}", token, email);
        let email_body = format!(
            "<h3>Eravaya Password Reset Request</h3>\
             <p>Hello {},</p>\
             <p>Your verification code is: <strong>{}</strong></p>\
             <p>Or click this link to reset your password: <a href=\"{}\">Reset Password</a></p>\
             <p>This code will expire in 15 minutes.</p>",
            user_record.username, code, reset_url
        );

        let email_to_send = email.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::utils::email::send_email(&email_to_send, "Reset Your Eravaya Password", &email_body).await {
                tracing::warn!("Failed to send reset email: {}", e);
            }
        });

        tracing::info!("================ PASSWORD RESET CREATED ================");
        tracing::info!("Target Email: {}", email);
        tracing::info!("Verification Code: {}", code);
        tracing::info!("Signed Reset URL: {}", reset_url);
        tracing::info!("========================================================");
    }

    Ok((
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({
            "message": "If an active account exists with that email, a password reset link and verification code have been sent."
        }))
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct ConfirmPasswordResetPayload {
    pub email: String,
    pub code: String,
    pub token: Option<String>,
    pub new_password: String,
}

#[tracing::instrument(err, skip(state))]
pub async fn confirm_password_reset(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::Json(payload): axum::Json<ConfirmPasswordResetPayload>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    if payload.new_password.len() < 8 {
        return Err(AppError::BadRequest("New password must be at least 8 characters long".into()));
    }

    let email = payload.email.trim().to_lowercase();
    let code = payload.code.trim();

    let record = sqlx::query!(
        "SELECT id, user_id, expires_at FROM password_resets \
         WHERE LOWER(email) = LOWER($1) AND code = $2 AND used_at IS NULL \
         ORDER BY created_at DESC LIMIT 1",
        email,
        code
    )
    .fetch_optional(&state.db)
    .await?;

    let reset_record = match record {
        Some(r) => r,
        None => return Err(AppError::BadRequest("Invalid verification code or email".into())),
    };

    if reset_record.expires_at < time::OffsetDateTime::now_utc() {
        return Err(AppError::BadRequest("Password reset code has expired. Please request a new one.".into()));
    }

    let new_password_hash = hash_password(&payload.new_password)?;

    let mut tx = state.db.begin().await?;

    sqlx::query!("UPDATE users SET password_hash = $1, updated_at = now() WHERE id = $2", new_password_hash, reset_record.user_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query!("UPDATE password_resets SET used_at = now() WHERE id = $1", reset_record.id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    crate::utils::audit::log_action(
        &state.db,
        None,
        Some(reset_record.user_id),
        "PASSWORD_RESET_SUCCESS",
        None,
        None,
        None,
        None,
    ).await.ok();

    Ok((
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({
            "message": "Password successfully reset. You can now log in with your new password."
        }))
    ))
}
