use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{Datelike, NaiveDate, Utc};
use qrcode::{render::svg, QrCode};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    error::AppError,
    routes::settings::admissions::{AdmissionConfigPayload, RequiredDocItem},
    state::AppState,
    utils::{datetime::DateUtils, imagekit::ImageKitClient, whatsapp::send_whatsapp_message},
};

#[derive(Debug, Deserialize)]
pub struct ValidateTokenQuery {
    pub token: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UploadDocumentPayload {
    pub admission_id: Uuid,
    pub application_number: String,
    pub image_base64: String,
    pub document_type: Option<String>,
    pub file_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SendOtpPayload {
    pub target: String,
    pub school_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct VerifyOtpPayload {
    pub target: String,
    pub otp: String,
}

#[derive(Debug, Deserialize)]
pub struct OfficerRequestOtpPayload {
    pub admission_id: Uuid,
    pub phone: String,
}

#[derive(Debug, Deserialize)]
pub struct OfficerVerifyOtpPayload {
    pub admission_id: Uuid,
    pub phone: String,
    pub otp: String,
}

#[derive(Debug, Deserialize)]
pub struct OfficerVerifiedDocsQuery {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct PublicAdmissionSubmitRequest {
    pub phone_token: String,
    pub email_token: String,
    pub captcha_token: String,
    pub captcha_answer: String,

    pub applicant_first_name: String,
    pub applicant_middle_name: Option<String>,
    pub applicant_last_name: String,
    pub date_of_birth: String, // YYYY-MM-DD
    pub applicant_gender: Option<String>,
    pub applicant_phone: String,
    pub applicant_email: String,

    pub guardian_name: String,
    pub guardian_relationship: String,
    pub guardian_phone: String,
    pub guardian_email: Option<String>,
    pub guardian_occupation: Option<String>,

    #[serde(default)]
    pub custom_fields: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct CustomFieldOption {
    pub id: Uuid,
    pub field_name: String,
    pub field_key: String,
    pub field_type: String,
    pub is_required: bool,
    pub options_json: Option<serde_json::Value>,
    pub display_order: i32,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/config", get(get_public_admission_config))
        .route("/captcha", get(get_captcha_challenge))
        .route("/validate-token", get(validate_preverified_token))
        .route("/send-phone-otp", post(send_phone_otp))
        .route("/verify-phone-otp", post(verify_phone_otp))
        .route("/send-email-otp", post(send_email_otp))
        .route("/verify-email-otp", post(verify_email_otp))
        .route("/submit", post(submit_admission_application))
        .route("/application/:id", get(get_submitted_application_details))
        .route(
            "/application/:id/verified-documents",
            get(get_officer_verified_documents),
        )
        .route("/officer-request-otp", post(officer_request_otp))
        .route("/officer-verify-otp", post(officer_verify_otp))
        .route("/upload-document", post(upload_admission_document))
        .route("/documents/:file_id", get(serve_local_document))
        .layer(axum::extract::DefaultBodyLimit::max(25 * 1024 * 1024))
}

/// Helper to resolve the active/primary school
async fn resolve_primary_school(pool: &sqlx::PgPool) -> Result<(Uuid, String), AppError> {
    let row = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT id, name
        FROM schools
        WHERE status = 'ACTIVE'
        ORDER BY created_at ASC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("No active school found in system".into()))?;

    Ok(row)
}

/// GET /api/v1/admissions/public/config
/// Public endpoint returning school details, admission window status, academic year, and custom fields.
pub async fn get_public_admission_config(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let (school_id, school_name) = resolve_primary_school(&state.db).await?;

    // 1. Fetch saved admission settings
    let setting_val = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = 'admission_settings'
        "#,
    )
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?;

    let default_docs = vec![
        RequiredDocItem {
            id: "birth_cert".into(),
            label: "Birth Certificate".into(),
            required: true,
        },
        RequiredDocItem {
            id: "prev_report".into(),
            label: "Previous Year Report Card".into(),
            required: true,
        },
        RequiredDocItem {
            id: "transfer_cert".into(),
            label: "Transfer Certificate (TC)".into(),
            required: false,
        },
        RequiredDocItem {
            id: "photo".into(),
            label: "Passport-size Photographs (4 copies)".into(),
            required: true,
        },
        RequiredDocItem {
            id: "address_proof".into(),
            label: "Address Proof (Aadhaar / Utility Bill)".into(),
            required: true,
        },
        RequiredDocItem {
            id: "caste_cert".into(),
            label: "Caste / Category Certificate".into(),
            required: false,
        },
        RequiredDocItem {
            id: "medical_fit".into(),
            label: "Medical Fitness Certificate".into(),
            required: false,
        },
    ];

    let current_year = Utc::now().date_naive().year();
    let default_ay_name = format!("{}-{}", current_year, current_year + 1);

    // Active academic year from database
    let active_ay = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT id, name
        FROM academic_years
        WHERE school_id = $1 AND status = 'ACTIVE'
        ORDER BY start_date DESC
        LIMIT 1
        "#,
    )
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?;

    let config: AdmissionConfigPayload = if let Some(val) = setting_val {
        serde_json::from_value(val).unwrap_or_else(|_| AdmissionConfigPayload {
            academic_year_id: active_ay.as_ref().map(|a| a.0),
            academic_year_name: active_ay
                .as_ref()
                .map(|a| a.1.clone())
                .or(Some(default_ay_name)),
            prefix: "ADM".into(),
            separator: "-".into(),
            start_number: "001".into(),
            year_in_format: true,
            intake_capacity: 500,
            open_date: format!("{}-01-01", current_year),
            close_date: format!("{}-12-31", current_year),
            required_docs: default_docs,
            is_open: Some(true),
        })
    } else {
        AdmissionConfigPayload {
            academic_year_id: active_ay.as_ref().map(|a| a.0),
            academic_year_name: active_ay
                .as_ref()
                .map(|a| a.1.clone())
                .or(Some(default_ay_name)),
            prefix: "ADM".into(),
            separator: "-".into(),
            start_number: "001".into(),
            year_in_format: true,
            intake_capacity: 500,
            open_date: format!("{}-01-01", current_year),
            close_date: format!("{}-12-31", current_year),
            required_docs: default_docs,
            is_open: Some(true),
        }
    };

    // Calculate in-date status
    let today = Utc::now().date_naive();
    let is_in_date = match (
        NaiveDate::parse_from_str(&config.open_date, "%Y-%m-%d"),
        NaiveDate::parse_from_str(&config.close_date, "%Y-%m-%d"),
    ) {
        (Ok(open), Ok(close)) => today >= open && today <= close,
        _ => true,
    };

    // 2. Fetch configured custom fields for ADMISSION or STUDENT
    let custom_fields_rows = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            String,
            bool,
            Option<serde_json::Value>,
            i32,
        ),
    >(
        r#"
        SELECT id, field_name, field_key, field_type, is_required, options_json, display_order
        FROM custom_fields
        WHERE school_id = $1
          AND entity_type IN ('ADMISSION', 'STUDENT', 'ALL', 'USER')
          AND field_key NOT LIKE 'doc_%'
        ORDER BY display_order ASC, field_name ASC
        "#,
    )
    .bind(school_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let custom_fields: Vec<CustomFieldOption> = custom_fields_rows
        .into_iter()
        .map(
            |(id, field_name, field_key, field_type, is_required, options_json, display_order)| {
                CustomFieldOption {
                    id,
                    field_name,
                    field_key,
                    field_type,
                    is_required,
                    options_json,
                    display_order,
                }
            },
        )
        .collect();

    // 3. Fetch active classes configured for this school
    let classes_rows = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT id, name
        FROM classes
        WHERE school_id = $1
        ORDER BY sequence ASC, name ASC
        "#,
    )
    .bind(school_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let classes: Vec<serde_json::Value> = classes_rows
        .into_iter()
        .map(|(id, name)| json!({ "id": id, "name": name }))
        .collect();

    let bot_phone =
        std::env::var("WHATSAPP_BOT_PHONE").unwrap_or_else(|_| "919605449544".to_string());

    Ok(Json(json!({
        "success": true,
        "school_id": school_id,
        "school_name": school_name,
        "is_window_open": is_in_date,
        "open_date": config.open_date,
        "close_date": config.close_date,
        "academic_year_id": config.academic_year_id,
        "academic_year_name": config.academic_year_name,
        "required_docs": config.required_docs,
        "custom_fields": custom_fields,
        "classes": classes,
        "whatsapp_bot_phone": bot_phone
    })))
}

/// GET /api/v1/admissions/public/captcha
/// Generate a lightweight arithmetic challenge
pub async fn get_captcha_challenge() -> Result<impl IntoResponse, AppError> {
    let mut rng = rand::thread_rng();
    let a: u32 = rng.gen_range(5..=25);
    let b: u32 = rng.gen_range(3..=15);
    let sum = a + b;

    let salt = Uuid::new_v4().to_string();
    let token = format!("{salt}:{a}:{b}:{sum}");
    let encoded_token = hex::encode(token.as_bytes());

    Ok(Json(json!({
        "question": format!("What is {a} + {b}?"),
        "captcha_token": encoded_token,
    })))
}

/// GET /api/v1/admissions/public/validate-token
/// Validates a pre-verified WhatsApp token or verified phone session and returns target phone/email
pub async fn validate_preverified_token(
    State(state): State<AppState>,
    Query(query): Query<ValidateTokenQuery>,
) -> Result<impl IntoResponse, AppError> {
    let clean_phone: String = query
        .phone
        .as_deref()
        .unwrap_or("")
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    let phone_10 = if clean_phone.len() > 10 {
        clean_phone[clean_phone.len() - 10..].to_string()
    } else {
        clean_phone.clone()
    };
    let phone_plus91 = format!("+91{}", phone_10);

    let token_clean = query.token.as_deref().unwrap_or("").trim();

    let otp_row = if !token_clean.is_empty() {
        sqlx::query_as::<_, (Uuid, String, bool, chrono::DateTime<Utc>, Option<String>)>(
            r#"
            SELECT id, target, verified, expires_at, verification_token
            FROM admission_otps
            WHERE verification_token = $1
              AND target_type = 'PHONE'
              AND verified = true
              AND expires_at > now()
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(token_clean)
        .fetch_optional(&state.db)
        .await?
    } else if !clean_phone.is_empty() {
        sqlx::query_as::<_, (Uuid, String, bool, chrono::DateTime<Utc>, Option<String>)>(
            r#"
            SELECT id, target, verified, expires_at, verification_token
            FROM admission_otps
            WHERE (target = $1 OR target = $2 OR target = $3)
              AND target_type = 'PHONE'
              AND verified = true
              AND expires_at > now()
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(&clean_phone)
        .bind(&phone_10)
        .bind(&phone_plus91)
        .fetch_optional(&state.db)
        .await?
    } else {
        None
    };

    match otp_row {
        Some((_, target, verified, _, v_token)) => {
            if !clean_phone.is_empty()
                && !target.contains(&phone_10)
                && !clean_phone.contains(&target)
            {
                return Err(AppError::BadRequest(
                    "Token does not match the provided phone number".into(),
                ));
            }

            let effective_token = v_token.unwrap_or_else(|| {
                if !token_clean.is_empty() {
                    token_clean.to_string()
                } else {
                    format!("phone_verified_{}", Uuid::new_v4())
                }
            });

            // Also check if an associated guardian/parent exists in users or admission_guardians
            let user_info = sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>, Option<String>)>(
                "SELECT display_name, first_name, last_name, email FROM users WHERE phone = $1 OR phone = $2 LIMIT 1",
            )
            .bind(&target)
            .bind(&phone_10)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            let (guardian_name, email_opt) = match user_info {
                Some((disp, f, l, em)) => {
                    let name = disp.or_else(|| match (f.as_deref(), l.as_deref()) {
                        (Some(first), Some(last)) if !first.is_empty() => {
                            Some(format!("{first} {last}").trim().to_string())
                        }
                        (Some(first), _) if !first.is_empty() => Some(first.to_string()),
                        _ => None,
                    });
                    (name, em)
                }
                None => {
                    let prev_guardian = sqlx::query_as::<_, (String, Option<String>)>(
                        "SELECT name, email FROM admission_guardians WHERE phone = $1 OR phone = $2 ORDER BY id DESC LIMIT 1",
                    )
                    .bind(&target)
                    .bind(&phone_10)
                    .fetch_optional(&state.db)
                    .await
                    .ok()
                    .flatten();
                    match prev_guardian {
                        Some((name, em)) => (Some(name), em),
                        None => (None, None),
                    }
                }
            };

            Ok(Json(json!({
                "success": true,
                "valid": verified,
                "phone": target,
                "email": email_opt,
                "guardian_name": guardian_name,
                "token": effective_token,
                "message": "Mobile number is pre-verified via WhatsApp"
            })))
        }
        None => Err(AppError::BadRequest(
            "Verification token is invalid or has expired".into(),
        )),
    }
}

/// POST /api/v1/admissions/public/send-phone-otp
pub async fn send_phone_otp(
    State(state): State<AppState>,
    Json(payload): Json<SendOtpPayload>,
) -> Result<impl IntoResponse, AppError> {
    let raw_target = payload.target.trim();
    let normalized = crate::utils::phone::normalize_phone_number(raw_target);
    let jid_digits = normalized.trim_start_matches('+');

    let clean_phone: String = raw_target.chars().filter(|c| c.is_ascii_digit()).collect();

    if clean_phone.len() < 10 {
        return Err(AppError::BadRequest(
            "Please provide a valid 10-digit mobile number".into(),
        ));
    }

    let school_name = if let Some(sid) = payload.school_id {
        sqlx::query_scalar::<_, String>("SELECT name FROM schools WHERE id = $1")
            .bind(sid)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "School".to_string())
    } else {
        sqlx::query_scalar::<_, String>(
            "SELECT name FROM schools WHERE status = 'ACTIVE' ORDER BY created_at ASC LIMIT 1",
        )
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "School".to_string())
    };

    let otp: u32 = {
        let mut rng = rand::thread_rng();
        rng.gen_range(100_000..=999_999)
    };
    let otp_str = otp.to_string();

    // Clean up previous unverified OTPs for this number
    let _ = sqlx::query(
        r#"
        DELETE FROM admission_otps
        WHERE (target = $1 OR target = $2 OR target = $3) AND target_type = 'PHONE' AND verified = false
        "#,
    )
    .bind(&clean_phone)
    .bind(&normalized)
    .bind(jid_digits)
    .execute(&state.db)
    .await;

    sqlx::query(
        r#"
        INSERT INTO admission_otps (target, target_type, otp_code, expires_at)
        VALUES ($1, 'PHONE', $2, now() + INTERVAL '10 minutes')
        "#,
    )
    .bind(&clean_phone)
    .bind(&otp_str)
    .execute(&state.db)
    .await?;

    // Format proper WhatsApp JID with country code e.g. 919876543210@s.whatsapp.net
    let jid = format!("{jid_digits}@s.whatsapp.net");
    let msg = format!(
        "🔐 *{} Admission Verification*\n\nYour one-time phone verification code is: *{otp_str}*\n\nThis code expires in 10 minutes. Please enter it on the admission application page.",
        school_name
    );
    let _ = crate::utils::whatsapp::send_urgent_whatsapp_message(&state.db, &jid, &msg, None).await;

    println!("============================================================");
    println!("🎓 [ADMISSION WHATSAPP OTP DISPATCHED]");
    println!("Target Phone: {} (JID: {})", clean_phone, jid);
    println!("Verification Code: {}", otp_str);
    println!("============================================================");

    tracing::info!(phone = %clean_phone, jid = %jid, otp = %otp_str, "Dispatched admission phone OTP");

    Ok(Json(json!({
        "success": true,
        "message": "OTP has been sent to your mobile number via WhatsApp",
        "otp_hint": otp_str
    })))
}

/// POST /api/v1/admissions/public/verify-phone-otp
pub async fn verify_phone_otp(
    State(state): State<AppState>,
    Json(payload): Json<VerifyOtpPayload>,
) -> Result<impl IntoResponse, AppError> {
    let raw_target = payload.target.trim();
    let normalized = crate::utils::phone::normalize_phone_number(raw_target);
    let jid_digits = normalized.trim_start_matches('+');

    let clean_phone: String = raw_target.chars().filter(|c| c.is_ascii_digit()).collect();

    let otp_row = sqlx::query_as::<_, (Uuid, String, chrono::DateTime<Utc>)>(
        r#"
        SELECT id, otp_code, expires_at
        FROM admission_otps
        WHERE (target = $1 OR target = $2 OR target = $3) AND target_type = 'PHONE' AND verified = false
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&clean_phone)
    .bind(&normalized)
    .bind(jid_digits)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest(
            "No pending OTP found for this phone number. Please request a new code.".into(),
        )
    })?;

    if otp_row.2 <= Utc::now() {
        return Err(AppError::BadRequest(
            "OTP has expired. Please request a new one.".into(),
        ));
    }

    if otp_row.1 != payload.otp.trim() {
        return Err(AppError::BadRequest(
            "Invalid verification code. Please check and try again.".into(),
        ));
    }

    let token = format!("phone_verified_{}", Uuid::new_v4());

    sqlx::query(
        r#"
        UPDATE admission_otps
        SET verified = true, verification_token = $2
        WHERE id = $1
        "#,
    )
    .bind(otp_row.0)
    .bind(&token)
    .execute(&state.db)
    .await?;

    Ok(Json(json!({
        "success": true,
        "token": token,
        "message": "Phone number verified successfully!"
    })))
}

/// POST /api/v1/admissions/public/send-email-otp
pub async fn send_email_otp(
    State(state): State<AppState>,
    Json(payload): Json<SendOtpPayload>,
) -> Result<impl IntoResponse, AppError> {
    let clean_email = payload.target.trim().to_lowercase();
    if !clean_email.contains('@') || clean_email.len() < 5 {
        return Err(AppError::BadRequest(
            "Please provide a valid email address".into(),
        ));
    }

    let otp: u32 = {
        let mut rng = rand::thread_rng();
        rng.gen_range(100_000..=999_999)
    };
    let otp_str = otp.to_string();

    sqlx::query(
        r#"
        INSERT INTO admission_otps (target, target_type, otp_code, expires_at)
        VALUES ($1, 'EMAIL', $2, now() + INTERVAL '10 minutes')
        "#,
    )
    .bind(&clean_email)
    .bind(&otp_str)
    .execute(&state.db)
    .await?;

    let school_name = if let Some(sid) = payload.school_id {
        sqlx::query_scalar::<_, String>("SELECT name FROM schools WHERE id = $1")
            .bind(sid)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "School".to_string())
    } else {
        sqlx::query_scalar::<_, String>(
            "SELECT name FROM schools WHERE status = 'ACTIVE' ORDER BY created_at ASC LIMIT 1",
        )
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "School".to_string())
    };

    let email_subject = format!("{} Admission Verification Code", school_name);
    let email_body = format!(
        "<h3>{} Admission Verification</h3>\
         <p>Your one-time email verification code is: <strong>{}</strong></p>\
         <p>This code expires in 10 minutes. Please enter it on the admission application page.</p>",
        school_name, otp_str
    );
    let to_email = clean_email.clone();
    let s_name = school_name.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::utils::email::send_email_with_sender(
            Some(&s_name),
            &to_email,
            &email_subject,
            &email_body,
        )
        .await
        {
            tracing::error!("Failed to send admission email OTP to {}: {}", to_email, e);
        }
    });

    println!("============================================================");
    println!("🎓 [ADMISSION EMAIL OTP DISPATCHED]");
    println!("Target Email: {}", clean_email);
    println!("Verification Code: {}", otp_str);
    println!("============================================================");

    tracing::info!(email = %clean_email, otp = %otp_str, "Dispatched admission email OTP");

    Ok(Json(json!({
        "success": true,
        "message": "Verification code has been sent to your email.",
        "otp_hint": otp_str
    })))
}

/// POST /api/v1/admissions/public/verify-email-otp
pub async fn verify_email_otp(
    State(state): State<AppState>,
    Json(payload): Json<VerifyOtpPayload>,
) -> Result<impl IntoResponse, AppError> {
    let clean_email = payload.target.trim().to_lowercase();

    let otp_row = sqlx::query_as::<_, (Uuid, String, chrono::DateTime<Utc>)>(
        r#"
        SELECT id, otp_code, expires_at
        FROM admission_otps
        WHERE target = $1 AND target_type = 'EMAIL' AND verified = false
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&clean_email)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest(
            "No pending OTP found for this email address. Please request a new code.".into(),
        )
    })?;

    if otp_row.2 <= Utc::now() {
        return Err(AppError::BadRequest(
            "OTP has expired. Please request a new one.".into(),
        ));
    }

    if otp_row.1 != payload.otp.trim() {
        return Err(AppError::BadRequest(
            "Invalid verification code. Please check and try again.".into(),
        ));
    }

    let token = format!("email_verified_{}", Uuid::new_v4());

    sqlx::query(
        r#"
        UPDATE admission_otps
        SET verified = true, verification_token = $2
        WHERE id = $1
        "#,
    )
    .bind(otp_row.0)
    .bind(&token)
    .execute(&state.db)
    .await?;

    Ok(Json(json!({
        "success": true,
        "token": token,
        "message": "Email address verified successfully!"
    })))
}

/// POST /api/v1/admissions/public/submit
/// Complete public admission application submission adhering to Gcpe360 SRD schema.
pub async fn submit_admission_application(
    State(state): State<AppState>,
    Json(payload): Json<PublicAdmissionSubmitRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (school_id, school_name) = resolve_primary_school(&state.db).await?;

    // 1. Verify captcha answer
    let decoded_bytes = hex::decode(&payload.captcha_token)
        .map_err(|_| AppError::BadRequest("Invalid captcha token".into()))?;
    let captcha_str = String::from_utf8(decoded_bytes)
        .map_err(|_| AppError::BadRequest("Invalid captcha format".into()))?;
    let parts: Vec<&str> = captcha_str.split(':').collect();
    if parts.len() != 4 {
        return Err(AppError::BadRequest("Invalid captcha challenge".into()));
    }
    let expected_sum = parts[3];
    if payload.captcha_answer.trim() != expected_sum {
        return Err(AppError::BadRequest(
            "Incorrect captcha answer. Please solve the arithmetic question.".into(),
        ));
    }

    // 2. Verify Phone Token
    let clean_phone: String = payload
        .applicant_phone
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    let phone_10 = if clean_phone.len() > 10 {
        clean_phone[clean_phone.len() - 10..].to_string()
    } else {
        clean_phone.clone()
    };

    let phone_verified = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT verified
        FROM admission_otps
        WHERE (target = $1 OR target = $2) AND target_type = 'PHONE' AND verification_token = $3
        "#,
    )
    .bind(&clean_phone)
    .bind(&phone_10)
    .bind(&payload.phone_token)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(false);

    if !phone_verified {
        return Err(AppError::BadRequest(
            "Phone number has not been verified with OTP".into(),
        ));
    }

    // 3. Verify Email Token
    let clean_email = payload.applicant_email.trim().to_lowercase();
    let email_verified = if payload.phone_token.starts_with("wa_verified_") {
        if !payload.email_token.is_empty() && !payload.email_token.starts_with("wa_verified_") {
            sqlx::query_scalar::<_, bool>(
                r#"
                SELECT verified
                FROM admission_otps
                WHERE target = $1 AND target_type = 'EMAIL' AND verification_token = $2
                "#,
            )
            .bind(&clean_email)
            .bind(&payload.email_token)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or(false)
        } else {
            true // Pre-verified via WhatsApp!
        }
    } else {
        sqlx::query_scalar::<_, bool>(
            r#"
            SELECT verified
            FROM admission_otps
            WHERE target = $1 AND target_type = 'EMAIL' AND verification_token = $2
            "#,
        )
        .bind(&clean_email)
        .bind(&payload.email_token)
        .fetch_optional(&state.db)
        .await?
        .unwrap_or(false)
    };

    if !email_verified {
        return Err(AppError::BadRequest(
            "Email address has not been verified with OTP".into(),
        ));
    }

    // 4. Resolve Admission Settings & Academic Year (SRD Tracking)
    let setting_val = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = 'admission_settings'
        "#,
    )
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?;

    let (academic_year_id, academic_year_name, prefix, separator, year_in_format) = if let Some(
        val,
    ) =
        setting_val
    {
        let conf: AdmissionConfigPayload =
            serde_json::from_value(val).unwrap_or_else(|_| AdmissionConfigPayload {
                academic_year_id: None,
                academic_year_name: None,
                prefix: "ADM".into(),
                separator: "-".into(),
                start_number: "001".into(),
                year_in_format: true,
                intake_capacity: 500,
                open_date: "2026-01-01".into(),
                close_date: "2026-12-31".into(),
                required_docs: vec![],
                is_open: Some(true),
            });

        // Verify window is currently open
        let today = Utc::now().date_naive();
        if let (Ok(open), Ok(close)) = (
            NaiveDate::parse_from_str(&conf.open_date, "%Y-%m-%d"),
            NaiveDate::parse_from_str(&conf.close_date, "%Y-%m-%d"),
        ) {
            if today < open || today > close {
                return Err(AppError::BadRequest(format!(
                    "Admissions are currently closed. The application window is open from {} to {}.",
                    conf.open_date, conf.close_date
                )));
            }
        }

        (
            conf.academic_year_id,
            conf.academic_year_name,
            conf.prefix,
            conf.separator,
            conf.year_in_format,
        )
    } else {
        (None, None, "ADM".into(), "-".into(), true)
    };

    // Ensure accurate academic_year_id from database per SRD
    let (ay_id, ay_name) = if let Some(id) = academic_year_id {
        let name = sqlx::query_scalar::<_, String>("SELECT name FROM academic_years WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or_else(|| academic_year_name.unwrap_or_else(|| "2026-2027".into()));
        (id, name)
    } else {
        let active_ay = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM academic_years WHERE school_id = $1 AND status = 'ACTIVE' ORDER BY start_date DESC LIMIT 1"
        )
        .bind(school_id)
        .fetch_optional(&state.db)
        .await?;

        match active_ay {
            Some((id, name)) => (id, name),
            None => {
                let fallback_id = Uuid::new_v4();
                (fallback_id, "2026-2027".to_string())
            }
        }
    };

    // 5. Generate formatted Application Number
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admissions WHERE school_id = $1")
        .bind(school_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let seq_num = count + 1;
    let app_number = if year_in_format {
        format!("{prefix}{separator}{ay_name}{separator}{seq_num:03}")
    } else {
        format!("{prefix}{separator}{seq_num:03}")
    };

    // 6. Parse Date of Birth
    let dob = DateUtils::parse_iso_date(&payload.date_of_birth)
        .map_err(|e| AppError::BadRequest(format!("Invalid date of birth: {e}")))?;

    // 7. Begin Database Transaction for Atomic SRD Insertion
    let mut tx = state.db.begin().await?;

    // Check for duplicate applicant in the same academic year to prevent double submissions
    let existing_app = sqlx::query_as::<_, (Uuid, Option<String>)>(
        r#"
        SELECT a.id, a.application_number
        FROM admissions a
        JOIN admission_applicants ap ON ap.admission_id = a.id
        WHERE a.school_id = $1
          AND a.academic_year_id = $2
          AND LOWER(TRIM(ap.first_name)) = LOWER(TRIM($3))
          AND LOWER(TRIM(ap.last_name)) = LOWER(TRIM($4))
          AND ap.date_of_birth = $5
          AND a.status NOT IN ('REJECTED', 'CANCELLED')
        LIMIT 1
        "#,
    )
    .bind(school_id)
    .bind(ay_id)
    .bind(&payload.applicant_first_name)
    .bind(&payload.applicant_last_name)
    .bind(dob)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some((_, existing_app_no)) = existing_app {
        let app_no_str = existing_app_no.unwrap_or_else(|| "PENDING".to_string());
        return Err(AppError::Conflict(format!(
            "An admission application for {} {} has already been submitted (Application No: {}). Please do not submit duplicate applications.",
            payload.applicant_first_name, payload.applicant_last_name, app_no_str
        )));
    }

    let admission_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO admissions (school_id, academic_year_id, application_number, status, submitted_at)
        VALUES ($1, $2, $3, 'PENDING', now())
        RETURNING id
        "#,
    )
    .bind(school_id)
    .bind(ay_id)
    .bind(&app_number)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO admission_applicants
            (admission_id, first_name, middle_name, last_name, date_of_birth, gender, phone, email)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(admission_id)
    .bind(&payload.applicant_first_name)
    .bind(payload.applicant_middle_name.as_deref())
    .bind(&payload.applicant_last_name)
    .bind(dob)
    .bind(payload.applicant_gender.as_deref())
    .bind(Some(&clean_phone))
    .bind(Some(&clean_email))
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO admission_guardians
            (admission_id, name, relationship, phone, email, occupation)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(admission_id)
    .bind(&payload.guardian_name)
    .bind(&payload.guardian_relationship)
    .bind(&payload.guardian_phone)
    .bind(payload.guardian_email.as_deref())
    .bind(payload.guardian_occupation.as_deref())
    .execute(&mut *tx)
    .await?;

    // Record initial status history
    sqlx::query(
        r#"
        INSERT INTO admission_status_history (admission_id, to_status, reason)
        VALUES ($1, 'PENDING', 'Online Admission Portal Submission')
        "#,
    )
    .bind(admission_id)
    .execute(&mut *tx)
    .await?;

    // Custom values insertion
    for (key, val) in &payload.custom_fields {
        let field_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM custom_fields WHERE school_id = $1 AND entity_type IN ('ADMISSION', 'STUDENT') AND field_key = $2",
        )
        .bind(school_id)
        .bind(key)
        .fetch_optional(&mut *tx)
        .await?;

        let fid = match field_id {
            Some(id) => id,
            None => {
                let display_title = key.replace('_', " ");
                let new_fid: Uuid = sqlx::query_scalar(
                    r#"
                    INSERT INTO custom_fields (school_id, entity_type, field_name, field_key, field_type, is_required)
                    VALUES ($1, 'ADMISSION', $2, $3, 'TEXT', false)
                    RETURNING id
                    "#,
                )
                .bind(school_id)
                .bind(&display_title)
                .bind(key)
                .fetch_one(&mut *tx)
                .await?;
                new_fid
            }
        };

        sqlx::query(
            r#"
            INSERT INTO admission_custom_values (admission_id, field_id, value)
            VALUES ($1, $2, $3)
            ON CONFLICT (admission_id, field_id) DO UPDATE SET value = EXCLUDED.value
            "#,
        )
        .bind(admission_id)
        .bind(fid)
        .bind(val)
        .execute(&mut *tx)
        .await?;
    }

    // Invalidate phone verification token so it cannot be reused for duplicate submissions
    let _ = sqlx::query(
        r#"
        UPDATE admission_otps
        SET verified = false, verification_token = NULL
        WHERE verification_token = $1
        "#,
    )
    .bind(&payload.phone_token)
    .execute(&mut *tx)
    .await;

    tx.commit().await?;

    // 8. Dispatch Confirmation WhatsApp Message
    let applicant_name = format!(
        "{} {}",
        payload.applicant_first_name, payload.applicant_last_name
    );
    let normalized_guardian = crate::utils::phone::normalize_phone_number(&payload.guardian_phone);
    let jid_digits = normalized_guardian.trim_start_matches('+');
    let jid = format!("{jid_digits}@s.whatsapp.net");
    let confirm_msg = format!(
        "🎉 *{} Admission Application Submitted!*\n\n\
        Dear *{}*,\n\
        We have received the admission application for *{}* at *{}*.\n\n\
        📋 *Application Number*: *{}*\n\
        📅 *Academic Year*: *{}*\n\
        📌 *Status*: Submitted & Under Review\n\n\
        Our admissions desk has been notified. You can use your application number for any future updates or inquiries.",
        school_name,
        payload.guardian_name,
        applicant_name,
        school_name,
        app_number,
        ay_name
    );
    let _ = send_whatsapp_message(&state.db, &jid, &confirm_msg, None).await;

    tracing::info!(
        admission_id = %admission_id,
        application_number = %app_number,
        academic_year = %ay_name,
        phone = %clean_phone,
        "Online admission application successfully submitted per SRD"
    );

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "admission_id": admission_id,
            "application_number": app_number,
            "academic_year": ay_name,
            "school_name": school_name,
            "message": "Admission application submitted successfully!"
        })),
    ))
}

/// GET /api/v1/admissions/public/application/:id
/// Returns canonical verified submitted data from DB for client-side document rendering
pub async fn get_submitted_application_details(
    State(state): State<AppState>,
    Path(admission_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let adm_row = sqlx::query_as::<
        _,
        (
            Uuid,
            Uuid,
            Option<Uuid>,
            Option<String>,
            String,
            Option<chrono::DateTime<Utc>>,
        ),
    >(
        r#"
        SELECT id, school_id, academic_year_id, application_number, status::text, submitted_at
        FROM admissions
        WHERE id = $1
        "#,
    )
    .bind(admission_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Admission application not found".into()))?;

    let (id, school_id, ay_id, app_no, status, submitted_at) = adm_row;
    let app_number_str = app_no.clone().unwrap_or_else(|| "PENDING".to_string());

    // School details
    let school = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        r#"
        SELECT name,
               NULLIF(CONCAT_WS(', ', address_line_1, address_line_2), '') as address,
               city, phone, email
        FROM schools
        WHERE id = $1
        "#,
    )
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(("School".to_string(), None, None, None, None));

    // Academic year name
    let ay_name: String = if let Some(ay) = ay_id {
        sqlx::query_scalar("SELECT name FROM academic_years WHERE id = $1")
            .bind(ay)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or_else(|| "2026-2027".to_string())
    } else {
        "2026-2027".to_string()
    };

    // Applicant
    let applicant = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            String,
            NaiveDate,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        r#"
        SELECT first_name, middle_name, last_name, date_of_birth, gender, phone, email
        FROM admission_applicants
        WHERE admission_id = $1
        LIMIT 1
        "#,
    )
    .bind(admission_id)
    .fetch_optional(&state.db)
    .await?;

    // Guardian
    let guardian = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>)>(
        r#"
        SELECT name, relationship, phone, email, occupation
        FROM admission_guardians
        WHERE admission_id = $1
        LIMIT 1
        "#,
    )
    .bind(admission_id)
    .fetch_optional(&state.db)
    .await?;

    // Custom fields & values
    let custom_values = sqlx::query_as::<_, (String, String, String)>(
        r#"
        SELECT cf.field_name, cf.field_key, acv.value
        FROM admission_custom_values acv
        JOIN custom_fields cf ON cf.id = acv.field_id
        WHERE acv.admission_id = $1
        ORDER BY cf.display_order ASC
        "#,
    )
    .bind(admission_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let custom_fields_json: Vec<serde_json::Value> = custom_values
        .into_iter()
        .map(|(name, key, val)| {
            json!({
                "name": name,
                "key": key,
                "value": val
            })
        })
        .collect();

    // Admission form template config from school_settings
    let template_config = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM school_settings WHERE school_id = $1 AND key = 'report_tpl_admission_form'",
    )
    .bind(school_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Admissions config from school_settings (for dynamically configured required documents)
    let adm_setting_val = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM school_settings WHERE school_id = $1 AND key = 'admissions_config'",
    )
    .bind(school_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let required_docs = adm_setting_val
        .and_then(|v| v.get("required_docs").cloned())
        .unwrap_or_else(|| {
            json!([
                { "id": "birth_cert", "label": "Birth Certificate", "required": true },
                { "id": "prev_report", "label": "Previous Year Report Card", "required": true },
                { "id": "transfer_cert", "label": "Transfer Certificate (TC)", "required": false },
                { "id": "photo", "label": "Passport-size Photographs (4 copies)", "required": true },
                { "id": "address_proof", "label": "Address Proof (Aadhaar / Utility Bill)", "required": true },
                { "id": "caste_cert", "label": "Caste / Category Certificate", "required": false },
                { "id": "medical_fit", "label": "Medical Fitness Certificate", "required": false }
            ])
        });

    // Generate Verification QR Code SVG (User Request: "below needs to be a qr code when scanned can view the uploaded pdfs")
    let app_url = std::env::var("APP_URL")
        .unwrap_or_else(|_| "https://my-frontend-kappa-five.vercel.app".to_string());
    let clean_url = app_url.trim_end_matches('/');
    let verify_url =
        format!("{clean_url}/admissions/verify?id={admission_id}&app_no={app_number_str}");

    let qr_code_svg = if let Ok(code) = QrCode::new(verify_url.as_bytes()) {
        code.render::<svg::Color>().min_dimensions(120, 120).build()
    } else {
        String::new()
    };

    Ok(Json(json!({
        "success": true,
        "application": {
            "id": id,
            "school_id": school_id,
            "school_name": school.0,
            "school_address": school.1.unwrap_or_else(|| "Campus Road, Knowledge Park".to_string()),
            "school_city": school.2.unwrap_or_default(),
            "school_phone": school.3.unwrap_or_default(),
            "school_email": school.4.unwrap_or_default(),
            "academic_year": ay_name,
            "application_number": app_number_str,
            "status": status,
            "submitted_at": submitted_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
            "applicant": applicant.map(|a| json!({
                "first_name": a.0,
                "middle_name": a.1.unwrap_or_default(),
                "last_name": a.2,
                "date_of_birth": a.3.to_string(),
                "gender": a.4.unwrap_or_else(|| "Male".to_string()),
                "phone": a.5.unwrap_or_default(),
                "email": a.6.unwrap_or_default()
            })),
            "guardian": guardian.map(|g| json!({
                "name": g.0,
                "relationship": g.1,
                "phone": g.2,
                "email": g.3.unwrap_or_default(),
                "occupation": g.4.unwrap_or_default()
            })),
            "custom_fields": custom_fields_json,
            "template_config": template_config,
            "required_docs": required_docs,
            "qr_code_svg": qr_code_svg,
            "verify_url": verify_url
        }
    })))
}

/// POST /api/v1/admissions/public/upload-document
/// Uploads client-rendered Admission Form image to ImageKit.io with 24-hour signed link
pub async fn upload_admission_document(
    State(state): State<AppState>,
    Json(payload): Json<UploadDocumentPayload>,
) -> Result<impl IntoResponse, AppError> {
    use base64::Engine;

    // 1. Clean base64 image data
    let raw_b64 = if let Some(idx) = payload.image_base64.find(";base64,") {
        &payload.image_base64[idx + 8..]
    } else {
        &payload.image_base64
    };

    let image_bytes = base64::engine::general_purpose::STANDARD
        .decode(raw_b64.trim())
        .map_err(|e| AppError::BadRequest(format!("Invalid base64 document image: {e}")))?;

    // 2. Fetch school and application context
    let adm_row = sqlx::query_as::<_, (Uuid, Uuid, Option<Uuid>, Option<String>)>(
        "SELECT id, school_id, academic_year_id, application_number FROM admissions WHERE id = $1",
    )
    .bind(payload.admission_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Admission record not found".into()))?;

    let (admission_id, school_id, ay_id, app_no) = adm_row;
    let application_number = app_no.unwrap_or(payload.application_number);

    let school_name = sqlx::query_scalar::<_, String>("SELECT name FROM schools WHERE id = $1")
        .bind(school_id)
        .fetch_optional(&state.db)
        .await?
        .unwrap_or_else(|| "School".to_string());

    let ay_name: String = if let Some(ay) = ay_id {
        sqlx::query_scalar("SELECT name FROM academic_years WHERE id = $1")
            .bind(ay)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or_else(|| "2026-2027".to_string())
    } else {
        "2026-2027".to_string()
    };

    // 3. Upload to ImageKit / local archive with 24-hour signed link
    let doc_type = payload.document_type.as_deref().unwrap_or("form");
    let file_clean = application_number.replace(['/', ' '], "_");
    let file_name = if let Some(ref custom_name) = payload.file_name {
        custom_name.clone()
    } else if doc_type == "form" {
        format!("Admission_Form_{file_clean}.png")
    } else {
        format!("{doc_type}_{file_clean}.png")
    };
    let folder = format!("/admissions/{ay_name}/{file_clean}");

    let upload_res = ImageKitClient::upload_document(image_bytes, &file_name, &folder).await?;

    // If uploading a supporting document attachment (e.g. birth_cert, address_proof, photo):
    if doc_type != "form" {
        let field_key = format!("doc_{doc_type}_url");
        let field_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM custom_fields WHERE school_id = $1 AND field_key = $2",
        )
        .bind(school_id)
        .bind(&field_key)
        .fetch_optional(&state.db)
        .await?;

        let fid = match field_id {
            Some(id) => id,
            None => {
                let display_title = format!("Document: {}", doc_type.replace('_', " "));
                sqlx::query_scalar(
                    r#"
                    INSERT INTO custom_fields (school_id, entity_type, field_name, field_key, field_type, is_required)
                    VALUES ($1, 'ADMISSION', $2, $3, 'TEXT', false)
                    RETURNING id
                    "#,
                )
                .bind(school_id)
                .bind(&display_title)
                .bind(&field_key)
                .fetch_one(&state.db)
                .await?
            }
        };

        sqlx::query(
            r#"
            INSERT INTO admission_custom_values (admission_id, field_id, value)
            VALUES ($1, $2, $3)
            ON CONFLICT (admission_id, field_id) DO UPDATE SET value = EXCLUDED.value
            "#,
        )
        .bind(admission_id)
        .bind(fid)
        .bind(&upload_res.signed_url_24h)
        .execute(&state.db)
        .await?;

        return Ok(Json(json!({
            "success": true,
            "document_url": upload_res.signed_url_24h,
            "file_id": upload_res.file_id,
            "expires_at": upload_res.expires_at,
            "message": format!("Document {doc_type} uploaded successfully to ImageKit")
        })));
    }

    // 4. Retrieve applicant and guardian contacts
    let applicant_name = sqlx::query_as::<_, (String, String)>(
        "SELECT first_name, last_name FROM admission_applicants WHERE admission_id = $1",
    )
    .bind(admission_id)
    .fetch_optional(&state.db)
    .await?
    .map(|(f, l)| format!("{f} {l}"))
    .unwrap_or_else(|| "Applicant".to_string());

    let guardian_info = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT name, phone, email FROM admission_guardians WHERE admission_id = $1",
    )
    .bind(admission_id)
    .fetch_optional(&state.db)
    .await?;

    let signed_url = upload_res.signed_url_24h.clone();
    let email_target = guardian_info.as_ref().and_then(|g| g.2.clone());
    let guardian_name = guardian_info
        .as_ref()
        .map(|g| g.0.clone())
        .unwrap_or_else(|| "Parent / Guardian".to_string());
    let guardian_phone = guardian_info
        .as_ref()
        .map(|g| g.1.clone())
        .unwrap_or_default();

    // 5. Send Email if target available
    if let Some(target_email) = email_target {
        if !target_email.trim().is_empty() && target_email.contains('@') {
            let email_subj = format!(
                "Official Admission Application Form – {} ({})",
                application_number, school_name
            );
            let email_html = format!(
                r#"<!DOCTYPE html>
                <html>
                <body style="font-family: Arial, sans-serif; line-height: 1.6; color: #1e293b; max-width: 600px; margin: 0 auto; padding: 24px; background: #ffffff;">
                  <div style="text-align: center; border-bottom: 2px solid #0284c7; padding-bottom: 16px; margin-bottom: 24px;">
                    <h2 style="margin: 0; color: #0f172a; font-size: 22px;">{school_name}</h2>
                    <p style="margin: 4px 0 0; font-size: 13px; color: #64748b;">Official Online Admissions &amp; Student Registration</p>
                  </div>
                  <p style="font-size: 14px;">Dear <strong>{guardian_name}</strong>,</p>
                  <p style="font-size: 14px;">The admission application for <strong>{applicant_name}</strong> has been registered in the institutional ERP database.</p>
                  
                  <div style="background: #f8fafc; border: 1.5px dashed #cbd5e1; border-radius: 12px; padding: 18px; margin: 20px 0; text-align: center;">
                    <div style="font-size: 11px; color: #64748b; text-transform: uppercase; font-weight: bold; letter-spacing: 0.5px;">Official Application Number</div>
                    <div style="font-size: 24px; font-weight: 900; color: #0369a1; letter-spacing: 1px; margin-top: 4px;">{app_no}</div>
                    <div style="font-size: 12px; color: #64748b; margin-top: 4px;">Academic Session: <strong>{ay_name}</strong></div>
                  </div>

                  <p style="font-size: 13px; color: #475569;">
                    Your official box-box formatted Admission Application Form has been rendered and preserved in <strong>ImageKit.io</strong> storage. Click below to view or download:
                  </p>

                  <div style="text-align: center; margin: 28px 0;">
                    <a href="{signed_url}" style="background: #0284c7; color: #ffffff; padding: 14px 32px; border-radius: 10px; text-decoration: none; font-weight: bold; font-size: 14px; display: inline-block; box-shadow: 0 4px 12px rgba(2, 132, 199, 0.3);">
                      📥 View &amp; Download Admission Form (24-Hour Link)
                    </a>
                  </div>

                  <p style="font-size: 11px; color: #94a3b8; text-align: center; border-top: 1px solid #e2e8f0; padding-top: 16px;">
                    🔒 <em>This download link is securely signed and valid for 24 hours. A permanent copy is retained in the school admission office registry.</em>
                  </p>
                </body>
                </html>"#,
                school_name = school_name,
                guardian_name = guardian_name,
                applicant_name = applicant_name,
                app_no = application_number,
                ay_name = ay_name,
                signed_url = signed_url
            );

            tokio::spawn(async move {
                let _ =
                    crate::utils::email::send_email(&target_email, &email_subj, &email_html).await;
            });
        }
    }

    // 6. Deliver to WhatsApp if phone available
    let clean_phone: String = guardian_phone
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    if clean_phone.len() >= 10 {
        let jid = format!("{clean_phone}@s.whatsapp.net");
        let wa_msg = format!(
            "📄 *Official Admission Form Generated*\n\n\
            Dear *{}*,\n\
            The official admission application form for *{}* (Application No: `{}`) has been generated!\n\n\
            📥 *Download Your Document (Valid 24 Hours)*:\n\
            {}\n\n\
            Please download and retain this signed copy for your school verification.",
            guardian_name,
            applicant_name,
            application_number,
            upload_res.signed_url_24h
        );
        let _ = send_whatsapp_message(&state.db, &jid, &wa_msg, None).await;
    }

    Ok(Json(json!({
        "success": true,
        "document_url": upload_res.signed_url_24h,
        "file_id": upload_res.file_id,
        "expires_at": upload_res.expires_at,
        "message": "Admission document uploaded and 24-hour link dispatched via email & WhatsApp"
    })))
}

/// GET /api/v1/admissions/public/documents/:file_id
pub async fn serve_local_document(
    Path(file_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let clean_id: String = file_id
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    let file_path = std::path::Path::new("uploads/admissions").join(format!("{clean_id}.png"));
    let bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|_| AppError::NotFound("Document not found or expired".into()))?;

    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "image/png"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "inline; filename=\"admission_form.png\"",
            ),
        ],
        bytes,
    ))
}

pub fn normalize_phone_last_10(phone: &str) -> String {
    let clean: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    if clean.len() >= 10 {
        clean[clean.len() - 10..].to_string()
    } else {
        clean
    }
}

pub fn mask_email(email: &str) -> String {
    let trimmed = email.trim();
    if let Some((user, domain)) = trimmed.split_once('@') {
        if user.len() <= 2 {
            format!("{}***@{}", &user[0..1], domain)
        } else {
            let first = &user[0..1];
            let last = &user[user.len() - 1..];
            format!("{first}***{last}@{domain}")
        }
    } else {
        "***@***.com".to_string()
    }
}

/// POST /api/v1/admissions/public/officer-request-otp
/// Authenticates phone number for scanned admission form and dispatches OTP to registered email.
pub async fn officer_request_otp(
    State(state): State<AppState>,
    Json(payload): Json<OfficerRequestOtpPayload>,
) -> Result<impl IntoResponse, AppError> {
    let admission_id = payload.admission_id;

    // 1. Verify admission record exists
    let adm = sqlx::query_as::<_, (Uuid, Uuid, Option<String>)>(
        "SELECT id, school_id, application_number FROM admissions WHERE id = $1",
    )
    .bind(admission_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Admission record not found".into()))?;

    let (_, school_id, app_no_opt) = adm;
    let app_number = app_no_opt.unwrap_or_else(|| "PENDING".to_string());

    // 2. Fetch applicant and guardian contact info
    let applicant = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
        ),
    >(
        r#"
        SELECT first_name, middle_name, last_name, phone, email
        FROM admission_applicants
        WHERE admission_id = $1
        LIMIT 1
        "#,
    )
    .bind(admission_id)
    .fetch_optional(&state.db)
    .await?;

    let guardian = sqlx::query_as::<_, (String, String, Option<String>)>(
        r#"
        SELECT name, phone, email
        FROM admission_guardians
        WHERE admission_id = $1
        LIMIT 1
        "#,
    )
    .bind(admission_id)
    .fetch_optional(&state.db)
    .await?;

    // 3. Match normalized phone number against applicant or guardian
    let input_10 = normalize_phone_last_10(&payload.phone);
    if input_10.is_empty() || input_10.len() < 7 {
        return Err(AppError::BadRequest(
            "Please provide a valid phone number (at least 10 digits).".into(),
        ));
    }

    let applicant_phone_10 = applicant
        .as_ref()
        .and_then(|a| a.3.as_deref())
        .map(normalize_phone_last_10);
    let guardian_phone_10 = guardian.as_ref().map(|g| normalize_phone_last_10(&g.1));

    let matches_applicant = applicant_phone_10.as_deref() == Some(&input_10);
    let matches_guardian = guardian_phone_10.as_deref() == Some(&input_10);

    if !matches_applicant && !matches_guardian {
        return Err(AppError::BadRequest(
            "The entered phone number does not match registered applicant or guardian records."
                .into(),
        ));
    }

    // 4. Resolve registered email address (guardian email preferred, fallback to applicant email)
    let target_email = guardian
        .as_ref()
        .and_then(|g| g.2.clone())
        .filter(|e| e.contains('@') && !e.trim().is_empty())
        .or_else(|| {
            applicant
                .as_ref()
                .and_then(|a| a.4.clone())
                .filter(|e| e.contains('@') && !e.trim().is_empty())
        })
        .ok_or_else(|| {
            AppError::BadRequest(
                "No registered email found for this application record. Please contact school administration.".into(),
            )
        })?;

    // 5. Generate secure 6-digit OTP
    let otp: u32 = {
        let mut rng = rand::thread_rng();
        rng.gen_range(100_000..=999_999)
    };
    let otp_str = otp.to_string();

    let target_key = format!("{}:{}", admission_id, input_10);

    // Save to admission_otps
    sqlx::query(
        r#"
        INSERT INTO admission_otps (target, target_type, otp_code, expires_at)
        VALUES ($1, 'OFFICER_VERIFY', $2, now() + INTERVAL '10 minutes')
        "#,
    )
    .bind(&target_key)
    .bind(&otp_str)
    .execute(&state.db)
    .await?;

    // Fetch school name for email branding
    let school_name = sqlx::query_scalar::<_, String>("SELECT name FROM schools WHERE id = $1")
        .bind(school_id)
        .fetch_optional(&state.db)
        .await?
        .unwrap_or_else(|| "School".to_string());

    // 6. Send OTP Email asynchronously
    let email_dest = target_email.clone();
    let email_otp = otp_str.clone();
    let email_app_no = app_number.clone();
    let email_school_name = school_name.clone();

    tokio::spawn(async move {
        let subject =
            format!("{email_school_name} Document Verification Security Code – {email_app_no}");
        let body = format!(
            r#"<!DOCTYPE html>
<html>
<body style="margin: 0; padding: 24px; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; background-color: #f8fafc; color: #1e293b;">
  <div style="max-width: 520px; margin: 0 auto; background: #ffffff; border: 2px solid #e2e8f0; border-radius: 24px; overflow: hidden; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.05);">
    <div style="background: #0284c7; padding: 24px; text-align: center; color: #ffffff;">
      <div style="font-size: 11px; font-weight: 800; letter-spacing: 2px; text-transform: uppercase; background: rgba(255,255,255,0.2); display: inline-block; padding: 4px 12px; border-radius: 9999px; margin-bottom: 8px;">
        Institutional Verification Gate
      </div>
      <h1 style="margin: 0; font-size: 22px; font-weight: 900; letter-spacing: -0.5px;">Document Access OTP</h1>
      <p style="margin: 6px 0 0; font-size: 13px; opacity: 0.9;">{email_school_name}</p>
    </div>
    <div style="padding: 28px 24px; text-align: center;">
      <p style="margin: 0 0 16px; font-size: 14px; line-height: 1.6; color: #334155;">
        An admissions officer has scanned the QR seal on your application <strong>{email_app_no}</strong> and requested authorization to view your uploaded supporting documents.
      </p>
      <div style="background: #f0f9ff; border: 2px dashed #0284c7; border-radius: 16px; padding: 20px; margin: 20px 0;">
        <div style="font-size: 11px; font-weight: 700; color: #0284c7; text-transform: uppercase; letter-spacing: 1px; margin-bottom: 6px;">One-Time Security Code</div>
        <div style="font-family: monospace, Courier; font-size: 34px; font-weight: 900; letter-spacing: 8px; color: #0369a1;">
          {email_otp}
        </div>
      </div>
      <p style="margin: 0; font-size: 12px; color: #64748b; line-height: 1.5;">
        Valid for <strong>10 minutes</strong>. Provide this code directly to the admissions officer to authorize viewing your admission files.
      </p>
    </div>
    <div style="border-top: 1px solid #f1f5f9; padding: 16px 24px; background: #fafafa; text-align: center; font-size: 11px; color: #94a3b8;">
      {email_school_name} • Authenticated Document Security
    </div>
  </div>
</body>
</html>"#
        );

        if let Err(e) = crate::utils::email::send_email_with_sender(
            Some(&email_school_name),
            &email_dest,
            &subject,
            &body,
        )
        .await
        {
            tracing::error!(
                "Failed to dispatch officer verification OTP to {}: {}",
                email_dest,
                e
            );
        }
    });

    println!("============================================================");
    println!("🛡️ [OFFICER VERIFICATION OTP DISPATCHED]");
    println!("Application ID: {}", admission_id);
    println!("Application No: {}", app_number);
    println!("Matched Phone: {}", input_10);
    println!("Target Email: {}", target_email);
    println!("Verification Code: {}", otp_str);
    println!("============================================================");

    let masked_email = mask_email(&target_email);
    let masked_phone = format!("******{}", &input_10[input_10.len().saturating_sub(4)..]);

    Ok(Json(json!({
        "success": true,
        "masked_email": masked_email,
        "masked_phone": masked_phone,
        "admission_id": admission_id,
        "application_number": app_number,
        "message": format!("Verification code dispatched to registered email ({})", masked_email)
    })))
}

/// POST /api/v1/admissions/public/officer-verify-otp
/// Confirms the 6-digit email OTP and returns a session verification token for viewing uploaded files.
pub async fn officer_verify_otp(
    State(state): State<AppState>,
    Json(payload): Json<OfficerVerifyOtpPayload>,
) -> Result<impl IntoResponse, AppError> {
    let input_10 = normalize_phone_last_10(&payload.phone);
    let target_key = format!("{}:{}", payload.admission_id, input_10);

    let row = sqlx::query_as::<_, (Uuid, String, chrono::DateTime<Utc>, bool)>(
        r#"
        SELECT id, otp_code, expires_at, verified
        FROM admission_otps
        WHERE target = $1 AND target_type = 'OFFICER_VERIFY'
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&target_key)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest(
            "No active OTP request found for this mobile number. Please request a new code.".into(),
        )
    })?;

    let (otp_id, db_otp, expires_at, _already_verified) = row;

    if Utc::now() > expires_at {
        return Err(AppError::BadRequest(
            "Verification code has expired. Please request a new code.".into(),
        ));
    }

    if db_otp.trim() != payload.otp.trim() {
        return Err(AppError::BadRequest(
            "Incorrect verification code. Please check and try again.".into(),
        ));
    }

    // Generate secure session token (valid for 4 hours)
    let token = format!("ov_{}_{}", Uuid::new_v4().simple(), Utc::now().timestamp());

    sqlx::query(
        r#"
        UPDATE admission_otps
        SET verified = true, verification_token = $1
        WHERE id = $2
        "#,
    )
    .bind(&token)
    .bind(otp_id)
    .execute(&state.db)
    .await?;

    Ok(Json(json!({
        "success": true,
        "verification_token": token,
        "admission_id": payload.admission_id,
        "message": "Authorization verified successfully."
    })))
}

/// GET /api/v1/admissions/public/application/:id/verified-documents?token=...
/// Returns full admission details and all uploaded documents for authenticated officer review.
pub async fn get_officer_verified_documents(
    State(state): State<AppState>,
    Path(admission_id): Path<Uuid>,
    Query(query): Query<OfficerVerifiedDocsQuery>,
) -> Result<impl IntoResponse, AppError> {
    // 1. Authenticate verification token
    let otp_row = sqlx::query_as::<_, (Uuid, String, chrono::DateTime<Utc>, bool)>(
        r#"
        SELECT id, target, created_at, verified
        FROM admission_otps
        WHERE verification_token = $1 AND target_type = 'OFFICER_VERIFY' AND verified = true
        LIMIT 1
        "#,
    )
    .bind(&query.token)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| {
        AppError::Unauthorized(
            "Invalid or expired document verification session. Please verify again.".into(),
        )
    })?;

    let (_otp_id, target, created_at, _verified) = otp_row;

    // Token valid for 4 hours
    if Utc::now() - created_at > chrono::Duration::hours(4) {
        return Err(AppError::Unauthorized(
            "Verification session has expired. Please scan QR code and authenticate again.".into(),
        ));
    }

    let expected_prefix = admission_id.to_string();
    if !target.starts_with(&expected_prefix) {
        return Err(AppError::Unauthorized(
            "Verification token does not match the requested admission application.".into(),
        ));
    }

    // 2. Fetch application and school details
    let adm_row = sqlx::query_as::<
        _,
        (
            Uuid,
            Uuid,
            Option<Uuid>,
            Option<String>,
            String,
            Option<chrono::DateTime<Utc>>,
        ),
    >(
        r#"
        SELECT id, school_id, academic_year_id, application_number, status::text, submitted_at
        FROM admissions
        WHERE id = $1
        "#,
    )
    .bind(admission_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Admission record not found".into()))?;

    let (id, school_id, ay_id, app_no, status, submitted_at) = adm_row;
    let app_number_str = app_no.clone().unwrap_or_else(|| "PENDING".to_string());

    let school = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        r#"
        SELECT name, address_line1, city, phone, email
        FROM schools
        WHERE id = $1
        "#,
    )
    .bind(school_id)
    .fetch_one(&state.db)
    .await?;

    let ay_name: String = if let Some(ay) = ay_id {
        sqlx::query_scalar("SELECT name FROM academic_years WHERE id = $1")
            .bind(ay)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or_else(|| "2026-2027".to_string())
    } else {
        "2026-2027".to_string()
    };

    let applicant = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            String,
            NaiveDate,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        r#"
        SELECT first_name, middle_name, last_name, date_of_birth, gender, phone, email
        FROM admission_applicants
        WHERE admission_id = $1
        LIMIT 1
        "#,
    )
    .bind(admission_id)
    .fetch_optional(&state.db)
    .await?;

    let guardian = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>)>(
        r#"
        SELECT name, relationship, phone, email, occupation
        FROM admission_guardians
        WHERE admission_id = $1
        LIMIT 1
        "#,
    )
    .bind(admission_id)
    .fetch_optional(&state.db)
    .await?;

    // 3. Fetch custom fields and compile documents list
    let custom_values = sqlx::query_as::<_, (String, String, String)>(
        r#"
        SELECT cf.field_name, cf.field_key, acv.value
        FROM admission_custom_values acv
        JOIN custom_fields cf ON cf.id = acv.field_id
        WHERE acv.admission_id = $1
        ORDER BY cf.display_order ASC
        "#,
    )
    .bind(admission_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let custom_fields_json: Vec<serde_json::Value> = custom_values
        .iter()
        .map(|(name, key, val)| {
            json!({
                "name": name,
                "key": key,
                "value": val
            })
        })
        .collect();

    // 4. Extract all uploaded documents
    let mut documents: Vec<serde_json::Value> = Vec::new();
    let mut student_photo_url: Option<String> = None;

    for (field_name, field_key, value) in &custom_values {
        let val_trimmed = value.trim();
        let is_url_or_b64 =
            val_trimmed.starts_with("http") || val_trimmed.starts_with("data:image");
        let is_doc_key = field_key.starts_with("doc_")
            || field_key.contains("document")
            || field_key.contains("photo")
            || field_key.contains("cert")
            || field_key.contains("proof");

        if is_doc_key || is_url_or_b64 {
            let key_lower = field_key.to_lowercase();
            let is_photo = key_lower.contains("photo") || key_lower.contains("passport");

            if is_photo && student_photo_url.is_none() && is_url_or_b64 {
                student_photo_url = Some(val_trimmed.to_string());
            }

            // Derive human-friendly title
            let title = if key_lower.contains("photo") || key_lower.contains("passport") {
                "Student Passport Photograph".to_string()
            } else if key_lower.contains("birth") {
                "Birth Certificate".to_string()
            } else if key_lower.contains("address") || key_lower.contains("aadhaar") {
                "Address & Identification Proof".to_string()
            } else if key_lower.contains("report") || key_lower.contains("mark") {
                "Previous Academic Report Card".to_string()
            } else if key_lower.contains("tc") || key_lower.contains("transfer") {
                "Transfer Certificate (TC)".to_string()
            } else if key_lower.contains("medical") {
                "Medical Fitness Certificate".to_string()
            } else if key_lower.contains("caste") || key_lower.contains("category") {
                "Caste / Category Certificate".to_string()
            } else {
                field_name.replace("Document: ", "")
            };

            let is_pdf = val_trimmed.to_lowercase().contains(".pdf");
            let file_name = if is_pdf {
                format!("{}.pdf", title.replace(['/', ' '], "_"))
            } else {
                format!("{}.jpg", title.replace(['/', ' '], "_"))
            };

            documents.push(json!({
                "id": field_key,
                "title": title,
                "url": val_trimmed,
                "file_name": file_name,
                "file_type": if is_pdf { "pdf" } else { "image" },
                "is_photo": is_photo
            }));
        }
    }

    // Also check storage_objects table if available
    let storage_rows = sqlx::query_as::<_, (String, String, String, String)>(
        r#"
        SELECT file_name, file_path, url, mime_type
        FROM storage_objects
        WHERE entity_id = $1::text
        ORDER BY created_at DESC
        "#,
    )
    .bind(admission_id.to_string())
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    for (f_name, _path, url, mime) in storage_rows {
        if !documents.iter().any(|d| d["url"] == url) {
            let is_pdf = mime.contains("pdf") || url.ends_with(".pdf");
            let is_photo = f_name.to_lowercase().contains("photo") || mime.contains("image");
            documents.push(json!({
                "id": format!("storage_{}", Uuid::new_v4().simple()),
                "title": f_name.replace('_', " "),
                "url": url,
                "file_name": f_name,
                "file_type": if is_pdf { "pdf" } else { "image" },
                "is_photo": is_photo
            }));
        }
    }

    // Generate Verification QR Code SVG
    let app_url = std::env::var("APP_URL")
        .unwrap_or_else(|_| "https://my-frontend-kappa-five.vercel.app".to_string());
    let clean_url = app_url.trim_end_matches('/');
    let verify_url =
        format!("{clean_url}/admissions/verify?id={admission_id}&app_no={app_number_str}");

    let qr_code_svg = if let Ok(code) = QrCode::new(verify_url.as_bytes()) {
        code.render::<svg::Color>().min_dimensions(120, 120).build()
    } else {
        String::new()
    };

    let template_config = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM school_settings WHERE school_id = $1 AND key = 'report_tpl_admission_form'",
    )
    .bind(school_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let adm_setting_val = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM school_settings WHERE school_id = $1 AND key = 'admissions_config'",
    )
    .bind(school_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let required_docs = adm_setting_val
        .and_then(|v| v.get("required_docs").cloned())
        .unwrap_or_else(|| {
            json!([
                { "id": "birth_cert", "label": "Birth Certificate", "required": true },
                { "id": "prev_report", "label": "Previous Year Report Card", "required": true },
                { "id": "transfer_cert", "label": "Transfer Certificate (TC)", "required": false },
                { "id": "photo", "label": "Passport-size Photographs (4 copies)", "required": true },
                { "id": "address_proof", "label": "Address Proof (Aadhaar / Utility Bill)", "required": true },
                { "id": "caste_cert", "label": "Caste / Category Certificate", "required": false },
                { "id": "medical_fit", "label": "Medical Fitness Certificate", "required": false }
            ])
        });

    let application_data = json!({
        "id": id,
        "school_id": school_id,
        "school_name": school.0,
        "school_address": school.1.unwrap_or_else(|| "Campus Road, Knowledge Park".to_string()),
        "school_city": school.2.unwrap_or_default(),
        "school_phone": school.3.unwrap_or_default(),
        "school_email": school.4.unwrap_or_default(),
        "academic_year": ay_name,
        "application_number": app_number_str,
        "status": status,
        "submitted_at": submitted_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
        "applicant": applicant.map(|a| json!({
            "first_name": a.0,
            "middle_name": a.1.unwrap_or_default(),
            "last_name": a.2,
            "date_of_birth": a.3.to_string(),
            "gender": a.4.unwrap_or_else(|| "Male".to_string()),
            "phone": a.5.unwrap_or_default(),
            "email": a.6.unwrap_or_default()
        })),
        "guardian": guardian.map(|g| json!({
            "name": g.0,
            "relationship": g.1,
            "phone": g.2,
            "email": g.3.unwrap_or_default(),
            "occupation": g.4.unwrap_or_default()
        })),
        "custom_fields": custom_fields_json,
        "template_config": template_config,
        "required_docs": required_docs,
        "qr_code_svg": qr_code_svg,
        "verify_url": verify_url
    });

    Ok(Json(json!({
        "success": true,
        "admission_id": admission_id,
        "student_photo_url": student_photo_url,
        "application": application_data,
        "documents": documents
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_captcha_token_encoding_and_validation() {
        let a = 12;
        let b = 8;
        let sum = a + b;
        let salt = Uuid::new_v4().to_string();
        let token = format!("{salt}:{a}:{b}:{sum}");
        let encoded_token = hex::encode(token.as_bytes());

        // Validate decoding logic
        let decoded = hex::decode(&encoded_token).expect("hex decode");
        let decoded_str = String::from_utf8(decoded).expect("utf8");
        let parts: Vec<&str> = decoded_str.split(':').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[3], "20");
    }

    #[test]
    fn test_phone_number_cleaning_and_last_10_extraction() {
        let phone = "+91 98765 43210";
        let clean: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
        assert_eq!(clean, "919876543210");
        let phone_10 = if clean.len() > 10 {
            clean[clean.len() - 10..].to_string()
        } else {
            clean.clone()
        };
        assert_eq!(phone_10, "9876543210");
    }

    #[test]
    fn test_school_address_concat_ws_formatting() {
        let addr1: Option<&str> = Some("123 Campus Road");
        let addr2: Option<&str> = Some("Knowledge Park Phase 2");
        let parts: Vec<&str> = [addr1, addr2].into_iter().flatten().collect();
        let formatted = parts.join(", ");
        assert_eq!(formatted, "123 Campus Road, Knowledge Park Phase 2");

        let addr1_only: Option<&str> = Some("Central Avenue");
        let addr2_none: Option<&str> = None;
        let parts2: Vec<&str> = [addr1_only, addr2_none].into_iter().flatten().collect();
        assert_eq!(parts2.join(", "), "Central Avenue");
    }

    #[test]
    fn test_officer_verification_phone_normalization_and_email_masking() {
        assert_eq!(normalize_phone_last_10("+91 96054 49544"), "9605449544");
        assert_eq!(normalize_phone_last_10("09605449544"), "9605449544");
        assert_eq!(normalize_phone_last_10("9605449544"), "9605449544");
        assert_eq!(normalize_phone_last_10("(+91) 987-654-3210"), "9876543210");

        assert_eq!(mask_email("john.doe@gmail.com"), "j***e@gmail.com");
        assert_eq!(mask_email("ab@eraya.edu"), "a***@eraya.edu");
        assert_eq!(mask_email("officer@school.org"), "o***r@school.org");
    }
}
