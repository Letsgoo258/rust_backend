use crate::error::AppError;
use crate::modules::auth::backend::AuthSession;
use crate::state::AppState;
use crate::utils::whatsapp::{
    generate_qr_svg, AlertReason, ConnectionState, WhatsAppConfig, WhatsAppService,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Serialize)]
pub struct WhatsAppStats {
    pub pending: i64,
    pub sent_today: i64,
    pub failed: i64,
    pub daily_limit: u32,
    pub messages_per_minute: u32,
}

#[derive(Serialize)]
pub struct WhatsAppStatusResponse {
    pub connection_state: ConnectionState,
    pub qr_raw: Option<String>,
    pub qr_svg: Option<String>,
    pub ban_cooldown_until: Option<DateTime<Utc>>,
    pub stats: WhatsAppStats,
    pub last_rotated_at: Option<DateTime<Utc>>,
    pub alert_recipients: Vec<String>,
}

#[derive(Deserialize)]
pub struct SendTestRequest {
    pub recipient: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct LogsQuery {
    pub limit: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct OutboxLogRow {
    pub id: Uuid,
    pub recipient_jid: String,
    pub message_type: String,
    pub status: String,
    pub priority: i32,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
}

/// Helper to ensure the current authenticated user has administrative privileges.
fn ensure_admin(auth_session: &AuthSession) -> Result<(), AppError> {
    let user = auth_session
        .user
        .as_ref()
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SUPER_ADMIN".to_string()) && !user.is_school_admin() {
        return Err(AppError::Forbidden(
            "Access denied: School Admin or Super Admin role required".into(),
        ));
    }
    Ok(())
}

/// Helper to get or lazily initialize the WhatsApp service instance.
async fn get_or_init_service(state: &AppState) -> Result<Arc<WhatsAppService>, AppError> {
    let guard = state.whatsapp.read().await;
    if let Some(service) = guard.as_ref() {
        return Ok(Arc::clone(service));
    }
    drop(guard);

    let mut write_guard = state.whatsapp.write().await;
    if let Some(service) = write_guard.as_ref() {
        return Ok(Arc::clone(service));
    }

    let config = WhatsAppConfig::new(state.db.clone());
    let service = WhatsAppService::start(config)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Starting WhatsApp service: {e}")))?;

    let arc = Arc::new(service);
    *write_guard = Some(Arc::clone(&arc));
    Ok(arc)
}

/// GET /api/v1/settings/whatsapp/status
pub async fn get_status(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    ensure_admin(&auth_session)?;

    let service_opt = state.whatsapp.read().await.clone();

    let (conn_state, qr_raw, qr_svg, ban_until, last_rotated) = match service_opt.as_ref() {
        Some(svc) => {
            let state = svc.get_connection_state().await;
            let qr = svc.get_qr_code().await;
            let svg = qr.as_deref().and_then(|raw| generate_qr_svg(raw).ok());
            let ban = svc.get_ban_cooldown().await;
            let rotated = svc.last_rotated_at().await;
            (state, qr, svg, ban, rotated)
        }
        None => (ConnectionState::Disconnected, None, None, None, None),
    };

    // Aggregate statistics from PostgreSQL
    let pending_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM whatsapp_outbox WHERE status IN ('pending', 'typing')",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or((0,));

    let failed_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM whatsapp_outbox WHERE status = 'failed'")
            .fetch_one(&state.db)
            .await
            .unwrap_or((0,));

    let sent_today: (i32,) = sqlx::query_as(
        r#"
        SELECT COALESCE(daily_count, 0) 
        FROM whatsapp_rate_limit 
        WHERE key = 'global_outbound'
        "#,
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None)
    .unwrap_or((0,));

    // Resolve alert recipients
    let alert_recipients = if let Some(svc) = service_opt {
        svc.alert_manager().resolve_admin_emails().await
    } else {
        let temp_mgr = crate::utils::whatsapp::alert::WhatsAppAlertManager::new(state.db.clone());
        temp_mgr.resolve_admin_emails().await
    };

    let resp = WhatsAppStatusResponse {
        connection_state: conn_state,
        qr_raw,
        qr_svg,
        ban_cooldown_until: ban_until,
        stats: WhatsAppStats {
            pending: pending_count.0,
            sent_today: sent_today.0 as i64,
            failed: failed_count.0,
            daily_limit: 200,
            messages_per_minute: 40,
        },
        last_rotated_at: last_rotated,
        alert_recipients,
    };

    Ok(Json(resp))
}

/// POST /api/v1/settings/whatsapp/initialize
pub async fn initialize(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    ensure_admin(&auth_session)?;

    let mut write_guard = state.whatsapp.write().await;
    let should_restart = match write_guard.as_ref() {
        Some(svc) => {
            let conn = svc.get_connection_state().await;
            conn != ConnectionState::Connected
        }
        None => true,
    };

    if should_restart {
        if let Some(old_svc) = write_guard.take() {
            let _ = old_svc.disconnect().await;
        }
        let config = WhatsAppConfig::new(state.db.clone());
        let service = WhatsAppService::start(config).await.map_err(|e| {
            AppError::InternalServerError(format!("Starting WhatsApp service: {e}"))
        })?;
        *write_guard = Some(Arc::new(service));
    }

    let service = Arc::clone(write_guard.as_ref().unwrap());
    drop(write_guard);

    // Poll briefly for the newly generated pairing QR code
    let mut qr = service.get_qr_code().await;
    if qr.is_none() {
        for _ in 0..15 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            qr = service.get_qr_code().await;
            if qr.is_some() {
                break;
            }
        }
    }

    let qr_svg = qr.as_deref().and_then(|raw| generate_qr_svg(raw).ok());

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "WhatsApp service initialized successfully. Scan the QR code to connect.",
        "qr_raw": qr,
        "qr_svg": qr_svg,
    })))
}

/// POST /api/v1/settings/whatsapp/rotate-keys
pub async fn rotate_keys(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    ensure_admin(&auth_session)?;

    let service = get_or_init_service(&state).await?;
    service
        .rotate_keys()
        .await
        .map_err(|e| AppError::InternalServerError(format!("Key rotation failed: {e}")))?;

    let last_rotated = service.last_rotated_at().await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Signal cryptographic pre-keys rotated and session backed up successfully.",
        "last_rotated_at": last_rotated,
    })))
}

/// POST /api/v1/settings/whatsapp/disconnect
pub async fn disconnect(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    ensure_admin(&auth_session)?;

    let mut guard = state.whatsapp.write().await;
    if let Some(service) = guard.take() {
        let _ = service.disconnect().await;
    }

    // Clean up primary device from database to require fresh QR pairing
    let _ = sqlx::query("DELETE FROM whatsapp_session_store WHERE key = 'primary_device'")
        .execute(&state.db)
        .await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "WhatsApp bot disconnected and session unlinked.",
    })))
}

/// POST /api/v1/settings/whatsapp/send-test
pub async fn send_test(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<SendTestRequest>,
) -> Result<impl IntoResponse, AppError> {
    ensure_admin(&auth_session)?;

    if payload.recipient.trim().is_empty() || payload.message.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Recipient phone number and message text are required".into(),
        ));
    }

    let service = get_or_init_service(&state).await?;
    let idempotency_key = format!("test-msg-{}", Uuid::new_v4());

    let msg_id = service
        .enqueue_text(
            &payload.recipient,
            &payload.message,
            5, // Medium-high priority for test messages
            Some(&idempotency_key),
            false,
        )
        .await
        .map_err(|e| AppError::InternalServerError(format!("Enqueuing test message: {e}")))?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "message_id": msg_id,
            "message": "Test WhatsApp message enqueued successfully into the outbound worker queue.",
        })),
    ))
}

/// POST /api/v1/settings/whatsapp/test-alert
pub async fn test_alert(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    ensure_admin(&auth_session)?;

    let service = get_or_init_service(&state).await?;
    let alert_manager = service.alert_manager();

    let count = alert_manager
        .send_alert(AlertReason::ManualTest, true)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Dispatching test alert: {e}")))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "emails_sent": count,
        "message": format!("Diagnostic critical alert email dispatched to {count} administrator(s)."),
    })))
}

/// GET /api/v1/settings/whatsapp/logs
pub async fn get_logs(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Query(query): Query<LogsQuery>,
) -> Result<impl IntoResponse, AppError> {
    ensure_admin(&auth_session)?;

    let limit = query.limit.unwrap_or(50).clamp(1, 100);

    let rows: Vec<OutboxLogRow> = sqlx::query_as(
        r#"
        SELECT id, recipient_jid, message_type, status, priority, 
               attempt_count, max_attempts, last_error, payload, created_at,
               completed_at as sent_at
        FROM whatsapp_outbox
        ORDER BY created_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Querying outbox logs: {e}")))?;

    Ok(Json(serde_json::json!({
        "logs": rows,
    })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", get(get_status))
        .route("/initialize", post(initialize))
        .route("/rotate-keys", post(rotate_keys))
        .route("/disconnect", post(disconnect))
        .route("/send-test", post(send_test))
        .route("/test-alert", post(test_alert))
        .route("/logs", get(get_logs))
}
