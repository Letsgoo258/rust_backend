use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use rand::Rng;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use whatsapp_rust::client::Client;
use whatsapp_rust::prelude::*;
use whatsapp_rust::Jid;

use super::client::WhatsAppClient;
use super::queue::{OutboxItem, QueueManager};

/// Helper to parse recipient phone number or full JID into a typed Jid.
pub fn parse_jid(recipient: &str) -> Result<Jid> {
    let trimmed = recipient.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Recipient JID/phone cannot be empty"));
    }

    if trimmed.contains('@') {
        trimmed
            .parse::<Jid>()
            .map_err(|e| anyhow!("Invalid JID format '{trimmed}': {e}"))
    } else {
        // Strip non-digits (e.g. +91 98765 43210 -> 919876543210)
        let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return Err(anyhow!("Recipient '{trimmed}' contains no digits"));
        }
        Ok(Jid::pn(&digits))
    }
}

/// Calculate human-like typing duration in milliseconds based on text length:
/// Base: 1500ms minimum
/// Additional: (message_length / 10) * 150ms, capped at 5000ms
/// Random jitter: + rand(0..800)ms
pub fn calculate_typing_duration(text_len: usize, jitter: u64) -> u64 {
    let base = 1500u64;
    let per_char = ((text_len / 10) as u64) * 150;
    (base + per_char + jitter).min(5000)
}

/// Simulate human-like typing:
/// 1. Send "composing" presence
/// 2. Sleep for computed typing duration
/// 3. Send "paused" presence
/// 4. Wait 200..600ms before hitting send
pub async fn simulate_typing(client: &Client, jid: &Jid, text_len: usize) -> Result<()> {
    if let Err(e) = client.chatstate().send_composing(jid).await {
        debug!(target: "whatsapp::sender", "Could not send composing presence: {e}");
    }

    let jitter = rand::thread_rng().gen_range(0..800);
    let duration = calculate_typing_duration(text_len, jitter);
    tokio::time::sleep(Duration::from_millis(duration)).await;

    if let Err(e) = client.chatstate().send_paused(jid).await {
        debug!(target: "whatsapp::sender", "Could not send paused presence: {e}");
    }

    let send_wait = rand::thread_rng().gen_range(200..600);
    tokio::time::sleep(Duration::from_millis(send_wait)).await;
    Ok(())
}

/// Token bucket and calendar day rate limiter with PostgreSQL persistence.
#[derive(Clone)]
pub struct RateLimiter {
    pool: PgPool,
    messages_per_minute: u32,
    daily_limit: u32,
}

impl RateLimiter {
    pub fn new(pool: PgPool, messages_per_minute: u32, daily_limit: u32) -> Self {
        Self {
            pool,
            messages_per_minute,
            daily_limit,
        }
    }

    /// Check rate limits against database state and wait if limits are reached.
    pub async fn check_and_wait(&self, key: &str) -> Result<()> {
        let now = Utc::now();

        // 1. Fetch or initialize rate limit record
        let row_opt = sqlx::query(
            "SELECT window_start, count, daily_count, daily_reset_at FROM whatsapp_rate_limit WHERE key = $1"
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!("Querying whatsapp_rate_limit: {e}"))?;

        let (window_start, count, daily_count, daily_reset_at): (
            DateTime<Utc>,
            i32,
            i32,
            DateTime<Utc>,
        ) = match row_opt {
            Some(row) => (
                row.get("window_start"),
                row.get("count"),
                row.get("daily_count"),
                row.get("daily_reset_at"),
            ),
            None => {
                let initial_reset = now + ChronoDuration::days(1);
                sqlx::query(
                    r#"
                    INSERT INTO whatsapp_rate_limit (key, window_start, count, daily_count, daily_reset_at)
                    VALUES ($1, $2, 0, 0, $3)
                    ON CONFLICT (key) DO NOTHING
                    "#
                )
                .bind(key)
                .bind(now)
                .bind(initial_reset)
                .execute(&self.pool)
                .await
                .map_err(|e| anyhow!("Initializing whatsapp_rate_limit row: {e}"))?;

                (now, 0, 0, initial_reset)
            }
        };

        // 2. Check Daily Limit
        if now >= daily_reset_at {
            // Reset daily bucket
            let next_reset = now + ChronoDuration::days(1);
            sqlx::query(
                "UPDATE whatsapp_rate_limit SET daily_count = 0, daily_reset_at = $1 WHERE key = $2"
            )
            .bind(next_reset)
            .bind(key)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow!("Resetting daily rate limit bucket: {e}"))?;
        } else if daily_count >= self.daily_limit as i32 {
            let wait_secs = (daily_reset_at - now).num_seconds().max(1);
            warn!(
                key,
                wait_secs, "Daily WhatsApp limit reached. Sleeping until daily reset."
            );
            tokio::time::sleep(Duration::from_secs(wait_secs as u64)).await;
            return Box::pin(self.check_and_wait(key)).await;
        }

        // 3. Check Rolling 60s Window (Per-Minute Limit)
        let elapsed_secs = (now - window_start).num_seconds();
        if elapsed_secs >= 60 {
            // Reset minute window
            sqlx::query(
                "UPDATE whatsapp_rate_limit SET window_start = $1, count = 0 WHERE key = $2",
            )
            .bind(now)
            .bind(key)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow!("Resetting minute rate limit window: {e}"))?;
        } else if count >= self.messages_per_minute as i32 {
            let wait_secs = (60 - elapsed_secs).max(1);
            debug!(
                key,
                wait_secs, "Per-minute WhatsApp limit reached. Waiting for rolling window reset."
            );
            tokio::time::sleep(Duration::from_secs(wait_secs as u64)).await;
            return Box::pin(self.check_and_wait(key)).await;
        }

        Ok(())
    }

    /// Record a sent message in the rate limiter state.
    pub async fn record_sent(&self, key: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE whatsapp_rate_limit
            SET count = count + 1,
                daily_count = daily_count + 1
            WHERE key = $1
            "#,
        )
        .bind(key)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!("Incrementing rate limit counters: {e}"))?;

        Ok(())
    }
}

/// Outbox consumer and dispatcher.
pub struct MessageSender {
    queue: QueueManager,
    client: Arc<WhatsAppClient>,
    rate_limiter: RateLimiter,
}

impl MessageSender {
    pub fn new(
        queue: QueueManager,
        client: Arc<WhatsAppClient>,
        rate_limiter: RateLimiter,
    ) -> Self {
        Self {
            queue,
            client,
            rate_limiter,
        }
    }

    /// Start the background queue consumer loop.
    pub async fn run(self: Arc<Self>, mut shutdown_rx: tokio::sync::broadcast::Receiver<()>) {
        info!("WhatsApp queue sender worker started");

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("WhatsApp queue sender worker received shutdown signal");
                    break;
                }
                _ = tokio::time::sleep(Duration::from_millis(500)) => {
                    // Check temporary ban cooldown
                    if let Some(until) = self.client.ban_cooldown().await {
                        let now = Utc::now();
                        if until > now {
                            let wait_duration = (until - now)
                                .to_std()
                                .unwrap_or(Duration::from_secs(60));
                            warn!(
                                wait_secs = wait_duration.as_secs(),
                                "Queue paused due to temporary ban cooldown"
                            );
                            tokio::time::sleep(wait_duration).await;
                            continue;
                        }
                    }

                    // Dequeue next eligible pending message
                    match self.queue.dequeue_next().await {
                        Ok(Some(item)) => {
                            // Enforce multi-tier rate limiting before sending
                            if let Err(e) = self.rate_limiter.check_and_wait("global").await {
                                warn!("Rate limiter check error: {e:#}");
                            }

                            // Process send
                            self.process_item(item).await;

                            // Jitter between dequeues: 1.0s - 2.0s
                            let jitter_ms = rand::thread_rng().gen_range(1000..2000);
                            tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
                        }
                        Ok(None) => {
                            // Queue is empty; idle for 2s to minimize DB query overhead
                            tokio::time::sleep(Duration::from_millis(2000)).await;
                        }
                        Err(e) => {
                            error!("Error dequeuing from outbox: {e:#}");
                            tokio::time::sleep(Duration::from_millis(3000)).await;
                        }
                    }
                }
            }
        }

        info!("WhatsApp queue sender worker stopped");
    }

    async fn process_item(&self, item: OutboxItem) {
        let res = self.send_item(&item).await;
        match res {
            Ok(_) => {
                info!(id = %item.id, recipient = %item.recipient_jid, "WhatsApp message sent successfully");
                if let Err(e) = self.queue.mark_sent(item.id).await {
                    error!(id = %item.id, "Failed to mark outbox item as sent: {e:#}");
                }
                if let Err(e) = self.rate_limiter.record_sent("global").await {
                    warn!("Failed to record rate limit sent metric: {e:#}");
                }
            }
            Err(e) => {
                let error_msg = format!("{e:#}");
                error!(
                    id = %item.id,
                    recipient = %item.recipient_jid,
                    error = %error_msg,
                    attempt = item.attempt_count,
                    max = item.max_attempts,
                    "WhatsApp message send failed"
                );

                if let Err(err) = self
                    .queue
                    .mark_failed(item.id, &error_msg, item.attempt_count, item.max_attempts)
                    .await
                {
                    error!(id = %item.id, "Failed to record failure in outbox: {err:#}");
                }
            }
        }
    }

    async fn send_item(&self, item: &OutboxItem) -> Result<()> {
        let jid = parse_jid(&item.recipient_jid)?;
        let skip_typing = item
            .payload
            .get("skip_typing")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let text = item
            .payload
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Human-like typing simulation for conversational messages
        if !skip_typing && !text.is_empty() {
            simulate_typing(&self.client.client(), &jid, text.len())
                .await
                .ok();
        }

        let wa_client = self.client.client();

        if item.message_type == "text" || !text.is_empty() {
            wa_client.send_text(jid, text).await.map_err(|e| {
                anyhow!("Failed to send text message to {}: {e}", item.recipient_jid)
            })?;
        } else {
            // Media message handling (payload contains media reference)
            let caption = item
                .payload
                .get("caption")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let msg = if !caption.is_empty() {
                wa::Message::text(caption)
            } else {
                wa::Message::default()
            };

            wa_client.send_message(jid, msg).await.map_err(|e| {
                anyhow!(
                    "Failed to send media message to {}: {e}",
                    item.recipient_jid
                )
            })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_typing_duration_caps() {
        // Short text: 10 chars -> base(1500) + 1*150 = 1650 + jitter
        let dur = calculate_typing_duration(10, 0);
        assert_eq!(dur, 1650);

        // Long text: 1000 chars -> base(1500) + 100*150 = 16500 -> capped at 5000
        let dur_capped = calculate_typing_duration(1000, 200);
        assert_eq!(dur_capped, 5000);
    }

    #[test]
    fn test_parse_jid_formats() {
        let jid1 = parse_jid("919876543210@s.whatsapp.net").unwrap();
        assert_eq!(jid1.to_string(), "919876543210@s.whatsapp.net");

        let jid2 = parse_jid("+91 98765 43210").unwrap();
        assert_eq!(jid2.to_string(), "919876543210@s.whatsapp.net");

        let jid3 = parse_jid("+1 (555) 234-5678").unwrap();
        assert_eq!(jid3.to_string(), "15552345678@s.whatsapp.net");

        let group_jid = parse_jid("120363021033254949@g.us").unwrap();
        assert_eq!(group_jid.to_string(), "120363021033254949@g.us");

        let invalid = parse_jid("   ");
        assert!(invalid.is_err());
    }
}
