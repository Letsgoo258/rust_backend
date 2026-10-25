use axum::{
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    error::AppError,
    modules::{auth::backend::AuthSession, whatsapp::repository::WhatsAppRepository},
    state::AppState,
    utils::{
        capabilities::CapabilityController, password::hash_password, roles::assign_user_role,
        whatsapp::send_whatsapp_message,
    },
};

#[derive(Debug, Deserialize, Default)]
pub struct GenerateLinkTokenPayload {
    pub device_info: Option<serde_json::Value>,
}

/// POST /api/v1/whatsapp/auth/generate-link-token
/// Generates an anti-spoofing link token for the authenticated ERP user.
pub async fn generate_link_token(
    auth_session: AuthSession,
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    payload_opt: Option<Json<GenerateLinkTokenPayload>>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session.user.ok_or_else(|| {
        AppError::Unauthorized("Please log in to ERAVAYA ERP to link WhatsApp".into())
    })?;

    // Extract client IP (prioritize X-Forwarded-For or X-Real-IP)
    let ip_str = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| addr.ip().to_string());

    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Unknown");

    let device_info = payload_opt
        .and_then(|p| p.0.device_info)
        .unwrap_or_else(|| serde_json::json!({ "user_agent": user_agent, "ip": ip_str }));

    // Generate random 6-digit link token formatted as ERA-<digits>
    let random_digits: u32 = {
        let mut rng = rand::thread_rng();
        rng.gen_range(100_000..=999_999)
    };
    let token = format!("ERA-{random_digits}");

    let primary_role = user
        .roles
        .first()
        .cloned()
        .unwrap_or_else(|| "STUDENT".into());

    WhatsAppRepository::create_link_token(
        &state.db,
        &token,
        user.id,
        user.school_id,
        &primary_role,
        &ip_str,
        Some(user_agent),
        device_info,
        15, // 15 minutes expiration
    )
    .await?;

    let bot_phone =
        std::env::var("WHATSAPP_BOT_PHONE").unwrap_or_else(|_| "919605449544".to_string());

    let display_name = user
        .display_name
        .clone()
        .unwrap_or_else(|| user.username.clone());

    tracing::info!(
        user_id = %user.id,
        token = %token,
        ip = %ip_str,
        role = %primary_role,
        "Generated WhatsApp ERP link token"
    );

    Ok(Json(json!({
        "success": true,
        "token": token,
        "bot_phone": bot_phone,
        "user_name": display_name,
        "role": primary_role,
        "expires_in_secs": 900
    })))
}

#[derive(Debug, Deserialize)]
pub struct VerifyTokenQuery {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct SendOtpPayload {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct CompleteVerificationPayload {
    pub token: String,
    pub otp: String,
    pub mode: String, // "link_existing" | "register_new"
    pub school_id: Option<Uuid>,
    pub role: Option<String>, // "PARENT" | "STUDENT"
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub admission_number: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SchoolOption {
    pub id: Uuid,
    pub name: String,
    pub code: String,
}

/// Verify token and fetch metadata (whether user exists, active schools).
pub async fn verify_token(
    State(state): State<AppState>,
    Query(query): Query<VerifyTokenQuery>,
) -> Result<impl IntoResponse, AppError> {
    let token_row = WhatsAppRepository::get_verification_token(&state.db, &query.token)
        .await?
        .ok_or_else(|| AppError::BadRequest("Invalid or expired verification token".into()))?;

    let existing_user = WhatsAppRepository::find_user_by_phone(&state.db, &token_row.phone).await?;

    let schools = sqlx::query_as::<_, (Uuid, String, String)>(
        r#"
        SELECT id, name, code
        FROM schools
        WHERE status = 'ACTIVE'
        ORDER BY name ASC
        LIMIT 50
        "#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(id, name, code)| SchoolOption { id, name, code })
    .collect::<Vec<_>>();

    let existing_user_json = existing_user.map(|(id, school_id, username, display_name, roles)| {
        json!({
            "id": id,
            "school_id": school_id,
            "username": username,
            "display_name": display_name,
            "roles": roles,
        })
    });

    Ok(Json(json!({
        "valid": true,
        "phone": token_row.phone,
        "has_existing_user": existing_user_json.is_some(),
        "existing_user": existing_user_json,
        "schools": schools,
    })))
}

/// Send 6-digit verification OTP directly to the user's WhatsApp chat.
pub async fn send_otp(
    State(state): State<AppState>,
    Json(payload): Json<SendOtpPayload>,
) -> Result<impl IntoResponse, AppError> {
    let token_row = WhatsAppRepository::get_verification_token(&state.db, &payload.token)
        .await?
        .ok_or_else(|| AppError::BadRequest("Invalid or expired verification token".into()))?;

    let otp: u32 = {
        let mut rng = rand::thread_rng();
        rng.gen_range(100_000..=999_999)
    };
    let otp_str = otp.to_string();

    WhatsAppRepository::set_verification_otp(&state.db, &payload.token, &otp_str).await?;

    // Send OTP directly to WhatsApp chat
    let message = format!(
        "🔐 *ERAVAYA ERP Verification Code*\n\nYour one-time code is: *{otp_str}*\n\nThis code expires in 10 minutes. Please enter it on the verification page to complete linking your account."
    );

    let _ = send_whatsapp_message(&state.db, &token_row.sender_jid, &message, None).await;
    tracing::warn!(phone = %token_row.phone, otp = %otp_str, "Generated WhatsApp verification OTP");

    Ok(Json(json!({
        "success": true,
        "message": "OTP has been sent to your WhatsApp number",
    })))
}

/// Complete verification: link existing account or register new student/parent.
pub async fn complete_verification(
    State(state): State<AppState>,
    Json(payload): Json<CompleteVerificationPayload>,
) -> Result<impl IntoResponse, AppError> {
    let token_row = WhatsAppRepository::get_verification_token(&state.db, &payload.token)
        .await?
        .ok_or_else(|| AppError::BadRequest("Invalid or expired verification token".into()))?;

    let expected_otp = token_row.otp_code.as_deref().ok_or_else(|| {
        AppError::BadRequest("No OTP requested. Please request an OTP first.".into())
    })?;

    if expected_otp != payload.otp.trim() {
        return Err(AppError::BadRequest(
            "Invalid OTP code. Please check and try again.".into(),
        ));
    }

    if let Some(expires_at) = token_row.otp_expires_at {
        if expires_at <= Utc::now() {
            return Err(AppError::BadRequest(
                "OTP code has expired. Please request a new one.".into(),
            ));
        }
    }

    let phone_override = payload
        .phone
        .as_deref()
        .map(|p| p.chars().filter(|c| c.is_ascii_digit()).collect::<String>())
        .filter(|p| !p.is_empty());
    let effective_phone = phone_override.as_deref().unwrap_or(&token_row.phone);

    let (user_id, school_id, primary_role, display_name) = if payload.mode == "link_existing" {
        let existing = WhatsAppRepository::find_user_by_phone(&state.db, effective_phone)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "No existing account found for mobile number '{effective_phone}'."
                ))
            })?;

        let user_id = existing.0;
        let school_id = existing.1;
        let display_name = existing.3;
        let primary_role = existing
            .4
            .first()
            .cloned()
            .unwrap_or_else(|| "STUDENT".into());

        (user_id, school_id, primary_role, display_name)
    } else {
        // Mode is "register_new"
        let school_id = payload.school_id.ok_or_else(|| {
            AppError::BadRequest("School selection is required for registration.".into())
        })?;

        let role_input = payload.role.as_deref().unwrap_or("STUDENT").to_uppercase();

        let chosen_role = if role_input == "PARENT" {
            "PARENT"
        } else {
            "STUDENT"
        };

        let first_name = payload
            .first_name
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AppError::BadRequest("First name is required.".into()))?;

        let last_name = payload
            .last_name
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("");

        let display_name = if last_name.is_empty() {
            first_name.to_string()
        } else {
            format!("{first_name} {last_name}")
        };

        // Random secure password for the user record
        let temp_pass = uuid::Uuid::new_v4().to_string();
        let pass_hash = hash_password(&temp_pass)?;
        let username = format!("{}_{}", chosen_role.to_lowercase(), effective_phone);

        let user_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO users (
                school_id, first_name, last_name, display_name, username, phone,
                email, password_hash, status, phone_verified, admission_number
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'ACTIVE', true, $9)
            RETURNING id
            "#,
        )
        .bind(school_id)
        .bind(first_name)
        .bind(last_name)
        .bind(&display_name)
        .bind(&username)
        .bind(effective_phone)
        .bind(payload.email.as_deref())
        .bind(&pass_hash)
        .bind(payload.admission_number.as_deref())
        .fetch_one(&state.db)
        .await?;

        // Assign role in user_roles table
        let mut conn = state.db.acquire().await?;
        assign_user_role(&mut *conn, school_id, user_id, chosen_role).await?;

        (user_id, school_id, chosen_role.to_string(), display_name)
    };

    // Resolve user capabilities
    let caps = CapabilityController::resolve_user_capabilities(
        &state.db,
        school_id,
        Some(user_id),
        &[primary_role.clone()],
    )
    .await;

    // Create or renew active session and populate whatsapp_capabilities table
    let session_id = WhatsAppRepository::create_or_renew_session(
        &state.db,
        user_id,
        school_id,
        &token_row.sender_jid,
        effective_phone,
        &primary_role,
        &caps,
        7, // 7 days sliding session
    )
    .await?;

    // Mark verification token as completed
    let _ = WhatsAppRepository::mark_token_verified(&state.db, &payload.token).await;

    // Send confirmation message to WhatsApp
    let welcome_msg = format!(
        "🎉 *Account Successfully Linked!*\n\n\
        Welcome, *{display_name}*!\n\
        Your WhatsApp is now authenticated with ERAVAYA ERP as a *{primary_role}*.\n\n\
        You can now check attendance, fees, timetable, and exam updates directly here anytime.\n\
        Type *help* to view your interactive menu!"
    );
    let _ = send_whatsapp_message(&state.db, &token_row.sender_jid, &welcome_msg, None).await;

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "session_id": session_id,
            "role": primary_role,
            "display_name": display_name,
            "message": "WhatsApp session verified and linked successfully!",
        })),
    ))
}
