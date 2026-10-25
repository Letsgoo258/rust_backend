use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{Datelike, NaiveDate, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    error::AppError,
    routes::settings::admissions::{AdmissionConfigPayload, RequiredDocItem},
    state::AppState,
    utils::{datetime::DateUtils, whatsapp::send_whatsapp_message},
};

#[derive(Debug, Deserialize)]
pub struct SendOtpPayload {
    pub target: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyOtpPayload {
    pub target: String,
    pub otp: String,
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
        .route("/send-phone-otp", post(send_phone_otp))
        .route("/verify-phone-otp", post(verify_phone_otp))
        .route("/send-email-otp", post(send_email_otp))
        .route("/verify-email-otp", post(verify_email_otp))
        .route("/submit", post(submit_admission_application))
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
        WHERE school_id = $1 AND entity_type IN ('ADMISSION', 'STUDENT')
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
        "custom_fields": custom_fields
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

/// POST /api/v1/admissions/public/send-phone-otp
pub async fn send_phone_otp(
    State(state): State<AppState>,
    Json(payload): Json<SendOtpPayload>,
) -> Result<impl IntoResponse, AppError> {
    let clean_phone: String = payload
        .target
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    if clean_phone.len() < 10 {
        return Err(AppError::BadRequest(
            "Please provide a valid 10-digit mobile number".into(),
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
        VALUES ($1, 'PHONE', $2, now() + INTERVAL '10 minutes')
        "#,
    )
    .bind(&clean_phone)
    .bind(&otp_str)
    .execute(&state.db)
    .await?;

    // Attempt to dispatch via connected WhatsApp bot directly
    let jid = format!("{clean_phone}@s.whatsapp.net");
    let msg = format!(
        "🔐 *ERAVAYA Admission Verification*\n\nYour one-time phone verification code is: *{otp_str}*\n\nThis code expires in 10 minutes. Please enter it on the admission application page."
    );
    let _ = send_whatsapp_message(&state.db, &jid, &msg, None).await;

    tracing::info!(phone = %clean_phone, otp = %otp_str, "Dispatched admission phone OTP");

    Ok(Json(json!({
        "success": true,
        "message": "OTP has been sent to your mobile number via WhatsApp",
    })))
}

/// POST /api/v1/admissions/public/verify-phone-otp
pub async fn verify_phone_otp(
    State(state): State<AppState>,
    Json(payload): Json<VerifyOtpPayload>,
) -> Result<impl IntoResponse, AppError> {
    let clean_phone: String = payload
        .target
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();

    let otp_row = sqlx::query_as::<_, (Uuid, String, chrono::DateTime<Utc>)>(
        r#"
        SELECT id, otp_code, expires_at
        FROM admission_otps
        WHERE target = $1 AND target_type = 'PHONE' AND verified = false
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&clean_phone)
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

    tracing::info!(email = %clean_email, otp = %otp_str, "Dispatched admission email OTP");

    Ok(Json(json!({
        "success": true,
        "message": "Verification code has been generated for your email.",
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
    let phone_verified = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT verified
        FROM admission_otps
        WHERE target = $1 AND target_type = 'PHONE' AND verification_token = $2
        "#,
    )
    .bind(&clean_phone)
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
    let email_verified = sqlx::query_scalar::<_, bool>(
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
    .unwrap_or(false);

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

        if let Some(fid) = field_id {
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
    }

    tx.commit().await?;

    // 8. Dispatch Confirmation WhatsApp Message
    let applicant_name = format!(
        "{} {}",
        payload.applicant_first_name, payload.applicant_last_name
    );
    let jid = format!("{clean_phone}@s.whatsapp.net");
    let confirm_msg = format!(
        "🎉 *ERAVAYA Admission Application Submitted!*\n\n\
        Dear *{}*,\n\
        We have received the admission application for *{}* at *{}*.\n\n\
        📋 *Application Number*: *{}*\n\
        📅 *Academic Year*: *{}*\n\
        📌 *Status*: Submitted & Under Review\n\n\
        Our admissions desk has been notified. You can use your application number for any future updates or inquiries.",
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
