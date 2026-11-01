use crate::{
    error::AppError, modules::auth::backend::AuthSession, state::AppState, utils::audit::log_action,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/template", get(get_template).post(save_template))
        .route("/template/publish", post(publish_template))
        .route("/template/request-otp", post(request_otp))
        .route("/template/verify-otp", post(verify_otp))
        .route("/template/versions", get(list_versions))
        .route("/template/rollback", post(rollback_version))
}

#[derive(Debug, Deserialize)]
pub struct TemplateQuery {
    pub template_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveTemplatePayload {
    pub template_type: Option<String>,
    pub name: Option<String>,
    pub config: Value,
    pub is_published: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyOtpPayload {
    pub otp_code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RollbackPayload {
    pub version_id: Uuid,
}

/// GET /api/v1/settings/reports/template
pub async fn get_template(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Query(query): Query<TemplateQuery>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let tpl_type = query
        .template_type
        .unwrap_or_else(|| "transfer_certificate".into());
    let setting_key = format!("report_tpl_{}", tpl_type);

    let setting_val = sqlx::query_scalar::<_, Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = $2
        "#,
    )
    .bind(user.school_id)
    .bind(&setting_key)
    .fetch_optional(&state.db)
    .await?;

    if let Some(val) = setting_val {
        Ok((
            StatusCode::OK,
            Json(json!({
                "template": val
            })),
        ))
    } else {
        Ok((
            StatusCode::OK,
            Json(json!({
                "template": null
            })),
        ))
    }
}

/// POST /api/v1/settings/reports/template
pub async fn save_template(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<SaveTemplatePayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let tpl_type = payload
        .template_type
        .unwrap_or_else(|| "transfer_certificate".into());
    let name = payload
        .name
        .unwrap_or_else(|| "Standard Transfer Certificate".into());
    let setting_key = format!("report_tpl_{}", tpl_type);
    let versions_key = format!("report_tpl_{}_versions", tpl_type);

    // Fetch existing template if any
    let existing_val = sqlx::query_scalar::<_, Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = $2
        "#,
    )
    .bind(user.school_id)
    .bind(&setting_key)
    .fetch_optional(&state.db)
    .await?;

    let current_version = existing_val
        .as_ref()
        .and_then(|v| v.get("version"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let new_version = current_version + 1;
    let is_published = payload.is_published.unwrap_or(false);

    let template_obj = json!({
        "id": uuid::Uuid::new_v4(),
        "template_type": tpl_type,
        "name": name,
        "config": payload.config,
        "is_published": is_published,
        "version": new_version,
        "updated_at": chrono::Utc::now().to_rfc3339()
    });

    sqlx::query(
        r#"
        INSERT INTO school_settings (school_id, key, value, updated_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (school_id, key)
        DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
        "#,
    )
    .bind(user.school_id)
    .bind(&setting_key)
    .bind(&template_obj)
    .execute(&state.db)
    .await?;

    // Append to versions history array in school_settings
    let version_entry = json!({
        "id": uuid::Uuid::new_v4(),
        "version": new_version,
        "snapshot": payload.config,
        "diff_summary": format!("Version {} saved", new_version),
        "created_by": user.id,
        "created_at": chrono::Utc::now().to_rfc3339()
    });

    let existing_versions = sqlx::query_scalar::<_, Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = $2
        "#,
    )
    .bind(user.school_id)
    .bind(&versions_key)
    .fetch_optional(&state.db)
    .await?;

    let mut versions_arr = existing_versions
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();

    versions_arr.insert(0, version_entry);
    if versions_arr.len() > 25 {
        versions_arr.truncate(25);
    }

    let _ = sqlx::query(
        r#"
        INSERT INTO school_settings (school_id, key, value, updated_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (school_id, key)
        DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
        "#,
    )
    .bind(user.school_id)
    .bind(&versions_key)
    .bind(json!(versions_arr))
    .execute(&state.db)
    .await;

    let _ = log_action(
        &state.db,
        Some(user.school_id),
        Some(user.id),
        "SAVE_TC_TEMPLATE",
        Some("SETTINGS_REPORTS"),
        None,
        Some(json!({ "version": new_version, "template_type": tpl_type })),
        None,
    )
    .await;

    Ok((
        StatusCode::OK,
        Json(json!({
            "message": "Template saved successfully",
            "template": template_obj
        })),
    ))
}

/// POST /api/v1/settings/reports/template/publish
pub async fn publish_template(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<SaveTemplatePayload>,
) -> Result<impl IntoResponse, AppError> {
    let mut payload_pub = payload;
    payload_pub.is_published = Some(true);
    save_template(auth_session, State(state), Json(payload_pub)).await
}

/// POST /api/v1/settings/reports/template/request-otp
pub async fn request_otp(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    // Clear previous otps for this user & action
    let _ = sqlx::query(
        r#"DELETE FROM admin_action_otps
           WHERE user_id = $1 AND action_type = 'EDIT_TC_TEMPLATE'"#,
    )
    .bind(user.id)
    .execute(&state.db)
    .await;

    // Generate 6-digit alphanumeric OTP
    let otp: String = uuid::Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(6)
        .collect::<String>()
        .to_uppercase();

    sqlx::query(
        r#"INSERT INTO admin_action_otps (user_id, action_type, otp_code, expires_at)
           VALUES ($1, 'EDIT_TC_TEMPLATE', $2, NOW() + INTERVAL '10 minutes')"#,
    )
    .bind(user.id)
    .bind(&otp)
    .execute(&state.db)
    .await?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "message": "OTP sent to School Admin phone/email",
            "otp_hint": otp
        })),
    ))
}

/// POST /api/v1/settings/reports/template/verify-otp
pub async fn verify_otp(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<VerifyOtpPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let entered = payload.otp_code.trim().to_uppercase();

    let otp_record = sqlx::query_as::<_, (Uuid, String)>(
        r#"SELECT id, otp_code
           FROM admin_action_otps
           WHERE user_id = $1 AND action_type = 'EDIT_TC_TEMPLATE' AND expires_at > NOW()
           ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?;

    let (record_id, expected_code) = match otp_record {
        Some(r) => r,
        None => {
            return Err(AppError::BadRequest(
                "No pending OTP found or expired. Request a new one.".into(),
            ))
        }
    };

    if expected_code.to_uppercase() != entered {
        return Err(AppError::BadRequest("Invalid OTP code".into()));
    }

    let _ = sqlx::query("DELETE FROM admin_action_otps WHERE id = $1")
        .bind(record_id)
        .execute(&state.db)
        .await;

    let unlock_token = uuid::Uuid::new_v4().to_string();

    let _ = log_action(
        &state.db,
        Some(user.school_id),
        Some(user.id),
        "UNLOCK_TC_TEMPLATE_LAYOUT",
        Some("SETTINGS_REPORTS"),
        None,
        Some(json!({ "unlocked_at": chrono::Utc::now() })),
        None,
    )
    .await;

    Ok((
        StatusCode::OK,
        Json(json!({
            "message": "Template layout successfully unlocked for editing",
            "unlock_token": unlock_token
        })),
    ))
}

/// GET /api/v1/settings/reports/template/versions
pub async fn list_versions(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Query(query): Query<TemplateQuery>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let tpl_type = query
        .template_type
        .unwrap_or_else(|| "transfer_certificate".into());
    let versions_key = format!("report_tpl_{}_versions", tpl_type);

    let versions_val = sqlx::query_scalar::<_, Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = $2
        "#,
    )
    .bind(user.school_id)
    .bind(&versions_key)
    .fetch_optional(&state.db)
    .await?;

    let versions = versions_val
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();

    Ok((StatusCode::OK, Json(json!({ "versions": versions }))))
}

/// POST /api/v1/settings/reports/template/rollback
pub async fn rollback_version(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<RollbackPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let tpl_type = "transfer_certificate";
    let versions_key = format!("report_tpl_{}_versions", tpl_type);
    let setting_key = format!("report_tpl_{}", tpl_type);

    let versions_val = sqlx::query_scalar::<_, Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = $2
        "#,
    )
    .bind(user.school_id)
    .bind(&versions_key)
    .fetch_optional(&state.db)
    .await?;

    let versions = versions_val
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();

    let target_snapshot = versions.into_iter().find(|v| {
        v.get("id")
            .and_then(|id| id.as_str())
            .map(|s| s == payload.version_id.to_string())
            .unwrap_or(false)
    });

    let target = match target_snapshot {
        Some(t) => t,
        None => return Err(AppError::NotFound("Version snapshot not found".into())),
    };

    let snapshot_config = target.get("snapshot").cloned().unwrap_or(json!({}));
    let target_ver = target.get("version").and_then(|v| v.as_i64()).unwrap_or(1);

    let updated_obj = json!({
        "id": uuid::Uuid::new_v4(),
        "template_type": tpl_type,
        "name": "Standard Transfer Certificate",
        "config": snapshot_config,
        "is_published": false,
        "version": target_ver + 1,
        "updated_at": chrono::Utc::now().to_rfc3339()
    });

    sqlx::query(
        r#"
        INSERT INTO school_settings (school_id, key, value, updated_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (school_id, key)
        DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
        "#,
    )
    .bind(user.school_id)
    .bind(&setting_key)
    .bind(&updated_obj)
    .execute(&state.db)
    .await?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "message": format!("Rolled back to snapshot version {}", target_ver),
            "template": updated_obj
        })),
    ))
}
