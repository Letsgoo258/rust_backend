use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequiredDocItem {
    pub id: String,
    pub label: String,
    pub required: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AdmissionConfigPayload {
    pub academic_year_id: Option<Uuid>,
    pub academic_year_name: Option<String>,
    pub prefix: String,
    pub separator: String,
    pub start_number: String,
    pub year_in_format: bool,
    pub intake_capacity: i32,
    pub open_date: String,  // YYYY-MM-DD
    pub close_date: String, // YYYY-MM-DD
    pub required_docs: Vec<RequiredDocItem>,
    pub is_open: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AcademicYearOption {
    pub id: Uuid,
    pub name: String,
    pub start_date: String,
    pub end_date: String,
    pub status: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/",
        get(get_admission_settings).post(save_admission_settings),
    )
}

/// GET /api/v1/settings/admissions
/// Returns admission settings for the school and active academic years.
pub async fn get_admission_settings(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    // 1. Fetch academic years
    let ay_rows = sqlx::query_as::<_, (Uuid, String, time::Date, time::Date, String)>(
        r#"
        SELECT id, name, start_date, end_date, status
        FROM academic_years
        WHERE school_id = $1
        ORDER BY start_date DESC
        "#,
    )
    .bind(user.school_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let academic_years: Vec<AcademicYearOption> = ay_rows
        .into_iter()
        .map(|(id, name, start, end, status)| AcademicYearOption {
            id,
            name,
            start_date: start.to_string(),
            end_date: end.to_string(),
            status,
        })
        .collect();

    let active_ay = academic_years
        .iter()
        .find(|ay| ay.status == "ACTIVE")
        .or_else(|| academic_years.first());

    // 2. Fetch saved admission settings
    let setting_val = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = 'admission_settings'
        "#,
    )
    .bind(user.school_id)
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

    let config: AdmissionConfigPayload = if let Some(val) = setting_val {
        serde_json::from_value(val).unwrap_or_else(|_| AdmissionConfigPayload {
            academic_year_id: active_ay.map(|a| a.id),
            academic_year_name: active_ay.map(|a| a.name.clone()).or(Some(default_ay_name)),
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
            academic_year_id: active_ay.map(|a| a.id),
            academic_year_name: active_ay.map(|a| a.name.clone()).or(Some(default_ay_name)),
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

    // Calculate dynamic in-date status
    let today = Utc::now().date_naive();
    let is_in_date = match (
        NaiveDate::parse_from_str(&config.open_date, "%Y-%m-%d"),
        NaiveDate::parse_from_str(&config.close_date, "%Y-%m-%d"),
    ) {
        (Ok(open), Ok(close)) => today >= open && today <= close,
        _ => true,
    };

    Ok(Json(json!({
        "success": true,
        "settings": config,
        "is_window_open": is_in_date,
        "academic_years": academic_years
    })))
}

/// POST /api/v1/settings/admissions
/// Save or update school admission settings.
pub async fn save_admission_settings(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<AdmissionConfigPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let is_admin = user.roles.iter().any(|r| {
        r == "SUPER_ADMIN" || r == "SCHOOL_ADMIN" || r == "PRINCIPAL" || r == "HEADMASTER"
    });

    if !is_admin {
        return Err(AppError::Forbidden(
            "Only administrators can update admission settings".into(),
        ));
    }

    let val_json = serde_json::to_value(&payload)
        .map_err(|e| AppError::BadRequest(format!("Invalid config payload: {e}")))?;

    sqlx::query(
        r#"
        INSERT INTO school_settings (school_id, key, value, updated_at)
        VALUES ($1, 'admission_settings', $2, now())
        ON CONFLICT (school_id, key)
        DO UPDATE SET value = EXCLUDED.value, updated_at = now()
        "#,
    )
    .bind(user.school_id)
    .bind(val_json)
    .execute(&state.db)
    .await?;

    tracing::info!(
        school_id = %user.school_id,
        user_id = %user.id,
        academic_year = ?payload.academic_year_name,
        "Updated school admission settings"
    );

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Admission settings saved successfully"
        })),
    ))
}
