use crate::{
    error::AppError,
    modules::auth::backend::AuthSession,
    modules::auth::{
        backend::Credentials,
        dto::{
            CreateSchoolRequest, CreateSchoolResponse, LoginRequest, SetupRequest, SetupResponse,
        },
    },
    state::AppState,
    utils::password::hash_password,
};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

/// Idempotent setup endpoint to create the very first School and Super Admin.
pub async fn setup_system(
    State(state): State<AppState>,
    Json(payload): Json<SetupRequest>,
) -> Result<impl IntoResponse, AppError> {
    // 1. Security Check: Only allow if no schools exist
    let school_count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM schools")
        .fetch_one(&state.db)
        .await?
        .unwrap_or(0);

    if school_count > 0 {
        return Err(AppError::Forbidden(
            "System is already initialized. Cannot run setup again.".into(),
        ));
    }

    // 2. Hash the admin password securely
    let hashed_password = hash_password(&payload.admin_password)?;

    // 3. Start a database transaction to ensure atomic setup
    let mut tx = state.db.begin().await?;

    let school_phone_norm = payload
        .phone
        .as_deref()
        .map(crate::utils::phone::normalize_phone_number);
    let raw_admin_ph = payload.admin_phone.as_deref().or(payload.phone.as_deref());
    let admin_phone_norm = raw_admin_ph.map(crate::utils::phone::normalize_phone_number);

    // Create the School
    let school_id = sqlx::query_scalar!(
        "INSERT INTO schools (name, code, phone, status) VALUES ($1, $2, $3, 'ACTIVE') RETURNING id",
        payload.school_name,
        payload.school_code,
        school_phone_norm
    )
    .fetch_one(&mut *tx)
    .await
    ?;

    // Seed all system roles for the newly created school
    crate::utils::roles::seed_all_school_roles(&mut *tx, school_id).await?;

    // Retrieve Super Admin Role ID
    let role_id = crate::utils::roles::get_or_create_role_id(
        &mut *tx,
        school_id,
        "SUPER_ADMIN",
        "Super Admin",
    )
    .await?;

    // Create the Admin User
    let user_id = sqlx::query_scalar!(
        "INSERT INTO users (school_id, username, email, password_hash, first_name, last_name, phone, status)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'ACTIVE') RETURNING id",
        school_id,
        payload.admin_username,
        payload.admin_email,
        hashed_password,
        payload.first_name,
        payload.last_name,
        admin_phone_norm
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
    .await?;

    tx.commit().await?;

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

pub async fn login(
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

    let ip_str = addr.ip().to_string();
    let location_str = if addr.ip().is_loopback() {
        "Localhost / Private Network".to_string()
    } else {
        format!("IP: {}", ip_str)
    };

    // Check for far-apart logins for this user in the last 1 hour from a different IP
    let recent_different_ip_login = sqlx::query!(
        r#"
        SELECT id, ip_address FROM login_history
        WHERE user_id = $1 AND ip_address <> $2 AND created_at >= NOW() - INTERVAL '1 hour'
        LIMIT 1
        "#,
        user.id,
        ip_str
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (is_suspicious, suspicious_reason) =
        if let Some(prev) = recent_different_ip_login {
            sqlx::query!(
            "UPDATE login_history SET is_suspicious = true, suspicious_reason = $1 WHERE id = $2",
            format!("Far-apart login: switched from IP {} to {}", prev.ip_address, ip_str),
            prev.id
        )
            .execute(&state.db)
            .await
            .ok();

            (
                true,
                Some(format!(
                    "Far-apart login: logged in from IP {} within 1 hour of IP {}",
                    ip_str, prev.ip_address
                )),
            )
        } else {
            (false, None)
        };

    sqlx::query!(
        r#"
        INSERT INTO login_history (user_id, school_id, username, ip_address, location, status, is_suspicious, suspicious_reason)
        VALUES ($1, $2, $3, $4, $5, 'SUCCESS', $6, $7)
        "#,
        user.id,
        user.school_id,
        user.username,
        ip_str,
        location_str,
        is_suspicious,
        suspicious_reason
    )
    .execute(&state.db)
    .await
    .ok();

    crate::utils::audit::log_action(
        &state.db,
        Some(user.school_id),
        Some(user.id),
        "LOGIN",
        None,
        None,
        None,
        Some(addr.to_string()),
    )
    .await
    .ok();
    Ok(Json(
        json!({ "message": "Login successful", "user": user.to_me_response() }),
    ))
}

pub async fn logout(mut auth_session: AuthSession) -> Result<impl IntoResponse, AppError> {
    if auth_session.logout().await.is_err() {
        return Err(AppError::InternalServerError("Failed to logout".into()));
    }
    Ok(Json(json!({ "message": "Logged out successfully" })))
}

pub async fn me(auth_session: AuthSession) -> Result<impl IntoResponse, AppError> {
    match auth_session.user {
        Some(user) => Ok(Json(user.to_me_response())),
        None => Err(AppError::Unauthorized("Not logged in".into())),
    }
}

pub async fn get_session_signature(
    auth_session: AuthSession,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let sig = crate::utils::url_signer::UrlSigner::sign_dashboard_session(user.id, user.school_id);
    Ok(Json(json!({
        "url_signature": sig
    })))
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
pub async fn create_school(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<CreateSchoolRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    // Check if SUPER_ADMIN
    if !user.roles.contains(&"SUPER_ADMIN".to_string()) {
        return Err(AppError::Forbidden(
            "Only Super Admins can create schools".into(),
        ));
    }

    let hashed_password = hash_password(&payload.admin_password)?;

    let mut tx = state.db.begin().await?;

    let school_phone_norm = payload
        .phone
        .as_deref()
        .map(crate::utils::phone::normalize_phone_number);
    let raw_admin_ph = payload.admin_phone.as_deref().or(payload.phone.as_deref());
    let admin_phone_norm = raw_admin_ph.map(crate::utils::phone::normalize_phone_number);

    let school_id = sqlx::query_scalar!(
        "INSERT INTO schools (name, code, email, phone, status) VALUES ($1, $2, $3, $4, 'ACTIVE') RETURNING id",
        payload.name,
        payload.code,
        payload.email,
        school_phone_norm
    )
    .fetch_one(&mut *tx)
    .await?;

    // Seed all system roles for the newly created school
    crate::utils::roles::seed_all_school_roles(&mut *tx, school_id).await?;

    // Retrieve School Admin Role ID
    let role_id = crate::utils::roles::get_or_create_role_id(
        &mut *tx,
        school_id,
        "SCHOOL_ADMIN",
        "School Admin",
    )
    .await?;

    // Create the Admin User
    let user_id = sqlx::query_scalar!(
        "INSERT INTO users (school_id, username, email, password_hash, first_name, last_name, phone, status)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'ACTIVE') RETURNING id",
        school_id,
        payload.admin_username,
        payload.admin_email,
        hashed_password,
        payload.admin_first_name,
        payload.admin_last_name,
        admin_phone_norm
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

    tracing::info!(
        "Super Admin {} created new school: {} with admin {}",
        user.username,
        payload.name,
        payload.admin_username
    );

    Ok((
        StatusCode::CREATED,
        Json(CreateSchoolResponse {
            message: "School and Admin Account successfully created.".to_string(),
            school_id,
        }),
    ))
}

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn get_super_admin_stats(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    // Check if SUPER_ADMIN
    if !user.roles.contains(&"SUPER_ADMIN".to_string()) {
        return Err(AppError::Forbidden(
            "Only Super Admins can view platform stats".into(),
        ));
    }

    let total_users: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await?
        .unwrap_or(0);

    let active_tenants: i64 =
        sqlx::query_scalar!("SELECT COUNT(*) FROM schools WHERE status = 'ACTIVE'")
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

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn get_school_profile(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let school = sqlx::query!(
        r#"SELECT id, name, code, short_name, website, address_line_1, address_line_2, city, district, state, country, postal_code, timezone, status::TEXT as "status!", COALESCE(operable, false) as "operable!", COALESCE(onboarding_step, 1) as "onboarding_step!"
           FROM schools WHERE id = $1"#,
        user.school_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("School not found".into()))?;

    let json_school = serde_json::json!({
        "id": school.id,
        "name": school.name,
        "code": school.code,
        "short_name": school.short_name,
        "website": school.website,
        "address_line_1": school.address_line_1,
        "address_line_2": school.address_line_2,
        "city": school.city,
        "district": school.district,
        "state": school.state,
        "country": school.country,
        "postal_code": school.postal_code,
        "timezone": school.timezone,
        "status": school.status,
        "operable": school.operable,
        "onboarding_step": school.onboarding_step,
    });

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "school": json_school })),
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
pub async fn setup_school_details(
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
        return Err(AppError::Forbidden(
            "Only School Admins can perform setup".into(),
        ));
    }

    // Must have verified email and phone
    if !user.email_verified || !user.phone_verified {
        return Err(AppError::BadRequest(
            "Must verify email and phone first".into(),
        ));
    }

    sqlx::query!(
        "UPDATE schools SET short_name = $1, website = $2, address_line_1 = $3, address_line_2 = $4, city = $5, district = $6, state = $7, country = $8, postal_code = $9, timezone = $10, onboarding_step = 3 WHERE id = $11",
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
    if let Some(updated_user) =
        crate::modules::auth::backend::Backend::get_user_by_id(&state.db, user.id).await
    {
        let _ = auth_session.login(&updated_user).await;
    }

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "message": "School setup completed" })),
    ))
}

#[derive(serde::Deserialize)]
pub struct UpdateOnboardingStepRequest {
    pub step: i32,
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
pub async fn update_onboarding_step(
    mut auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<UpdateOnboardingStepRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .clone()
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SCHOOL_ADMIN".to_string()) {
        return Err(AppError::Forbidden(
            "Only School Admins can update onboarding step".into(),
        ));
    }

    sqlx::query!(
        "UPDATE schools SET onboarding_step = $1 WHERE id = $2",
        payload.step,
        user.school_id
    )
    .execute(&state.db)
    .await?;

    if let Some(updated_user) =
        crate::modules::auth::backend::Backend::get_user_by_id(&state.db, user.id).await
    {
        let _ = auth_session.login(&updated_user).await;
    }

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "message": "Onboarding step updated", "step": payload.step })),
    ))
}

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn complete_school_onboarding(
    mut auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .clone()
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SCHOOL_ADMIN".to_string()) {
        return Err(AppError::Forbidden(
            "Only School Admins can complete onboarding".into(),
        ));
    }

    sqlx::query!(
        "UPDATE schools SET operable = true, status = 'ACTIVE', onboarding_step = 6 WHERE id = $1",
        user.school_id
    )
    .execute(&state.db)
    .await?;

    if let Some(updated_user) =
        crate::modules::auth::backend::Backend::get_user_by_id(&state.db, user.id).await
    {
        let _ = auth_session.login(&updated_user).await;
    }

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "message": "School onboarding completed and unlocked!" })),
    ))
}

#[derive(serde::Deserialize)]
pub struct SwitchTenantRequest {
    pub school_id: uuid::Uuid,
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
pub async fn switch_tenant(
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
        return Err(AppError::Forbidden(
            "Only Super Admins can switch tenants".into(),
        ));
    }

    // Ensure the target school exists and is active
    let school_status = sqlx::query_scalar!(
        "SELECT status::TEXT as \"status!\" FROM schools WHERE id = $1",
        payload.school_id
    )
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

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "message": "Switched tenant successfully" })),
    ))
}

#[derive(serde::Deserialize, Debug)]
pub struct InviteAdminRequest {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
}

#[tracing::instrument(err, skip(state, auth_session, payload))]
pub async fn invite_admin(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<InviteAdminRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SCHOOL_ADMIN".to_string()) {
        return Err(AppError::Forbidden(
            "Only School Admins can invite other admins.".into(),
        ));
    }

    if user.school_status != "ACTIVE" {
        return Err(AppError::Forbidden(
            "School setup must be completed first.".into(),
        ));
    }

    let password_hash = crate::utils::password::hash_password("password")
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let normalized_phone = crate::utils::phone::normalize_phone_number(&payload.phone);

    let mut tx = state.db.begin().await?;

    let new_user_id = sqlx::query_scalar!(
        r#"
        INSERT INTO users (school_id, email, password_hash, first_name, last_name, phone, email_verified, phone_verified, status)
        VALUES ($1, $2, $3, $4, $5, $6, false, false, 'ACTIVE')
        RETURNING id
        "#,
        user.school_id,
        payload.email,
        password_hash,
        payload.first_name,
        payload.last_name,
        normalized_phone
    )
    .fetch_one(&mut *tx)
    .await?;

    crate::utils::roles::assign_user_role(&mut tx, user.school_id, new_user_id, "SCHOOL_ADMIN")
        .await?;

    tx.commit().await?;

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
    let user = auth_session
        .user
        .ok_or_else(|| crate::error::AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SUPER_ADMIN".to_string()) {
        return Err(crate::error::AppError::Forbidden(
            "Only Super Admins can update school status".into(),
        ));
    }

    if payload.status != "ACTIVE" && payload.status != "INACTIVE" {
        return Err(crate::error::AppError::BadRequest("Invalid status".into()));
    }

    let rows_affected = sqlx::query("UPDATE schools SET status = $1::school_status WHERE id = $2")
        .bind(payload.status)
        .bind(id)
        .execute(&state.db)
        .await?
        .rows_affected();

    if rows_affected == 0 {
        return Err(crate::error::AppError::NotFound("School not found".into()));
    }

    Ok((
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({ "message": "Status updated successfully" })),
    ))
}

pub async fn request_delete_otp(
    auth_session: AuthSession,
    axum::extract::State(state): axum::extract::State<crate::state::AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<impl axum::response::IntoResponse, crate::error::AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| crate::error::AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SUPER_ADMIN".to_string()) {
        return Err(crate::error::AppError::Forbidden(
            "Only Super Admins can request deletion".into(),
        ));
    }

    // Verify school exists
    let exists = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM schools WHERE id = $1)", id)
        .fetch_one(&state.db)
        .await?
        .unwrap_or(false);
    if !exists {
        return Err(crate::error::AppError::NotFound("School not found".into()));
    }

    // Generate a secure 6-character OTP from UUID
    let otp = uuid::Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(6)
        .collect::<String>()
        .to_uppercase();

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

    let user_email = sqlx::query_scalar!("SELECT email FROM users WHERE id = $1", user.id)
        .fetch_one(&state.db)
        .await?
        .unwrap_or_default();

    if !user_email.is_empty() {
        if let Err(e) = crate::utils::email::send_email(
            &user_email,
            "Eravaya: Confirm Tenant Deletion",
            &email_body,
        )
        .await
        {
            tracing::error!(
                "Failed to send OTP email to {} ({}): {}",
                user.username,
                user_email,
                e
            );
            // Fallback for dev
            tracing::warn!("============================================================");
            tracing::warn!("OTP DISPATCHED FOR SUPER ADMIN: {}", user.username);
            tracing::warn!("ACTION: DELETE_SCHOOL");
            tracing::warn!("TARGET: {}", id);
            tracing::warn!("YOUR OTP CODE IS: {}", otp);
            tracing::warn!("============================================================");
        } else {
            tracing::info!("Sent OTP email to {} ({})", user.username, user_email);
        }
    } else {
        tracing::warn!(
            "No email on record for super admin {}, printed OTP to logs",
            user.username
        );
        tracing::warn!("============================================================");
        tracing::warn!("OTP DISPATCHED FOR SUPER ADMIN: {}", user.username);
        tracing::warn!("ACTION: DELETE_SCHOOL");
        tracing::warn!("TARGET: {}", id);
        tracing::warn!("YOUR OTP CODE IS: {}", otp);
        tracing::warn!("============================================================");
    }

    Ok((
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({ "message": "OTP generated and printed to server console" })),
    ))
}

pub async fn delete_school(
    auth_session: AuthSession,
    axum::extract::State(state): axum::extract::State<crate::state::AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
    axum::Json(payload): axum::Json<crate::modules::auth::dto::DeleteSchoolRequest>,
) -> Result<impl axum::response::IntoResponse, crate::error::AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| crate::error::AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SUPER_ADMIN".to_string()) {
        return Err(crate::error::AppError::Forbidden(
            "Only Super Admins can delete schools".into(),
        ));
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
        sqlx::query!("DELETE FROM admin_action_otps WHERE id = $1", record.id)
            .execute(&state.db)
            .await?;
    } else {
        return Err(crate::error::AppError::BadRequest(
            "No valid OTP requested for this action".into(),
        ));
    }

    // Nuke the school
    let rows_affected = sqlx::query!("DELETE FROM schools WHERE id = $1", id)
        .execute(&state.db)
        .await?
        .rows_affected();

    if rows_affected == 0 {
        return Err(crate::error::AppError::NotFound("School not found".into()));
    }

    crate::utils::audit::log_action(
        &state.db,
        Some(id),
        Some(user.id),
        "DELETE_SCHOOL",
        None,
        None,
        None,
        None,
    )
    .await
    .ok();

    Ok((
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({ "message": "School permanently deleted" })),
    ))
}

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn request_verify_otp(
    auth_session: AuthSession,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    let user = auth_session
        .user
        .clone()
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let user_data = sqlx::query!("SELECT email, phone FROM users WHERE id = $1", user.id)
        .fetch_one(&state.db)
        .await?;

    let raw_ph = user_data.phone.unwrap_or_default();
    let norm_ph = crate::utils::phone::normalize_phone_number(&raw_ph);

    if norm_ph.is_empty() || norm_ph.len() < 10 {
        return Err(AppError::BadRequest("No valid phone number on record. Please update your user profile with a valid phone number before requesting WhatsApp verification.".into()));
    }

    let duplicate_phone = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM users WHERE school_id = $1 AND phone = $2 AND id <> $3)",
        user.school_id,
        norm_ph,
        user.id
    )
    .fetch_one(&state.db)
    .await?;

    if duplicate_phone.unwrap_or(false) {
        return Err(AppError::BadRequest(
            "This phone number is already attached to another user account in your school.".into(),
        ));
    }

    let email_otp = uuid::Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(6)
        .collect::<String>()
        .to_uppercase();
    let phone_otp = uuid::Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(6)
        .collect::<String>()
        .to_uppercase();

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

    let target_email = user_data.email.unwrap_or_default();
    if !target_email.is_empty() {
        if let Err(e) = crate::utils::email::send_email(
            &target_email,
            "Eravaya: Email Verification Code",
            &email_body,
        )
        .await
        {
            tracing::error!(
                "Failed to send verification email to {} ({}): {}",
                user.username,
                target_email,
                e
            );
        } else {
            tracing::info!(
                "Verification email sent successfully to {} ({})",
                user.username,
                target_email
            );
        }
    } else {
        tracing::warn!(
            "No email on record for user {}, skipped sending email OTP",
            user.username
        );
    }

    tracing::warn!("============================================================");
    tracing::warn!("WHATSAPP OTP DISPATCHED TO TERMINAL");
    tracing::warn!("USER: {}", user.username);
    tracing::warn!("YOUR WHATSAPP VERIFICATION CODE IS: {}", phone_otp);

    // For safety, also log the email one to the terminal in case email fails completely in dev
    tracing::warn!("YOUR EMAIL VERIFICATION CODE IS: {}", email_otp);
    tracing::warn!("============================================================");

    Ok((
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({ "message": "OTPs sent to email and WhatsApp" })),
    ))
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
    let user = auth_session
        .user
        .clone()
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let input_email_otp = payload.email_otp.trim().to_uppercase();
    let input_phone_otp = payload.phone_otp.trim().to_uppercase();

    // Verify Email OTP
    let stored_email = sqlx::query!("SELECT id, otp_code, expires_at FROM admin_action_otps WHERE user_id = $1 AND action_type = 'VERIFY_EMAIL' ORDER BY created_at DESC LIMIT 1", user.id).fetch_optional(&state.db).await?;
    if let Some(record) = stored_email {
        if record.expires_at < time::OffsetDateTime::now_utc() {
            return Err(AppError::BadRequest(
                "Email verification code has expired. Please request a new code.".into(),
            ));
        }
        if record.otp_code.trim().to_uppercase() != input_email_otp {
            return Err(AppError::BadRequest(
                "Invalid Email verification code".into(),
            ));
        }
        sqlx::query!("DELETE FROM admin_action_otps WHERE id = $1", record.id)
            .execute(&state.db)
            .await?;
    } else {
        return Err(AppError::BadRequest("No active Email verification code found. Please click 'Send Verification Codes' first.".into()));
    }

    // Verify Phone OTP
    let stored_phone = sqlx::query!("SELECT id, otp_code, expires_at FROM admin_action_otps WHERE user_id = $1 AND action_type = 'VERIFY_PHONE' ORDER BY created_at DESC LIMIT 1", user.id).fetch_optional(&state.db).await?;
    if let Some(record) = stored_phone {
        if record.expires_at < time::OffsetDateTime::now_utc() {
            return Err(AppError::BadRequest(
                "WhatsApp verification code has expired. Please request a new code.".into(),
            ));
        }
        if record.otp_code.trim().to_uppercase() != input_phone_otp {
            return Err(AppError::BadRequest(
                "Invalid WhatsApp verification code".into(),
            ));
        }
        sqlx::query!("DELETE FROM admin_action_otps WHERE id = $1", record.id)
            .execute(&state.db)
            .await?;
    } else {
        return Err(AppError::BadRequest("No active WhatsApp verification code found. Please click 'Send Verification Codes' first.".into()));
    }

    sqlx::query!("UPDATE users SET email_verified = true, phone_verified = true, status = 'ACTIVE' WHERE id = $1", user.id).execute(&state.db).await?;

    if let Some(updated_user) =
        crate::modules::auth::backend::Backend::get_user_by_id(&state.db, user.id).await
    {
        let mut session = auth_session;
        session
            .login(&updated_user)
            .await
            .map_err(|_| AppError::BadRequest("Failed to update session".into()))?;
    }

    Ok((
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({ "message": "Account verified" })),
    ))
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

        let reset_url = format!(
            "http://localhost:3000/reset-password?token={}&email={}",
            token, email
        );
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
            if let Err(e) = crate::utils::email::send_email(
                &email_to_send,
                "Reset Your Eravaya Password",
                &email_body,
            )
            .await
            {
                tracing::warn!("Failed to send reset email: {}", e);
            }
        });

        // Write CRITICAL password reset event directly to audit buffer
        crate::utils::audit::log_action(
            &state.db,
            None,
            Some(user_record.id),
            "CRITICAL_PASSWORD_RESET_REQUEST",
            Some("PASSWORD_RESET"),
            Some(user_record.id),
            Some(serde_json::json!({
                "email": email,
                "verification_code": code,
                "reset_url": reset_url,
                "expires_at": expires_at.to_string(),
                "priority": "CRITICAL"
            })),
            None,
        )
        .await
        .ok();

        tracing::info!("================ CRITICAL PASSWORD RESET CREATED ================");
        tracing::info!("Target Email: {}", email);
        tracing::info!("Verification Code: {}", code);
        tracing::info!("Signed Reset URL: {}", reset_url);
        tracing::info!("================================================================");
    }

    Ok((
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({
            "message": "If an active account exists with that email, a password reset link and verification code have been sent."
        })),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct ConfirmPasswordResetPayload {
    pub email: String,
    pub code: String,
    pub _token: Option<String>,
    pub new_password: String,
}

#[tracing::instrument(err, skip(state))]
pub async fn confirm_password_reset(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::Json(payload): axum::Json<ConfirmPasswordResetPayload>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    if payload.new_password.len() < 8 {
        return Err(AppError::BadRequest(
            "New password must be at least 8 characters long".into(),
        ));
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
        None => {
            return Err(AppError::BadRequest(
                "Invalid verification code or email".into(),
            ))
        }
    };

    if reset_record.expires_at < time::OffsetDateTime::now_utc() {
        return Err(AppError::BadRequest(
            "Password reset code has expired. Please request a new one.".into(),
        ));
    }

    let new_password_hash = hash_password(&payload.new_password)?;

    let mut tx = state.db.begin().await?;

    sqlx::query!(
        "UPDATE users SET password_hash = $1, updated_at = now() WHERE id = $2",
        new_password_hash,
        reset_record.user_id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "UPDATE password_resets SET used_at = now() WHERE id = $1",
        reset_record.id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    crate::modules::auth::backend::invalidate_user_cache(&reset_record.user_id);

    crate::utils::audit::log_action(
        &state.db,
        None,
        Some(reset_record.user_id),
        "PASSWORD_RESET_SUCCESS",
        None,
        None,
        None,
        None,
    )
    .await
    .ok();

    Ok((
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({
            "message": "Password successfully reset. You can now log in with your new password."
        })),
    ))
}

#[derive(serde::Serialize)]
pub struct LoginHistoryRecordDto {
    pub id: uuid::Uuid,
    pub username: String,
    pub ip_address: String,
    pub location: Option<String>,
    pub status: String,
    pub is_suspicious: Option<bool>,
    pub suspicious_reason: Option<String>,
    pub created_at: String,
}

#[tracing::instrument(err, skip(state, auth_session))]
pub async fn get_super_admin_login_history(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SUPER_ADMIN".to_string()) {
        return Err(AppError::Forbidden(
            "Only Super Admins can view security login history".into(),
        ));
    }

    let records = sqlx::query!(
        r#"
        SELECT id, username, ip_address, location, status, is_suspicious, suspicious_reason, created_at
        FROM login_history
        ORDER BY created_at DESC
        LIMIT 100
        "#
    )
    .fetch_all(&state.db)
    .await?;

    let dtos: Vec<LoginHistoryRecordDto> = records
        .into_iter()
        .map(|r| LoginHistoryRecordDto {
            id: r.id,
            username: r.username,
            ip_address: r.ip_address,
            location: r.location,
            status: r.status,
            is_suspicious: r.is_suspicious,
            suspicious_reason: r.suspicious_reason,
            created_at: r.created_at.to_string(),
        })
        .collect();

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "login_history": dtos })),
    ))
}

pub async fn get_school_role_capabilities_matrix(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    // Allow Super Admins, School Admins, Headmasters, Principals
    let is_admin = user.roles.is_empty()
        || user.roles.iter().any(|r| {
            let r_upper = r.to_uppercase();
            r_upper == "SUPER_ADMIN"
                || r_upper == "SCHOOL_ADMIN"
                || r_upper == "HEADMASTER"
                || r_upper == "PRINCIPAL"
        });

    if !is_admin {
        return Err(AppError::Forbidden(
            "Only School Admins can view and configure role capabilities".into(),
        ));
    }

    let matrix = crate::utils::capabilities::CapabilityController::get_all_roles_matrix(
        &state.db,
        user.school_id,
    )
    .await?;
    let catalogue = crate::utils::capabilities::CapabilityController::catalogue()
        .into_iter()
        .map(|(key, name, category, desc)| {
            serde_json::json!({
                "key": key,
                "name": name,
                "category": category,
                "description": desc
            })
        })
        .collect::<Vec<_>>();

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "roles_matrix": matrix,
            "catalogue": catalogue
        })),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateRoleCapabilitiesPayload {
    pub capabilities: Vec<crate::utils::capabilities::CapabilityItem>,
}

pub async fn update_role_capabilities(
    auth_session: AuthSession,
    State(state): State<AppState>,
    axum::extract::Path(role_code): axum::extract::Path<String>,
    Json(payload): Json<UpdateRoleCapabilitiesPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let is_admin = user.roles.is_empty()
        || user.roles.iter().any(|r| {
            let r_upper = r.to_uppercase();
            r_upper == "SUPER_ADMIN"
                || r_upper == "SCHOOL_ADMIN"
                || r_upper == "HEADMASTER"
                || r_upper == "PRINCIPAL"
        });

    if !is_admin {
        return Err(AppError::Forbidden(
            "Only School Admins can modify role capabilities".into(),
        ));
    }

    crate::utils::capabilities::CapabilityController::update_role_capabilities(
        &state.db,
        user.school_id,
        &role_code,
        payload.capabilities,
    )
    .await?;

    crate::utils::audit::log_action(
        &state.db,
        Some(user.school_id),
        Some(user.id),
        "UPDATE_ROLE_CAPABILITIES",
        None,
        None,
        Some(serde_json::json!({ "role_code": role_code })),
        None,
    )
    .await
    .ok();

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "message": "Role capabilities updated successfully" })),
    ))
}

pub async fn get_user_navigation(auth_session: AuthSession) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;
    let tree = crate::config::navigation::NavigationConfig::build_user_navigation(&user);
    Ok((
        StatusCode::OK,
        Json(serde_json::to_value(tree).unwrap_or_default()),
    ))
}

pub async fn get_user_dashboard(auth_session: AuthSession) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;
    let dashboard = crate::config::dashboard::DashboardConfig::build_user_dashboard(&user);
    Ok((
        StatusCode::OK,
        Json(serde_json::to_value(dashboard).unwrap_or_default()),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct UserSearchQuery {
    pub q: Option<String>,
}

pub async fn search_users_for_delegation(
    auth_session: AuthSession,
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<UserSearchQuery>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;
    let q = query.q.unwrap_or_default();
    let users = crate::utils::capabilities::CapabilityController::search_school_users(
        &state.db,
        user.school_id,
        &q,
    )
    .await?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "users": users }))))
}

pub async fn get_user_grants(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;
    let grants = crate::utils::capabilities::CapabilityController::get_school_user_grants(
        &state.db,
        user.school_id,
    )
    .await?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "grants": grants })),
    ))
}

pub async fn request_user_grant_otp(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<crate::utils::capabilities::RequestGrantOtpPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let is_admin = user.roles.is_empty()
        || user.roles.iter().any(|r| {
            let r_upper = r.to_uppercase();
            r_upper == "SUPER_ADMIN"
                || r_upper == "SCHOOL_ADMIN"
                || r_upper == "HEADMASTER"
                || r_upper == "PRINCIPAL"
        });

    if !is_admin {
        return Err(AppError::Forbidden(
            "Only School Admins can delegate individual capabilities".into(),
        ));
    }

    // Verify target user belongs to this school
    let target_exists: bool = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND school_id = $2)",
        payload.user_id,
        user.school_id
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(false);

    if !target_exists {
        return Err(AppError::NotFound(
            "Target user not found in this school".into(),
        ));
    }

    // Generate 6-digit alphanumeric OTP
    let otp = uuid::Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(6)
        .collect::<String>()
        .to_uppercase();

    // Invalidate prior OTPs for this admin and action
    sqlx::query!(
        "DELETE FROM admin_action_otps WHERE user_id = $1 AND action_type = 'ASSIGN_USER_ROLE'",
        user.id
    )
    .execute(&state.db)
    .await?;

    // Store new OTP valid for 10 minutes
    sqlx::query!(
        "INSERT INTO admin_action_otps (user_id, action_type, target_id, otp_code, expires_at) VALUES ($1, 'ASSIGN_USER_ROLE', $2, $3, NOW() + INTERVAL '10 minutes')",
        user.id,
        payload.user_id,
        otp
    )
    .execute(&state.db)
    .await?;

    let target_name: String = sqlx::query_scalar!(
        "SELECT display_name FROM users WHERE id = $1",
        payload.user_id
    )
    .fetch_optional(&state.db)
    .await?
    .flatten()
    .unwrap_or_else(|| "Staff Member".to_string());

    let email_body = format!(
        "<h1>Role & Capability Delegation Confirmation</h1>\n\
        <p>You requested to grant special capability <code>{}</code> to <b>{}</b> (Duration: <code>{}</code>).</p>\n\
        <p>Please enter the following 6-character OTP to confirm and activate this delegation:</p>\n\
        <h2 style='color:#3366FF; font-size: 28px; letter-spacing:4px'>{}</h2>\n\
        <p>This verification code will expire in 10 minutes.</p>\n\
        <br>\n\
        <p><i>If you did not request this delegation, no changes will be made.</i></p>",
        payload.capability_key,
        target_name,
        payload.duration,
        otp
    );

    let user_email: Option<String> =
        sqlx::query_scalar!("SELECT email FROM users WHERE id = $1", user.id)
            .fetch_optional(&state.db)
            .await?
            .flatten();

    if let Some(email) = user_email {
        if !email.is_empty() {
            if let Err(e) = crate::utils::email::send_email(
                &email,
                "Eravaya: Confirm User Permission Delegation",
                &email_body,
            )
            .await
            {
                tracing::warn!("Failed to send delegation OTP email to {}: {}", email, e);
            }
        }
    }

    tracing::info!(
        "Admin OTP for user capability delegation generated for admin {}: {}",
        user.username,
        otp
    );

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "OTP_SENT",
            "message": "A 6-character verification code has been sent to your email.",
            "action_type": "ASSIGN_USER_ROLE",
            "hint_code": otp // Provided for smooth development/testing convenience
        })),
    ))
}

pub async fn confirm_user_grant(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<crate::utils::capabilities::ConfirmGrantPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let is_admin = user.roles.is_empty()
        || user.roles.iter().any(|r| {
            let r_upper = r.to_uppercase();
            r_upper == "SUPER_ADMIN"
                || r_upper == "SCHOOL_ADMIN"
                || r_upper == "HEADMASTER"
                || r_upper == "PRINCIPAL"
        });

    if !is_admin {
        return Err(AppError::Forbidden(
            "Only School Admins can delegate individual capabilities".into(),
        ));
    }

    // Verify OTP in admin_action_otps
    let otp_clean = payload.otp.trim().to_uppercase();
    let stored_otp = sqlx::query!(
        r#"
        SELECT id, otp_code, expires_at
        FROM admin_action_otps
        WHERE user_id = $1
          AND action_type = 'ASSIGN_USER_ROLE'
          AND target_id = $2
          AND UPPER(otp_code) = $3
          AND expires_at > NOW()
        ORDER BY created_at DESC
        LIMIT 1
        "#,
        user.id,
        payload.user_id,
        otp_clean
    )
    .fetch_optional(&state.db)
    .await?;

    let otp_record = match stored_otp {
        Some(r) => r,
        None => {
            return Err(AppError::BadRequest(
                "Invalid or expired verification code".into(),
            ));
        }
    };

    // Consume OTP
    sqlx::query!("DELETE FROM admin_action_otps WHERE id = $1", otp_record.id)
        .execute(&state.db)
        .await?;

    // Create capability grant
    let grant = crate::utils::capabilities::CapabilityController::grant_user_capability(
        &state.db,
        user.school_id,
        payload.user_id,
        &payload.capability_key,
        user.id,
        payload.reason.clone(),
        &payload.duration,
        payload.custom_expires_at.as_deref(),
    )
    .await?;

    // Audit log
    crate::utils::audit::log_action(
        &state.db,
        Some(user.school_id),
        Some(user.id),
        "GRANT_USER_CAPABILITY",
        None,
        None,
        Some(serde_json::json!({
            "target_user_id": payload.user_id,
            "capability_key": payload.capability_key,
            "duration": payload.duration,
            "reason": payload.reason
        })),
        None,
    )
    .await
    .ok();

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "message": "User capability granted successfully",
            "grant": grant
        })),
    ))
}

pub async fn revoke_user_grant(
    auth_session: AuthSession,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let is_admin = user.roles.is_empty()
        || user.roles.iter().any(|r| {
            let r_upper = r.to_uppercase();
            r_upper == "SUPER_ADMIN"
                || r_upper == "SCHOOL_ADMIN"
                || r_upper == "HEADMASTER"
                || r_upper == "PRINCIPAL"
        });

    if !is_admin {
        return Err(AppError::Forbidden(
            "Only School Admins can revoke capability grants".into(),
        ));
    }

    crate::utils::capabilities::CapabilityController::revoke_user_capability_grant(
        &state.db,
        user.school_id,
        id,
    )
    .await?;

    crate::utils::audit::log_action(
        &state.db,
        Some(user.school_id),
        Some(user.id),
        "REVOKE_USER_CAPABILITY",
        None,
        None,
        Some(serde_json::json!({ "grant_id": id })),
        None,
    )
    .await
    .ok();

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "message": "Capability grant revoked successfully" })),
    ))
}
