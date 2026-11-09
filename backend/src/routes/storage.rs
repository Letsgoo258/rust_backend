use crate::error::AppError;
use crate::state::AppState;
use crate::utils::imagekit::{ImageKitClient, ImageKitUploadOptions};
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/upload", post(upload_asset))
        .route("/:file_id", delete(delete_asset))
        .route("/signed-url", get(get_signed_url))
        .route("/view/:id", get(view_asset))
        .route("/migrate-legacy-assets", post(migrate_legacy_assets))
}

#[derive(Debug, Deserialize)]
pub struct StorageUploadPayload {
    pub file_base64: String,
    pub file_name: String,
    pub folder: Option<String>,
    pub mime_type: Option<String>,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub encrypt: Option<bool>,
    pub is_private: Option<bool>,
    pub retention_days: Option<i64>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct SignedUrlQuery {
    pub url: String,
    pub expire_seconds: Option<u64>,
}

/// Uploads an asset (logo, header banner, photo, document) to ImageKit with optional encryption
pub async fn upload_asset(
    State(state): State<AppState>,
    Json(payload): Json<StorageUploadPayload>,
) -> Result<Json<Value>, AppError> {
    let clean_b64 = if let Some(idx) = payload.file_base64.find(";base64,") {
        &payload.file_base64[idx + 8..]
    } else {
        &payload.file_base64
    };

    let file_bytes = base64::engine::general_purpose::STANDARD
        .decode(clean_b64.trim())
        .map_err(|e| AppError::BadRequest(format!("Invalid base64 payload: {e}")))?;

    if file_bytes.is_empty() {
        return Err(AppError::BadRequest("File cannot be empty".to_string()));
    }

    let opts = ImageKitUploadOptions {
        file_name: payload.file_name,
        folder: payload.folder.unwrap_or_else(|| "/general".to_string()),
        mime_type: payload.mime_type,
        tags: payload.tags.unwrap_or_default(),
        use_unique_file_name: false,
        is_private: payload.is_private.unwrap_or(false),
        encrypt: payload.encrypt.unwrap_or(false),
        retention_days: payload.retention_days,
        school_id: None,
        entity_type: payload.entity_type,
        entity_id: payload.entity_id,
    };

    let result = ImageKitClient::upload_file(file_bytes, opts, Some(&state.db)).await?;

    Ok(Json(json!({
        "success": true,
        "result": result
    })))
}

/// Deletes an asset from ImageKit and updates database ledger
pub async fn delete_asset(
    State(state): State<AppState>,
    Path(file_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    ImageKitClient::delete_file(&file_id, Some(&state.db)).await?;

    Ok(Json(json!({
        "success": true,
        "message": format!("Asset {file_id} deleted successfully from ImageKit and storage ledger")
    })))
}

/// Generates ImageKit-compliant signed URL for secure viewing
pub async fn get_signed_url(Query(params): Query<SignedUrlQuery>) -> Result<Json<Value>, AppError> {
    let private_key = ImageKitClient::get_private_key();
    let expire_seconds = params.expire_seconds.unwrap_or(86400); // 24 hours default
    let (signed_url, expires_at) =
        ImageKitClient::sign_url(&params.url, &private_key, expire_seconds);

    Ok(Json(json!({
        "success": true,
        "signed_url": signed_url,
        "expires_at": expires_at
    })))
}

/// Streams decrypted asset or returns signed URL for authorized viewing
pub async fn view_asset(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    #[derive(sqlx::FromRow)]
    struct AssetRecord {
        url: String,
        mime_type: String,
        is_encrypted: bool,
        encryption_iv: Option<String>,
        file_name: String,
    }

    let record = sqlx::query_as::<_, AssetRecord>(
        r#"
        SELECT url, mime_type, is_encrypted, encryption_iv, file_name
        FROM storage_objects
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Storage object {id} not found")))?;

    if !record.is_encrypted {
        let private_key = ImageKitClient::get_private_key();
        let (signed_url, _) = ImageKitClient::sign_url(&record.url, &private_key, 3600);
        return Ok((StatusCode::FOUND, [(header::LOCATION, signed_url)]).into_response());
    }

    // Encrypted Asset: Fetch ciphertext from remote URL, decrypt, and stream
    let client = reqwest::Client::new();
    let ciphertext = client
        .get(&record.url)
        .send()
        .await
        .map_err(|e| {
            AppError::InternalServerError(format!("Failed to fetch encrypted asset: {e}"))
        })?
        .bytes()
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to read asset bytes: {e}")))?;

    let iv = record
        .encryption_iv
        .ok_or_else(|| AppError::InternalServerError("Missing encryption IV".to_string()))?;
    let key = ImageKitClient::get_encryption_key();
    let plaintext = ImageKitClient::decrypt_content(&ciphertext, &iv, &key)?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, record.mime_type),
            (
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{}\"", record.file_name),
            ),
        ],
        plaintext,
    )
        .into_response())
}

/// Administrative migration: Scans school settings for legacy base64 images,
/// uploads them to ImageKit, updates records with CDN URLs, and purges base64 blobs
pub async fn migrate_legacy_assets(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let report = run_legacy_asset_migration(&state.db).await?;
    Ok(Json(json!({
        "success": true,
        "migration_report": report
    })))
}

#[derive(Debug, Serialize)]
pub struct MigrationReport {
    pub migrated_logos: usize,
    pub imagekit_logo_url: Option<String>,
    pub settings_updated: usize,
    pub bytes_freed: usize,
}

pub async fn run_legacy_asset_migration(pool: &sqlx::PgPool) -> Result<MigrationReport, AppError> {
    // 1. Fetch school_settings containing base64 images
    let rows: Vec<(Uuid, String, Value)> = sqlx::query_as(
        r#"
        SELECT id, key, value FROM school_settings
        WHERE value::text LIKE '%data:image%'
        "#,
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok(MigrationReport {
            migrated_logos: 0,
            imagekit_logo_url: None,
            settings_updated: 0,
            bytes_freed: 0,
        });
    }

    let mut uploaded_logo_url: Option<String> = None;
    let mut total_bytes_freed: usize = 0;
    let mut settings_updated: usize = 0;

    // 2. Extract first valid base64 image (the school crest logo)
    for (_, _, val) in &rows {
        if let Some(logo_str) = val
            .get("config")
            .and_then(|c| c.get("logo_url"))
            .and_then(|u| u.as_str())
        {
            if logo_str.starts_with("data:image/") {
                if let Some(idx) = logo_str.find(";base64,") {
                    let clean_b64 = &logo_str[idx + 8..];
                    if let Ok(bytes) =
                        base64::engine::general_purpose::STANDARD.decode(clean_b64.trim())
                    {
                        let opts = ImageKitUploadOptions {
                            file_name: "school_crest_logo.png".to_string(),
                            folder: "/school/branding".to_string(),
                            mime_type: Some("image/png".to_string()),
                            tags: vec!["school_logo".to_string(), "branding".to_string()],
                            use_unique_file_name: false,
                            is_private: false,
                            encrypt: false,
                            retention_days: None,
                            school_id: None,
                            entity_type: Some("SCHOOL_LOGO".to_string()),
                            entity_id: Some("master_branding".to_string()),
                        };

                        let res = ImageKitClient::upload_file(bytes, opts, Some(pool)).await?;
                        uploaded_logo_url = Some(res.url);
                        break;
                    }
                }
            }
        }
    }

    // 3. If uploaded or already known, replace all occurrences in database
    if let Some(ref cdn_url) = uploaded_logo_url {
        for (id, key, mut val) in rows {
            let before_len = serde_json::to_string(&val).unwrap_or_default().len();

            // Replace inside config.logo_url
            if let Some(cfg) = val.get_mut("config") {
                if let Some(logo_url) = cfg.get_mut("logo_url") {
                    if let Some(s) = logo_url.as_str() {
                        if s.starts_with("data:image/") {
                            *logo_url = Value::String(cdn_url.clone());
                        }
                    }
                }
                if let Some(header) = cfg.get_mut("header") {
                    if let Some(logo_url) = header.get_mut("logoUrl") {
                        if let Some(s) = logo_url.as_str() {
                            if s.starts_with("data:image/") {
                                *logo_url = Value::String(cdn_url.clone());
                            }
                        }
                    }
                }
            }

            // Replace inside snapshot array if version history table
            if let Some(arr) = val.as_array_mut() {
                for item in arr {
                    if let Some(snap) = item.get_mut("snapshot") {
                        if let Some(logo_url) = snap.get_mut("logo_url") {
                            if let Some(s) = logo_url.as_str() {
                                if s.starts_with("data:image/") {
                                    *logo_url = Value::String(cdn_url.clone());
                                }
                            }
                        }
                        if let Some(header) = snap.get_mut("header") {
                            if let Some(logo_url) = header.get_mut("logoUrl") {
                                if let Some(s) = logo_url.as_str() {
                                    if s.starts_with("data:image/") {
                                        *logo_url = Value::String(cdn_url.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let after_len = serde_json::to_string(&val).unwrap_or_default().len();
            if before_len > after_len {
                total_bytes_freed += before_len - after_len;
            }

            // Save clean record
            sqlx::query(
                r#"
                UPDATE school_settings
                SET value = $1, updated_at = NOW()
                WHERE id = $2
                "#,
            )
            .bind(&val)
            .bind(id)
            .execute(pool)
            .await?;

            settings_updated += 1;
            tracing::info!(key = %key, freed = %(before_len.saturating_sub(after_len)), "Purged legacy base64 image from settings");
        }
    }

    // 4. Delete any temporary files from local disk
    let local_upload_dir = std::path::Path::new("uploads");
    if local_upload_dir.exists() {
        let _ = tokio::fs::remove_dir_all(local_upload_dir).await;
        tracing::info!("Removed legacy local uploads directory from disk");
    }

    Ok(MigrationReport {
        migrated_logos: if uploaded_logo_url.is_some() { 1 } else { 0 },
        imagekit_logo_url: uploaded_logo_url,
        settings_updated,
        bytes_freed: total_bytes_freed,
    })
}
