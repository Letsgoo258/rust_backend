#![allow(dead_code)]

pub mod alert;
pub mod client;
pub mod queue;
pub mod sender;
pub mod store;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

#[allow(unused_imports)]
pub use alert::{AlertReason, WhatsAppAlertManager};
#[allow(unused_imports)]
pub use client::{ConnectionState, WhatsAppClient};
#[allow(unused_imports)]
pub use queue::{OutboxItem, OutboxStatus, QueueManager};
#[allow(unused_imports)]
pub use sender::{parse_jid, MessageSender, RateLimiter};
#[allow(unused_imports)]
pub use store::PostgresStore;

/// Configuration for the in-process WhatsApp messaging service.
#[derive(Debug, Clone)]
pub struct WhatsAppConfig {
    pub pool: PgPool,
    pub encryption_secret: String,
    pub messages_per_minute: u32,
    pub daily_limit: u32,
}

impl WhatsAppConfig {
    pub fn new(pool: PgPool) -> Self {
        let encryption_secret = std::env::var("WHATSAPP_ENCRYPTION_KEY")
            .unwrap_or_else(|_| "eravaya-default-whatsapp-secure-key-32b".to_string());

        Self {
            pool,
            encryption_secret,
            messages_per_minute: 40,
            daily_limit: 200,
        }
    }

    pub fn with_limits(mut self, per_minute: u32, per_day: u32) -> Self {
        self.messages_per_minute = per_minute;
        self.daily_limit = per_day;
        self
    }
}

/// Ensures all required PostgreSQL tables and indexes for the WhatsApp service exist.
pub async fn ensure_schema(pool: &PgPool) -> Result<()> {
    let statements = [
        r#"
        CREATE TABLE IF NOT EXISTS whatsapp_session_store (
            key TEXT PRIMARY KEY,
            value BYTEA NOT NULL,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS whatsapp_outbox (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            recipient_jid TEXT NOT NULL,
            message_type TEXT NOT NULL DEFAULT 'text',
            payload JSONB NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            priority INT NOT NULL DEFAULT 100,
            idempotency_key TEXT UNIQUE,
            scheduled_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            started_at TIMESTAMPTZ,
            completed_at TIMESTAMPTZ,
            attempt_count INT NOT NULL DEFAULT 0,
            max_attempts INT NOT NULL DEFAULT 3,
            last_error TEXT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_outbox_pending ON whatsapp_outbox (status, priority, scheduled_at)
        WHERE status IN ('pending', 'processing')
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS whatsapp_inbound (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            sender_jid TEXT NOT NULL,
            message_type TEXT NOT NULL,
            payload JSONB NOT NULL,
            whatsapp_timestamp TIMESTAMPTZ NOT NULL,
            received_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_whatsapp_inbound_sender ON whatsapp_inbound (sender_jid, received_at DESC)
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS whatsapp_rate_limit (
            key TEXT PRIMARY KEY,
            window_start TIMESTAMPTZ NOT NULL,
            count INT NOT NULL DEFAULT 0,
            daily_count INT NOT NULL DEFAULT 0,
            daily_reset_at TIMESTAMPTZ NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS whatsapp_user_sessions (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
            sender_jid TEXT NOT NULL UNIQUE,
            phone VARCHAR(50) NOT NULL,
            role_code VARCHAR(100) NOT NULL,
            expires_at TIMESTAMPTZ NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_whatsapp_user_sessions_jid ON whatsapp_user_sessions(sender_jid)
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_whatsapp_user_sessions_phone ON whatsapp_user_sessions(phone)
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS whatsapp_capabilities (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            session_id UUID NOT NULL REFERENCES whatsapp_user_sessions(id) ON DELETE CASCADE,
            user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            role_code VARCHAR(100) NOT NULL,
            capability_key VARCHAR(100) NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE (session_id, capability_key)
        )
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_whatsapp_capabilities_session ON whatsapp_capabilities(session_id)
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS whatsapp_verification_tokens (
            token VARCHAR(64) PRIMARY KEY,
            sender_jid TEXT NOT NULL,
            phone VARCHAR(50) NOT NULL,
            otp_code VARCHAR(10),
            otp_expires_at TIMESTAMPTZ,
            expires_at TIMESTAMPTZ NOT NULL,
            verified BOOLEAN NOT NULL DEFAULT false,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_whatsapp_verification_tokens_phone ON whatsapp_verification_tokens(phone)
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS whatsapp_link_tokens (
            token VARCHAR(32) PRIMARY KEY,
            user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
            role_code VARCHAR(50) NOT NULL,
            ip_address VARCHAR(45) NOT NULL,
            user_agent TEXT,
            device_info JSONB DEFAULT '{}',
            consumed BOOLEAN NOT NULL DEFAULT false,
            consumed_by_jid TEXT,
            consumed_at TIMESTAMPTZ,
            expires_at TIMESTAMPTZ NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_whatsapp_link_tokens_user ON whatsapp_link_tokens(user_id)
        "#,
        r#"
        ALTER TABLE whatsapp_user_sessions
            ADD COLUMN IF NOT EXISTS ip_address VARCHAR(45),
            ADD COLUMN IF NOT EXISTS user_agent TEXT,
            ADD COLUMN IF NOT EXISTS device_fingerprint TEXT,
            ADD COLUMN IF NOT EXISTS last_active_at TIMESTAMPTZ DEFAULT NOW(),
            ADD COLUMN IF NOT EXISTS is_revoked BOOLEAN NOT NULL DEFAULT false,
            ADD COLUMN IF NOT EXISTS suspicious_reason TEXT,
            ADD COLUMN IF NOT EXISTS login_token VARCHAR(32)
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS school_settings (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
            key VARCHAR(100) NOT NULL,
            value JSONB NOT NULL DEFAULT '{}'::jsonb,
            created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
            UNIQUE(school_id, key)
        )
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_school_settings_school_key ON school_settings(school_id, key)
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS admission_otps (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            target VARCHAR(255) NOT NULL,
            target_type VARCHAR(20) NOT NULL,
            otp_code VARCHAR(10) NOT NULL,
            verified BOOLEAN NOT NULL DEFAULT false,
            verification_token VARCHAR(64),
            expires_at TIMESTAMPTZ NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_admission_otps_target ON admission_otps(target, target_type)
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_admission_otps_token ON admission_otps(verification_token)
        "#,
    ];

    for stmt in statements {
        sqlx::query(stmt)
            .execute(pool)
            .await
            .map_err(|e| anyhow!("Ensuring WhatsApp database schema: {e}"))?;
    }

    Ok(())
}

/// The main public handle for the in-process WhatsApp service.
#[derive(Clone)]
pub struct WhatsAppService {
    client: Arc<WhatsAppClient>,
    queue: QueueManager,
    alert_manager: Arc<WhatsAppAlertManager>,
    shutdown_tx: broadcast::Sender<()>,
}

impl WhatsAppService {
    /// Initialize the bot, restore encrypted session from PostgreSQL, start the queue worker, and return a service handle.
    pub async fn start(config: WhatsAppConfig) -> Result<Self> {
        ensure_schema(&config.pool).await?;

        let store = PostgresStore::init(config.pool.clone(), &config.encryption_secret)
            .await
            .map_err(|e| anyhow!("Initializing PostgreSQL WhatsApp store: {e}"))?;

        let alert_manager = Arc::new(WhatsAppAlertManager::new(config.pool.clone()));

        let (client_raw, inbound_rx) =
            WhatsAppClient::init(config.pool.clone(), store, Arc::clone(&alert_manager))
                .await
                .map_err(|e| anyhow!("Initializing WhatsAppClient: {e}"))?;
        let client = Arc::new(client_raw);

        let queue = QueueManager::new(config.pool.clone());
        let rate_limiter = RateLimiter::new(
            config.pool.clone(),
            config.messages_per_minute,
            config.daily_limit,
        );

        let sender = Arc::new(MessageSender::new(
            queue.clone(),
            Arc::clone(&client),
            rate_limiter,
        ));

        let (shutdown_tx, _) = broadcast::channel(16);
        let shutdown_rx_sender = shutdown_tx.subscribe();
        let shutdown_rx_replies = shutdown_tx.subscribe();

        // 1. Spawn background queue consumer worker (for manual / blast outbox)
        tokio::spawn(async move {
            sender.run(shutdown_rx_sender).await;
        });

        // 2. Spawn full-time dedicated instant reply worker (for sub-100ms replies)
        let reply_worker = crate::modules::whatsapp::WhatsAppReplyWorker::new(
            inbound_rx,
            Arc::clone(&client),
            config.pool.clone(),
        );
        tokio::spawn(async move {
            reply_worker.run(shutdown_rx_replies).await;
        });

        Ok(Self {
            client,
            queue,
            alert_manager,
            shutdown_tx,
        })
    }

    /// Enqueue a text message. Returns the outbox row UUID.
    pub async fn enqueue_text(
        &self,
        recipient: &str,
        text: &str,
        priority: i32,
        idempotency_key: Option<&str>,
        skip_typing: bool,
    ) -> Result<Uuid> {
        let payload = serde_json::json!({
            "text": text,
            "skip_typing": skip_typing,
        });

        self.queue
            .enqueue(recipient, "text", payload, priority, idempotency_key)
            .await
    }

    /// Enqueue a media message (payload contains media reference/caption).
    pub async fn enqueue_media(
        &self,
        recipient: &str,
        media_type: &str,
        payload: serde_json::Value,
        priority: i32,
    ) -> Result<Uuid> {
        self.queue
            .enqueue(recipient, media_type, payload, priority, None)
            .await
    }

    /// Get current status of an outbox message.
    pub async fn get_status(&self, id: Uuid) -> Result<OutboxStatus> {
        self.queue.get_status(id).await
    }

    /// Retrieve the latest pairing QR code if pairing is active.
    pub async fn get_qr_code(&self) -> Option<String> {
        self.client.latest_qr().await
    }

    /// Retrieve the current connection state.
    pub async fn get_connection_state(&self) -> ConnectionState {
        self.client.connection_state().await
    }

    /// Retrieve temporary ban expiration if any.
    pub async fn get_ban_cooldown(&self) -> Option<DateTime<Utc>> {
        self.client.ban_cooldown().await
    }

    /// Rotate cryptographic pre-keys and update PostgreSQL session snapshot.
    pub async fn rotate_keys(&self) -> Result<()> {
        self.client.rotate_keys().await
    }

    /// Retrieve the timestamp of the last successful key rotation.
    pub async fn last_rotated_at(&self) -> Option<DateTime<Utc>> {
        self.client.last_rotated_at().await
    }

    /// Access the alert manager.
    pub fn alert_manager(&self) -> Arc<WhatsAppAlertManager> {
        Arc::clone(&self.alert_manager)
    }

    /// Disconnects the WhatsApp client.
    pub async fn disconnect(&self) -> Result<()> {
        self.client.disconnect().await
    }

    /// Manually trigger an encrypted session backup to PostgreSQL.
    pub async fn backup_session(&self) -> Result<()> {
        self.client.backup_session().await
    }

    /// Graceful shutdown: notifies worker to exit, flushes device snapshot, and closes connection.
    pub async fn shutdown(self) -> Result<()> {
        let _ = self.shutdown_tx.send(());
        self.client.shutdown().await
    }
}

// ---------------------------------------------------------------------------
// High-Level Public Utility API (Callable from any controller, module, or job)
// ---------------------------------------------------------------------------

/// Enqueue a standard text WhatsApp message using just a PostgreSQL pool reference.
///
/// Automatically formats the JID, handles idempotency, rate limiting, and typing simulation.
pub async fn send_whatsapp_message(
    pool: &PgPool,
    recipient: &str,
    text: &str,
    idempotency_key: Option<&str>,
) -> Result<Uuid> {
    let queue = QueueManager::new(pool.clone());
    let payload = serde_json::json!({
        "text": text,
        "skip_typing": false,
    });
    queue
        .enqueue(recipient, "text", payload, 0, idempotency_key)
        .await
}

/// Convenience helper to enqueue a WhatsApp text message without specifying an idempotency key.
pub async fn send_whatsapp_text(pool: &PgPool, recipient: &str, text: &str) -> Result<Uuid> {
    send_whatsapp_message(pool, recipient, text, None).await
}

/// Enqueue an urgent high-priority WhatsApp text message (priority 10, skips typing simulation).
///
/// Ideal for OTPs, emergency student alerts, or critical security verification codes.
pub async fn send_urgent_whatsapp_message(
    pool: &PgPool,
    recipient: &str,
    text: &str,
    idempotency_key: Option<&str>,
) -> Result<Uuid> {
    let queue = QueueManager::new(pool.clone());
    let payload = serde_json::json!({
        "text": text,
        "skip_typing": true,
    });
    queue
        .enqueue(recipient, "text", payload, 10, idempotency_key)
        .await
}

/// Enqueue a media WhatsApp message (e.g., fee receipt PDF, student report card).
pub async fn send_whatsapp_media(
    pool: &PgPool,
    recipient: &str,
    media_type: &str,
    payload: serde_json::Value,
    priority: i32,
    idempotency_key: Option<&str>,
) -> Result<Uuid> {
    let queue = QueueManager::new(pool.clone());
    queue
        .enqueue(recipient, media_type, payload, priority, idempotency_key)
        .await
}

/// Query the delivery status of an enqueued WhatsApp outbox message.
pub async fn get_whatsapp_message_status(pool: &PgPool, message_id: Uuid) -> Result<OutboxStatus> {
    let queue = QueueManager::new(pool.clone());
    queue.get_status(message_id).await
}

/// Generates a standalone vector SVG string from a raw pairing QR code string.
pub fn generate_qr_svg(raw_qr: &str) -> Result<String> {
    let code = qrcode::QrCode::new(raw_qr.as_bytes())
        .map_err(|e| anyhow!("Failed to generate QR code: {e}"))?;
    let svg = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(280, 280)
        .build();
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_qr_svg_produces_valid_svg() {
        let sample_qr = "2@1bCs890abcdef1234567890abcdef1234567890=";
        let svg = generate_qr_svg(sample_qr).expect("Failed to render QR to SVG");
        assert!(svg.contains("<svg"), "Expected SVG opening tag");
        assert!(svg.contains("</svg>"), "Expected SVG closing tag");
        assert!(svg.contains("viewBox"), "Expected viewBox attribute");
    }

    #[test]
    fn test_alert_reason_metadata() {
        let logged_out = AlertReason::LoggedOut;
        assert_eq!(
            logged_out.title(),
            "WhatsApp Session Expired / Device Unlinked"
        );
        assert!(logged_out.description().contains("unlinked"));

        let manual_test = AlertReason::ManualTest;
        assert_eq!(manual_test.title(), "WhatsApp Alert System Diagnostic Test");
    }
}
