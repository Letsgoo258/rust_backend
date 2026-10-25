use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use whatsapp_rust::bot::{Bot, BotHandle, MessageContext};
use whatsapp_rust::client::Client;
use whatsapp_rust::prelude::*;
use whatsapp_rust::types::events::Event;

use super::alert::{AlertReason, WhatsAppAlertManager};
use super::store::PostgresStore;
use wacore::store::DevicePropsOverride;
use whatsapp_rust::waproto::whatsapp::device_props::PlatformType;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Disconnected,
    QrReady,
    Connected,
    LoggedOut,
    TemporarilyBanned { until: DateTime<Utc> },
}

pub struct WhatsAppClient {
    client: Arc<Client>,
    bot_handle: Arc<RwLock<Option<BotHandle>>>,
    connection_state: Arc<RwLock<ConnectionState>>,
    latest_qr: Arc<RwLock<Option<String>>>,
    ban_cooldown_until: Arc<RwLock<Option<DateTime<Utc>>>>,
    pool: PgPool,
    store: Arc<PostgresStore>,
    alert_manager: Arc<WhatsAppAlertManager>,
}

impl WhatsAppClient {
    pub async fn init(
        pool: PgPool,
        store: Arc<PostgresStore>,
        alert_manager: Arc<WhatsAppAlertManager>,
    ) -> Result<(
        Self,
        tokio::sync::mpsc::UnboundedReceiver<crate::modules::whatsapp::InboundMessageEvent>,
    )> {
        let connection_state = Arc::new(RwLock::new(ConnectionState::Disconnected));
        let latest_qr = Arc::new(RwLock::new(None));
        let ban_cooldown_until: Arc<RwLock<Option<DateTime<Utc>>>> = Arc::new(RwLock::new(None));

        let state_qr = Arc::clone(&connection_state);
        let qr_ref = Arc::clone(&latest_qr);
        let on_qr = move |code: String, timeout: std::time::Duration| {
            let state_qr = Arc::clone(&state_qr);
            let qr_ref = Arc::clone(&qr_ref);
            async move {
                *state_qr.write().await = ConnectionState::QrReady;
                *qr_ref.write().await = Some(code.clone());
                warn!("========================================================");
                warn!(
                    "WHATSAPP QR PAIRING REQUIRED (valid for {}s):",
                    timeout.as_secs()
                );
                warn!("\n{}\n", code);
                warn!("========================================================");
            }
        };

        let state_conn = Arc::clone(&connection_state);
        let qr_clear = Arc::clone(&latest_qr);
        let on_conn = move |_client: Arc<Client>| {
            let state_conn = Arc::clone(&state_conn);
            let qr_clear = Arc::clone(&qr_clear);
            async move {
                *state_conn.write().await = ConnectionState::Connected;
                *qr_clear.write().await = None;
                info!("WhatsApp bot connected successfully!");
            }
        };

        let state_logout = Arc::clone(&connection_state);
        let alert_mgr_logout = Arc::clone(&alert_manager);
        let on_logout = move |_info| {
            let state_logout = Arc::clone(&state_logout);
            let alert_mgr = Arc::clone(&alert_mgr_logout);
            async move {
                *state_logout.write().await = ConnectionState::LoggedOut;
                error!("WhatsApp bot was logged out or session expired!");
                let _ = alert_mgr.send_alert(AlertReason::LoggedOut, false).await;
            }
        };

        let (inbound_tx, inbound_rx) =
            tokio::sync::mpsc::unbounded_channel::<crate::modules::whatsapp::InboundMessageEvent>();

        let tx = inbound_tx.clone();
        let on_msg = move |ctx: MessageContext| {
            let tx = tx.clone();
            async move {
                if let Some(text) = ctx.message.text_content() {
                    let sender_alt = ctx.info.source.sender_alt.as_ref().map(|j| j.to_string());
                    let event = crate::modules::whatsapp::InboundMessageEvent {
                        sender_jid: ctx.info.source.sender.to_string(),
                        sender_alt,
                        chat_jid: ctx.info.source.chat.to_string(),
                        raw_id: ctx.info.id.to_string(),
                        text: text.to_string(),
                        timestamp: Utc::now(),
                    };
                    let _ = tx.send(event);
                }
            }
        };

        let state_event = Arc::clone(&connection_state);
        let ban_cooldown_ref = Arc::clone(&ban_cooldown_until);
        let alert_mgr_ban = Arc::clone(&alert_manager);
        let qr_clear_event = Arc::clone(&latest_qr);
        let on_event = move |event: Arc<Event>, _client: Arc<Client>| {
            let state_event = Arc::clone(&state_event);
            let ban_ref = Arc::clone(&ban_cooldown_ref);
            let alert_mgr = Arc::clone(&alert_mgr_ban);
            let qr_clear = Arc::clone(&qr_clear_event);
            async move {
                match &*event {
                    Event::TemporaryBan(ban) => {
                        let duration = chrono::Duration::seconds(ban.expire.num_seconds());
                        let until = Utc::now() + duration;
                        warn!(
                            until = %until,
                            message = ?ban.message,
                            "WhatsApp reported TemporaryBan! Pausing queue."
                        );
                        *state_event.write().await = ConnectionState::TemporarilyBanned { until };
                        *ban_ref.write().await = Some(until);
                        let _ = alert_mgr
                            .send_alert(AlertReason::TemporaryBan { until }, false)
                            .await;
                    }
                    Event::PairSuccess(success) => {
                        info!(
                            id = ?success.id,
                            lid = ?success.lid,
                            platform = ?success.platform,
                            "WhatsApp device paired successfully via QR code!"
                        );
                        *state_event.write().await = ConnectionState::Connected;
                        *qr_clear.write().await = None;
                    }
                    Event::PairError(err) => {
                        error!(
                            error = %err.error,
                            id = ?err.id,
                            "WhatsApp pairing failed on phone!"
                        );
                    }
                    Event::ConnectFailure(fail) => {
                        warn!("WhatsApp ConnectFailure: {:?}", fail);
                    }
                    Event::StreamError(err) => {
                        warn!("WhatsApp StreamError: {:?}", err);
                    }
                    Event::ClientOutdated(_) => {
                        error!("WhatsApp server reports client version is outdated!");
                    }
                    _ => {}
                }
            }
        };

        // Use standard macOS Chrome companion identity with current Web revision so WhatsApp mobile apps accept pairing
        let app_version = whatsapp_rust::waproto::whatsapp::device_props::AppVersion {
            primary: Some(2),
            secondary: Some(3000),
            tertiary: Some(1051602143),
            ..Default::default()
        };

        let device_props = DevicePropsOverride::new()
            .with_os("macOS")
            .with_platform_type(PlatformType::CHROME)
            .with_version(app_version);

        let bot = Bot::builder()
            .with_backend_arc(store.clone())
            .with_device_props(device_props)
            .with_version((2, 3000, 1051602143))
            .on_qr_code(on_qr)
            .on_connected(on_conn)
            .on_logged_out(on_logout)
            .on_message(on_msg)
            .on_event(on_event)
            .build()
            .await
            .map_err(|e| anyhow!("Failed to build WhatsApp bot: {e}"))?;

        let bot_handle = bot.spawn();
        let client = bot_handle.client();

        Ok((
            Self {
                client,
                bot_handle: Arc::new(RwLock::new(Some(bot_handle))),
                connection_state,
                latest_qr,
                ban_cooldown_until,
                pool,
                store,
                alert_manager,
            },
            inbound_rx,
        ))
    }

    pub fn client(&self) -> Arc<Client> {
        Arc::clone(&self.client)
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn alert_manager(&self) -> Arc<WhatsAppAlertManager> {
        Arc::clone(&self.alert_manager)
    }

    pub async fn connection_state(&self) -> ConnectionState {
        self.connection_state.read().await.clone()
    }

    pub async fn latest_qr(&self) -> Option<String> {
        self.latest_qr.read().await.clone()
    }

    pub async fn ban_cooldown(&self) -> Option<DateTime<Utc>> {
        let guard = self.ban_cooldown_until.read().await;
        if let Some(until) = *guard {
            if until > Utc::now() {
                return Some(until);
            }
        }
        None
    }

    pub async fn set_temporary_ban(&self, until: DateTime<Utc>) {
        *self.ban_cooldown_until.write().await = Some(until);
        *self.connection_state.write().await = ConnectionState::TemporarilyBanned { until };
    }

    pub async fn backup_session(&self) -> Result<()> {
        self.store.backup_snapshot().await
    }

    /// Perform key rotation: flushes cryptographic state, saves current timestamp to session store.
    pub async fn rotate_keys(&self) -> Result<()> {
        info!("Executing Signal key rotation and session snapshot backup...");
        let now_str = Utc::now().to_rfc3339();

        let rotate_res = async {
            self.store.backup_snapshot().await?;
            sqlx::query(
                r#"
                INSERT INTO whatsapp_session_store (key, value, updated_at)
                VALUES ('metadata:last_rotated_at', $1, NOW())
                ON CONFLICT (key) DO UPDATE
                SET value = EXCLUDED.value, updated_at = NOW()
                "#,
            )
            .bind(now_str.as_bytes())
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow!("Failed to persist rotation timestamp: {e}"))?;
            Ok::<(), anyhow::Error>(())
        }
        .await;

        match rotate_res {
            Ok(_) => {
                info!("WhatsApp keys and session state rotated successfully.");
                Ok(())
            }
            Err(e) => {
                error!("Key rotation failed: {e:#}");
                let _ = self
                    .alert_manager
                    .send_alert(AlertReason::KeyRotationFailed(e.to_string()), true)
                    .await;
                Err(e)
            }
        }
    }

    /// Return timestamp of the last successful key rotation.
    pub async fn last_rotated_at(&self) -> Option<DateTime<Utc>> {
        let row: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT value FROM whatsapp_session_store WHERE key = 'metadata:last_rotated_at' LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten();

        if let Some(bytes) = row {
            if let Ok(s) = std::str::from_utf8(&bytes) {
                if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
                    return Some(dt.with_timezone(&Utc));
                }
            }
        }
        None
    }

    /// Disconnects the current bot session.
    pub async fn disconnect(&self) -> Result<()> {
        info!("Disconnecting WhatsApp client...");
        *self.connection_state.write().await = ConnectionState::Disconnected;
        *self.latest_qr.write().await = None;
        self.shutdown().await
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down WhatsApp client and disconnecting...");
        if let Some(handle) = self.bot_handle.write().await.take() {
            handle.shutdown().await;
        }

        // Backup final encrypted session to Postgres
        if let Err(e) = self.backup_session().await {
            warn!("Failed to backup session snapshot on shutdown: {e:#}");
        }

        info!("WhatsApp client shutdown complete.");
        Ok(())
    }
}
