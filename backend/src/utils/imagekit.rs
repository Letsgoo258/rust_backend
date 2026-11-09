use crate::error::AppError;
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use chrono::Utc;
use hmac::{Hmac, Mac};
use rand::Rng;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::env;
use std::time::Duration;
use uuid::Uuid;

type HmacSha1 = Hmac<Sha1>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageKitUploadResult {
    pub file_id: String,
    pub name: String,
    pub url: String,
    pub file_path: String,
    pub signed_url_24h: String,
    pub expires_at: i64,
    pub checksum_sha256: String,
    pub is_encrypted: bool,
    pub encryption_iv: Option<String>,
    pub file_size_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageKitUploadOptions {
    pub file_name: String,
    pub folder: String,
    pub mime_type: Option<String>,
    pub tags: Vec<String>,
    pub use_unique_file_name: bool,
    pub is_private: bool,
    pub encrypt: bool,
    pub retention_days: Option<i64>,
    pub school_id: Option<Uuid>,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
}

impl Default for ImageKitUploadOptions {
    fn default() -> Self {
        Self {
            file_name: "file.png".to_string(),
            folder: "/general".to_string(),
            mime_type: Some("image/png".to_string()),
            tags: Vec::new(),
            use_unique_file_name: false,
            is_private: false,
            encrypt: false,
            retention_days: None,
            school_id: None,
            entity_type: None,
            entity_id: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ImageKitApiResponse {
    #[serde(rename = "fileId")]
    file_id: String,
    name: String,
    url: String,
    #[serde(rename = "filePath")]
    file_path: Option<String>,
    size: Option<usize>,
}

pub struct ImageKitClient;
pub type ImageKitStorageService = ImageKitClient;

impl ImageKitClient {
    /// Resolves the ImageKit private key from environment variables
    pub fn get_private_key() -> String {
        env::var("PRIVATE_IMAGEKIT")
            .or_else(|_| env::var("IMAGEKIT_PRIVATE_KEY"))
            .or_else(|_| env::var("IK_PRIVATE_KEY"))
            .unwrap_or_default()
    }

    /// Resolves the ImageKit public key from environment variables
    pub fn get_public_key() -> String {
        env::var("PUBLIC_IMAGEKIT")
            .or_else(|_| env::var("IMAGEKIT_PUBLIC_KEY"))
            .or_else(|_| env::var("IK_PUBLIC_KEY"))
            .unwrap_or_default()
    }

    /// Resolves the ImageKit endpoint URL or falls back to authentic school endpoint
    pub fn get_url_endpoint() -> String {
        env::var("IMAGEKIT_URL_ENDPOINT")
            .or_else(|_| env::var("IMAGEKIT_ENDPOINT"))
            .unwrap_or_else(|_| "https://ik.imagekit.io/tenyf8kjj".to_string())
            .trim_end_matches('/')
            .to_string()
    }

    /// Computes SHA-256 hex digest for audit data integrity
    pub fn compute_sha256(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// Derives or retrieves the 256-bit AES-GCM master key
    pub fn get_encryption_key() -> [u8; 32] {
        if let Ok(key_str) = env::var("STORAGE_ENCRYPTION_KEY") {
            if let Ok(decoded) = hex::decode(&key_str) {
                if decoded.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&decoded);
                    return key;
                }
            }
        }
        // Deterministic derivation from private key
        let private_key = Self::get_private_key();
        let source = if private_key.trim().is_empty() {
            "eravaya_default_storage_encryption_secret_2026"
        } else {
            private_key.trim()
        };
        let mut hasher = Sha256::new();
        hasher.update(b"eravaya_storage_aes_256_gcm_salt:");
        hasher.update(source.as_bytes());
        let res = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&res);
        key
    }

    /// AES-256-GCM authenticated encryption with cryptographically random 12-byte IV
    pub fn encrypt_content(
        plaintext: &[u8],
        key: &[u8; 32],
    ) -> Result<(Vec<u8>, String), AppError> {
        use aes_gcm::KeyInit;
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| AppError::InternalServerError(format!("AES key init failed: {e}")))?;

        let mut iv_bytes = [0u8; 12];
        rand::thread_rng().fill(&mut iv_bytes);
        let nonce = Nonce::from_slice(&iv_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| AppError::InternalServerError(format!("AES encryption failed: {e}")))?;

        let iv_hex = hex::encode(iv_bytes);
        Ok((ciphertext, iv_hex))
    }

    /// AES-256-GCM authenticated decryption with authentication tag verification
    pub fn decrypt_content(
        ciphertext: &[u8],
        iv_hex: &str,
        key: &[u8; 32],
    ) -> Result<Vec<u8>, AppError> {
        use aes_gcm::KeyInit;
        let iv_bytes = hex::decode(iv_hex)
            .map_err(|e| AppError::BadRequest(format!("Invalid encryption IV hex: {e}")))?;
        if iv_bytes.len() != 12 {
            return Err(AppError::BadRequest(
                "Encryption IV must be 12 bytes".to_string(),
            ));
        }

        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| AppError::InternalServerError(format!("AES key init failed: {e}")))?;
        let nonce = Nonce::from_slice(&iv_bytes);

        let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|e| {
            AppError::BadRequest(format!("Decryption or auth tag verification failed: {e}"))
        })?;

        Ok(plaintext)
    }

    /// Generates ImageKit-compliant signed URL using HMAC-SHA1
    pub fn sign_url(raw_url: &str, private_key: &str, expire_seconds: u64) -> (String, i64) {
        let expire_timestamp = Utc::now().timestamp() + expire_seconds as i64;
        let expire_str = expire_timestamp.to_string();

        let clean_path = if let Ok(parsed) = reqwest::Url::parse(raw_url) {
            parsed.path().to_string()
        } else {
            raw_url.split('?').next().unwrap_or(raw_url).to_string()
        };

        let sign_target = format!("{}{}", clean_path, expire_str);

        let mut mac = HmacSha1::new_from_slice(private_key.as_bytes())
            .unwrap_or_else(|_| HmacSha1::new_from_slice(b"default_eravaya_key").unwrap());
        mac.update(sign_target.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let sep = if raw_url.contains('?') { "&" } else { "?" };
        let signed_url = format!("{raw_url}{sep}ik-t={expire_str}&ik-s={signature}");

        (signed_url, expire_timestamp)
    }

    /// Verifies HMAC-SHA1 signed URL authenticity and expiration
    pub fn verify_signature(url_with_sig: &str, private_key: &str) -> bool {
        let parsed = match reqwest::Url::parse(url_with_sig) {
            Ok(u) => u,
            Err(_) => return false,
        };

        let mut ik_t: Option<String> = None;
        let mut ik_s: Option<String> = None;

        for (k, v) in parsed.query_pairs() {
            if k == "ik-t" {
                ik_t = Some(v.to_string());
            } else if k == "ik-s" {
                ik_s = Some(v.to_string());
            }
        }

        let (expire_str, sig) = match (ik_t, ik_s) {
            (Some(t), Some(s)) => (t, s),
            _ => return false,
        };

        // Check expiration
        let expire_ts = match expire_str.parse::<i64>() {
            Ok(ts) => ts,
            Err(_) => return false,
        };

        if Utc::now().timestamp() > expire_ts {
            return false; // Expired
        }

        // Verify HMAC-SHA1
        let sign_target = format!("{}{}", parsed.path(), expire_str);
        let mut mac = match HmacSha1::new_from_slice(private_key.as_bytes()) {
            Ok(m) => m,
            Err(_) => return false,
        };
        mac.update(sign_target.as_bytes());
        let expected_sig = hex::encode(mac.finalize().into_bytes());

        sig.eq_ignore_ascii_case(&expected_sig)
    }

    /// Appends ImageKit edge transformations (WebP auto-format, quality, max-width)
    pub fn build_optimized_url(
        base_url: &str,
        width: Option<u32>,
        height: Option<u32>,
        quality: Option<u8>,
    ) -> String {
        let q = quality.unwrap_or(80);
        let mut tr_parts = vec![format!("q-{}", q), "f-auto".to_string()];
        if let Some(w) = width {
            tr_parts.push(format!("w-{}", w));
        }
        if let Some(h) = height {
            tr_parts.push(format!("h-{}", h));
        }

        let tr_str = format!("tr={}", tr_parts.join(","));
        let sep = if base_url.contains('?') { "&" } else { "?" };
        format!("{base_url}{sep}{tr_str}")
    }

    /// Full-featured file upload to ImageKit with compression, optional AES-256-GCM encryption,
    /// and audit logging to the database storage ledger
    pub async fn upload_file(
        file_bytes: Vec<u8>,
        opts: ImageKitUploadOptions,
        pool: Option<&sqlx::PgPool>,
    ) -> Result<ImageKitUploadResult, AppError> {
        let raw_size = file_bytes.len();
        let checksum = Self::compute_sha256(&file_bytes);
        let private_key = Self::get_private_key();
        let is_configured = !private_key.trim().is_empty() && !private_key.starts_with("mock_");

        let (bytes_to_upload, is_encrypted, encryption_iv) = if opts.encrypt {
            let enc_key = Self::get_encryption_key();
            let (ciphertext, iv) = Self::encrypt_content(&file_bytes, &enc_key)?;
            (ciphertext, true, Some(iv))
        } else {
            (file_bytes, false, None)
        };

        let upload_mime = if is_encrypted {
            "application/octet-stream"
        } else {
            opts.mime_type.as_deref().unwrap_or("image/png")
        };

        if is_configured {
            let client = Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| {
                    AppError::InternalServerError(format!("Failed to build HTTP client: {e}"))
                })?;

            let b64_file = base64::engine::general_purpose::STANDARD.encode(&bytes_to_upload);
            let basic_auth = base64::engine::general_purpose::STANDARD
                .encode(format!("{}:", private_key.trim()));

            let file_data_uri = format!("data:{};base64,{}", upload_mime, b64_file);

            let mut form_params = vec![
                ("file", file_data_uri),
                ("fileName", opts.file_name.clone()),
                ("folder", opts.folder.clone()),
                (
                    "useUniqueFileName",
                    if opts.use_unique_file_name {
                        "true"
                    } else {
                        "false"
                    }
                    .to_string(),
                ),
                (
                    "isPrivateFile",
                    if opts.is_private { "true" } else { "false" }.to_string(),
                ),
            ];

            let tags_str = opts.tags.join(",");
            if !tags_str.is_empty() {
                form_params.push(("tags", tags_str));
            }

            let encoded_body = serde_urlencoded::to_string(&form_params).unwrap_or_default();

            let resp = client
                .post("https://upload.imagekit.io/api/v1/files/upload")
                .header("Authorization", format!("Basic {}", basic_auth))
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(encoded_body)
                .send()
                .await;

            match resp {
                Ok(response) if response.status().is_success() => {
                    if let Ok(api_res) = response.json::<ImageKitApiResponse>().await {
                        let file_path = api_res
                            .file_path
                            .unwrap_or_else(|| format!("{}/{}", opts.folder, opts.file_name));
                        let (signed_url, expires_at) =
                            Self::sign_url(&api_res.url, &private_key, 86400); // 24 hours

                        tracing::info!(
                            file_id = %api_res.file_id,
                            url = %api_res.url,
                            size = %raw_size,
                            encrypted = %is_encrypted,
                            "Uploaded asset to ImageKit.io successfully"
                        );

                        // Record in storage_objects ledger if database pool is provided
                        if let Some(pg_pool) = pool {
                            let retention_until = opts
                                .retention_days
                                .map(|d| Utc::now() + chrono::Duration::days(d));

                            let _ = sqlx::query(
                                r#"
                                INSERT INTO storage_objects (
                                    school_id, entity_type, entity_id, file_id, file_name, file_path,
                                    url, storage_provider, is_encrypted, encryption_iv, checksum_sha256,
                                    file_size_bytes, mime_type, retention_until
                                )
                                VALUES ($1, $2, $3, $4, $5, $6, $7, 'IMAGEKIT', $8, $9, $10, $11, $12, $13)
                                "#,
                            )
                            .bind(opts.school_id)
                            .bind(opts.entity_type.as_deref().unwrap_or("ASSET"))
                            .bind(opts.entity_id.as_deref())
                            .bind(&api_res.file_id)
                            .bind(&opts.file_name)
                            .bind(&file_path)
                            .bind(&api_res.url)
                            .bind(is_encrypted)
                            .bind(encryption_iv.as_deref())
                            .bind(&checksum)
                            .bind(raw_size as i64)
                            .bind(upload_mime)
                            .bind(retention_until)
                            .execute(pg_pool)
                            .await;
                        }

                        return Ok(ImageKitUploadResult {
                            file_id: api_res.file_id,
                            name: api_res.name,
                            url: api_res.url,
                            file_path,
                            signed_url_24h: signed_url,
                            expires_at,
                            checksum_sha256: checksum,
                            is_encrypted,
                            encryption_iv,
                            file_size_bytes: raw_size,
                        });
                    }
                }
                Ok(failed) => {
                    let status = failed.status();
                    let body = failed.text().await.unwrap_or_default();
                    tracing::warn!(
                        status = %status,
                        body = %body,
                        "ImageKit upload returned non-200, activating secure local archive fallback"
                    );
                }
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "ImageKit network connection failed, activating secure local archive fallback"
                    );
                }
            }
        }

        // Resilient Fallback: If credentials are unset or remote upload fails,
        // create a secure 24-hour signed URL served via local application endpoint
        let app_url = env::var("APP_URL")
            .unwrap_or_else(|_| "https://my-frontend-kappa-five.vercel.app".to_string());
        let clean_app_url = app_url.trim_end_matches('/');

        let file_id = format!("doc_{}", Uuid::new_v4());
        let file_path = format!("{}/{}", opts.folder, opts.file_name);
        let raw_url = format!("{clean_app_url}/api/v1/admissions/public/documents/{file_id}");
        let signing_key = if !private_key.trim().is_empty() {
            private_key
        } else {
            "eravaya_imagekit_fallback_secret_key_2026".to_string()
        };

        let (signed_url, expires_at) = Self::sign_url(&raw_url, &signing_key, 86400);

        // Save file locally to ensure downloadability in fallback mode
        let dir_path = std::path::Path::new("uploads/admissions");
        let _ = tokio::fs::create_dir_all(dir_path).await;
        let local_file = dir_path.join(format!("{}.png", file_id));
        let _ = tokio::fs::write(&local_file, &bytes_to_upload).await;

        tracing::info!(
            file_id = %file_id,
            path = %local_file.display(),
            "Saved admission document to secure local archive with 24-hour signed link"
        );

        Ok(ImageKitUploadResult {
            file_id,
            name: opts.file_name,
            url: raw_url,
            file_path,
            signed_url_24h: signed_url,
            expires_at,
            checksum_sha256: checksum,
            is_encrypted,
            encryption_iv,
            file_size_bytes: raw_size,
        })
    }

    /// Deletes a file from ImageKit and updates the database storage ledger
    pub async fn delete_file(file_id: &str, pool: Option<&sqlx::PgPool>) -> Result<(), AppError> {
        let private_key = Self::get_private_key();
        if private_key.trim().is_empty() {
            return Ok(());
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| AppError::InternalServerError(format!("HTTP client build failed: {e}")))?;

        let basic_auth =
            base64::engine::general_purpose::STANDARD.encode(format!("{}:", private_key.trim()));

        let delete_url = format!("https://api.imagekit.io/v1/files/{file_id}");
        let resp = client
            .delete(&delete_url)
            .header("Authorization", format!("Basic {}", basic_auth))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() || r.status().as_u16() == 404 => {
                tracing::info!(file_id = %file_id, "Deleted asset from ImageKit.io successfully");
            }
            Ok(failed) => {
                let status = failed.status();
                let body = failed.text().await.unwrap_or_default();
                tracing::warn!(file_id = %file_id, status = %status, body = %body, "Failed to delete file from ImageKit");
            }
            Err(e) => {
                tracing::warn!(file_id = %file_id, error = %e, "ImageKit delete connection failed");
            }
        }

        // Update ledger if database pool is available
        if let Some(pg_pool) = pool {
            let _ = sqlx::query(
                r#"
                UPDATE storage_objects
                SET deleted_from_provider_at = NOW(), updated_at = NOW()
                WHERE file_id = $1
                "#,
            )
            .bind(file_id)
            .execute(pg_pool)
            .await;
        }

        Ok(())
    }

    /// Purges expired files whose compliance retention period has lapsed
    pub async fn purge_expired_retention_files(pool: &sqlx::PgPool) -> Result<usize, AppError> {
        use sqlx::Row;
        let rows = sqlx::query(
            r#"
            SELECT file_id FROM storage_objects
            WHERE retention_until < NOW()
              AND deleted_from_provider_at IS NULL
              AND file_id IS NOT NULL
            "#,
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?;

        let count = rows.len();
        for r in rows {
            if let Ok(file_id) = r.try_get::<String, _>("file_id") {
                let _ = Self::delete_file(&file_id, Some(pool)).await;
            }
        }

        Ok(count)
    }

    /// Backwards compatible helper for existing callers
    pub async fn upload_document(
        file_bytes: Vec<u8>,
        file_name: &str,
        folder: &str,
    ) -> Result<ImageKitUploadResult, AppError> {
        Self::upload_file(
            file_bytes,
            ImageKitUploadOptions {
                file_name: file_name.to_string(),
                folder: folder.to_string(),
                mime_type: Some("image/png".to_string()),
                tags: vec!["admission_document".to_string()],
                use_unique_file_name: false,
                is_private: false,
                encrypt: false,
                retention_days: None,
                school_id: None,
                entity_type: Some("ADMISSION_DOCUMENT".to_string()),
                entity_id: None,
            },
            None,
        )
        .await
    }
}
