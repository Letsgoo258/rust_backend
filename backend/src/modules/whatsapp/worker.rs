use crate::modules::whatsapp::repository::WhatsAppRepository;
use crate::modules::whatsapp::service::WhatsAppMessageService;
use crate::utils::whatsapp::{parse_jid, WhatsAppClient};
use chrono::{DateTime, Utc};
use rand::Rng;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct InboundMessageEvent {
    pub sender_jid: String,
    pub sender_alt: Option<String>,
    pub chat_jid: String,
    pub raw_id: String,
    pub text: String,
    pub timestamp: DateTime<Utc>,
}

/// A dedicated, full-time background worker for instant WhatsApp replies.
///
/// Bypasses database outbox queue polling and artificial typing sleeps:
/// 1. Receives in-memory inbound events instantly via unbounded mpsc channel.
/// 2. Evaluates intent (greeting, help menu, AI/fallback) in memory.
/// 3. Sends the reply directly through the connected WhatsApp WebSocket socket.
/// 4. Asynchronously persists inbound and outbox audit rows to PostgreSQL
///    in a detached task so DB I/O never delays message delivery.
pub struct WhatsAppReplyWorker {
    inbound_rx: mpsc::UnboundedReceiver<InboundMessageEvent>,
    client: Arc<WhatsAppClient>,
    pool: PgPool,
}

impl WhatsAppReplyWorker {
    pub fn new(
        inbound_rx: mpsc::UnboundedReceiver<InboundMessageEvent>,
        client: Arc<WhatsAppClient>,
        pool: PgPool,
    ) -> Self {
        Self {
            inbound_rx,
            client,
            pool,
        }
    }

    /// Full-time event loop that lives for the lifetime of the WhatsApp service.
    pub async fn run(mut self, mut shutdown_rx: broadcast::Receiver<()>) {
        info!("WhatsApp full-time instant reply worker started");

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("WhatsApp reply worker received shutdown signal");
                    break;
                }
                event_opt = self.inbound_rx.recv() => {
                    match event_opt {
                        Some(event) => {
                            self.process_instant_reply(event).await;
                        }
                        None => {
                            info!("WhatsApp inbound message channel closed, reply worker exiting");
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn process_instant_reply(&self, event: InboundMessageEvent) {
        let sender_jid = event.sender_jid.clone();
        let text = event.text.trim();
        if text.is_empty() {
            return;
        }

        let start_time = std::time::Instant::now();

        // 1. Generate reply messages (returns 2 messages for registration/login so link is easy to copy/paste)
        let reply_messages = WhatsAppMessageService::generate_reply_messages(
            &self.pool,
            &sender_jid,
            event.sender_alt.as_deref(),
            text,
        )
        .await;

        if reply_messages.is_empty() {
            return;
        }

        // 2. Deliver through WhatsApp socket with natural human reading delay and typing presence
        let wa_client = self.client.client();
        match parse_jid(&sender_jid) {
            Ok(jid) => {
                for (idx, msg) in reply_messages.iter().enumerate() {
                    if idx > 0 {
                        let between_pause = {
                            let mut rng = rand::thread_rng();
                            rng.gen_range(250..500)
                        };
                        tokio::time::sleep(Duration::from_millis(between_pause)).await;
                    }

                    // Display realistic "typing..." presence with random reading/typing jitter
                    simulate_human_typing(&wa_client, &jid, msg.len()).await;

                    match wa_client.send_text(jid.clone(), msg).await {
                        Ok(_) => {
                            let elapsed = start_time.elapsed().as_millis();
                            info!(
                                sender = %sender_jid,
                                msg_idx = idx + 1,
                                total_msgs = reply_messages.len(),
                                latency_ms = elapsed,
                                "WhatsApp reply delivered with typing presence directly via WebSocket"
                            );
                        }
                        Err(e) => {
                            error!(
                                sender = %sender_jid,
                                error = %e,
                                "Failed to deliver direct WhatsApp reply message"
                            );
                        }
                    }
                }
            }
            Err(e) => {
                error!(sender = %sender_jid, error = %e, "Invalid sender JID for direct reply");
            }
        }

        // 3. Asynchronously record to PostgreSQL without blocking message delivery
        let pool = self.pool.clone();
        let sender = sender_jid;
        let incoming_payload = serde_json::json!({
            "text": text,
            "raw_id": event.raw_id,
            "chat": event.chat_jid,
        });
        let reply_msg = reply_messages.join("\n\n");
        tokio::spawn(async move {
            // Record inbound message
            let _ = WhatsAppRepository::record_inbound(
                &pool,
                &sender,
                "text",
                incoming_payload,
                event.timestamp,
            )
            .await;

            // Record outbox as completed / sent
            let _ = sqlx::query(
                r#"
                INSERT INTO whatsapp_outbox (recipient_jid, message_type, payload, priority, status, created_at, completed_at, sent_at)
                VALUES ($1, 'text', $2, 10, 'sent', NOW(), NOW(), NOW())
                "#,
            )
            .bind(&sender)
            .bind(serde_json::json!({ "text": reply_msg, "skip_typing": true }))
            .execute(&pool)
            .await;

            // Increment rate limit metric
            let _ = sqlx::query(
                r#"
                UPDATE whatsapp_rate_limit
                SET count = count + 1, daily_count = daily_count + 1
                WHERE key = 'global'
                "#,
            )
            .execute(&pool)
            .await;
        });
    }
}

/// Simulates human-like behavior:
/// 1. Short initial pause (as if reading incoming message: 250..600ms)
/// 2. Broadcasts "composing" (typing...) presence to user chat
/// 3. Typing duration scaled to text length with random jitter (700ms base + len/25*60ms, capped at 1600ms, + 100..400ms jitter)
/// 4. Sends "paused" presence
/// 5. Brief send wait before dispatching text packet (150..300ms)
async fn simulate_human_typing(
    client: &whatsapp_rust::Client,
    jid: &whatsapp_rust::Jid,
    text_len: usize,
) {
    // 1. Initial reading delay (drop ThreadRng before await)
    let read_delay = {
        let mut rng = rand::thread_rng();
        rng.gen_range(250..600)
    };
    tokio::time::sleep(Duration::from_millis(read_delay)).await;

    // 2. Composing presence ("typing...")
    let _ = client.chatstate().send_composing(jid).await;

    // 3. Typing duration (drop ThreadRng before await)
    let typing_duration = {
        let mut rng = rand::thread_rng();
        let base_typing = 700u64;
        let extra_typing = ((text_len / 25) as u64 * 60).min(1600);
        let jitter = rng.gen_range(100..400);
        base_typing + extra_typing + jitter
    };
    tokio::time::sleep(Duration::from_millis(typing_duration)).await;

    // 4. Send paused presence
    let _ = client.chatstate().send_paused(jid).await;

    // 5. Brief send wait before dispatching text packet (drop ThreadRng before await)
    let send_delay = {
        let mut rng = rand::thread_rng();
        rng.gen_range(150..300)
    };
    tokio::time::sleep(Duration::from_millis(send_delay)).await;
}
