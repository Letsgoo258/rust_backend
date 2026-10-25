use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutboxStatus {
    Pending,
    Processing,
    Sent,
    Failed,
    Cancelled,
}

impl std::str::FromStr for OutboxStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(OutboxStatus::Pending),
            "processing" => Ok(OutboxStatus::Processing),
            "sent" => Ok(OutboxStatus::Sent),
            "failed" => Ok(OutboxStatus::Failed),
            "cancelled" => Ok(OutboxStatus::Cancelled),
            other => Err(anyhow!("Unknown outbox status: {other}")),
        }
    }
}

impl OutboxStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutboxStatus::Pending => "pending",
            OutboxStatus::Processing => "processing",
            OutboxStatus::Sent => "sent",
            OutboxStatus::Failed => "failed",
            OutboxStatus::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OutboxItem {
    pub id: Uuid,
    pub recipient_jid: String,
    pub message_type: String,
    pub payload: serde_json::Value,
    pub attempt_count: i32,
    pub max_attempts: i32,
}

#[derive(Clone)]
pub struct QueueManager {
    pool: PgPool,
}

impl QueueManager {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Enqueue an outbound message to `whatsapp_outbox` with optional idempotency key.
    pub async fn enqueue(
        &self,
        recipient_jid: &str,
        message_type: &str,
        payload: serde_json::Value,
        priority: i32,
        idempotency_key: Option<&str>,
    ) -> Result<Uuid> {
        if let Some(key) = idempotency_key {
            let row: Option<(Uuid,)> = sqlx::query_as(
                r#"
                INSERT INTO whatsapp_outbox (recipient_jid, message_type, payload, priority, idempotency_key)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (idempotency_key) DO NOTHING
                RETURNING id
                "#
            )
            .bind(recipient_jid)
            .bind(message_type)
            .bind(&payload)
            .bind(priority)
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow!("Enqueueing WhatsApp message with idempotency key: {e}"))?;

            if let Some((id,)) = row {
                Ok(id)
            } else {
                // If conflict occurred, return the existing row UUID
                let existing: (Uuid,) =
                    sqlx::query_as("SELECT id FROM whatsapp_outbox WHERE idempotency_key = $1")
                        .bind(key)
                        .fetch_one(&self.pool)
                        .await
                        .map_err(|e| {
                            anyhow!("Fetching existing outbox entry by idempotency key: {e}")
                        })?;

                Ok(existing.0)
            }
        } else {
            let row: (Uuid,) = sqlx::query_as(
                r#"
                INSERT INTO whatsapp_outbox (recipient_jid, message_type, payload, priority)
                VALUES ($1, $2, $3, $4)
                RETURNING id
                "#,
            )
            .bind(recipient_jid)
            .bind(message_type)
            .bind(&payload)
            .bind(priority)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow!("Enqueueing WhatsApp message: {e}"))?;

            Ok(row.0)
        }
    }

    /// Atomically select and transition the next eligible pending message to 'processing'
    /// using `FOR UPDATE SKIP LOCKED`.
    pub async fn dequeue_next(&self) -> Result<Option<OutboxItem>> {
        let row_opt = sqlx::query(
            r#"
            WITH next_row AS (
                SELECT id
                FROM whatsapp_outbox
                WHERE status = 'pending' AND scheduled_at <= NOW()
                ORDER BY priority ASC, scheduled_at ASC
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            UPDATE whatsapp_outbox o
            SET status = 'processing',
                started_at = NOW(),
                attempt_count = o.attempt_count + 1
            FROM next_row
            WHERE o.id = next_row.id
            RETURNING o.id, o.recipient_jid, o.message_type, o.payload, o.attempt_count, o.max_attempts
            "#
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!("Dequeuing next pending WhatsApp message: {e}"))?;

        match row_opt {
            Some(row) => Ok(Some(OutboxItem {
                id: row.get("id"),
                recipient_jid: row.get("recipient_jid"),
                message_type: row.get("message_type"),
                payload: row.get("payload"),
                attempt_count: row.get("attempt_count"),
                max_attempts: row.get("max_attempts"),
            })),
            None => Ok(None),
        }
    }

    /// Mark an outbox item as successfully sent.
    pub async fn mark_sent(&self, id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE whatsapp_outbox
            SET status = 'sent',
                completed_at = NOW(),
                last_error = NULL
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!("Marking outbox message sent: {e}"))?;

        Ok(())
    }

    /// Mark an outbox item as failed or reschedule with exponential backoff.
    pub async fn mark_failed(
        &self,
        id: Uuid,
        error_msg: &str,
        attempt_count: i32,
        max_attempts: i32,
    ) -> Result<()> {
        if attempt_count < max_attempts {
            // Exponential backoff: 2^attempt * 5 seconds (capped at 320s)
            let shift = attempt_count.clamp(1, 6);
            let backoff_secs = (1i64 << shift) * 5;

            sqlx::query(
                r#"
                UPDATE whatsapp_outbox
                SET status = 'pending',
                    scheduled_at = NOW() + make_interval(secs => $2::double precision),
                    last_error = $3
                WHERE id = $1
                "#,
            )
            .bind(id)
            .bind(backoff_secs as f64)
            .bind(error_msg)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow!("Rescheduling outbox message with backoff: {e}"))?;
        } else {
            sqlx::query(
                r#"
                UPDATE whatsapp_outbox
                SET status = 'failed',
                    completed_at = NOW(),
                    last_error = $2
                WHERE id = $1
                "#,
            )
            .bind(id)
            .bind(error_msg)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow!("Marking outbox message failed: {e}"))?;
        }

        Ok(())
    }

    /// Retrieve the current status of an outbox message.
    pub async fn get_status(&self, id: Uuid) -> Result<OutboxStatus> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT status FROM whatsapp_outbox WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| anyhow!("Querying outbox status: {e}"))?;

        match row {
            Some((status_str,)) => status_str.parse(),
            None => Err(anyhow!("Outbox entry with id {} not found", id)),
        }
    }
}
