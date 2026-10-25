use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppInboundRow {
    pub id: Uuid,
    pub sender_jid: String,
    pub message_type: String,
    pub payload: serde_json::Value,
    pub whatsapp_timestamp: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppOutboxRow {
    pub id: Uuid,
    pub recipient_jid: String,
    pub message_type: String,
    pub payload: serde_json::Value,
    pub status: String,
    pub priority: i32,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
    pub scheduled_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WhatsAppStats {
    pub total_inbound: i64,
    pub inbound_today: i64,
    pub total_outbox: i64,
    pub pending_outbox: i64,
    pub sent_outbox: i64,
    pub failed_outbox: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppUserSessionRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub school_id: Uuid,
    pub sender_jid: String,
    pub phone: String,
    pub role_code: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub display_name: Option<String>,
    pub username: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub device_fingerprint: Option<String>,
    pub last_active_at: Option<DateTime<Utc>>,
    pub is_revoked: bool,
    pub suspicious_reason: Option<String>,
    pub login_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppLinkTokenRow {
    pub token: String,
    pub user_id: Uuid,
    pub school_id: Uuid,
    pub role_code: String,
    pub ip_address: String,
    pub user_agent: Option<String>,
    pub device_info: serde_json::Value,
    pub consumed: bool,
    pub consumed_by_jid: Option<String>,
    pub consumed_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppVerificationTokenRow {
    pub token: String,
    pub sender_jid: String,
    pub phone: String,
    pub otp_code: Option<String>,
    pub otp_expires_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub verified: bool,
    pub created_at: DateTime<Utc>,
}

pub struct WhatsAppRepository;

impl WhatsAppRepository {
    /// Record an inbound message into `whatsapp_inbound`.
    pub async fn record_inbound(
        pool: &PgPool,
        sender_jid: &str,
        message_type: &str,
        payload: serde_json::Value,
        wa_ts: DateTime<Utc>,
    ) -> Result<Uuid, AppError> {
        let row = sqlx::query(
            r#"
            INSERT INTO whatsapp_inbound (sender_jid, message_type, payload, whatsapp_timestamp, received_at)
            VALUES ($1, $2, $3, $4, NOW())
            RETURNING id
            "#,
        )
        .bind(sender_jid)
        .bind(message_type)
        .bind(payload)
        .bind(wa_ts)
        .fetch_one(pool)
        .await?;

        Ok(row.get("id"))
    }

    /// Retrieve the most recent conversation messages for a sender (for AI context).
    pub async fn get_recent_conversation(
        pool: &PgPool,
        sender_jid: &str,
        limit: i64,
    ) -> Result<Vec<WhatsAppInboundRow>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT id, sender_jid, message_type, payload, whatsapp_timestamp, received_at
            FROM whatsapp_inbound
            WHERE sender_jid = $1
            ORDER BY received_at DESC
            LIMIT $2
            "#,
        )
        .bind(sender_jid)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        let messages = rows
            .into_iter()
            .map(|r| WhatsAppInboundRow {
                id: r.get("id"),
                sender_jid: r.get("sender_jid"),
                message_type: r.get("message_type"),
                payload: r.get("payload"),
                whatsapp_timestamp: r.get("whatsapp_timestamp"),
                received_at: r.get("received_at"),
            })
            .collect();

        Ok(messages)
    }

    /// Retrieve paginated list of inbound messages for audit/dashboard.
    pub async fn get_inbound_messages(
        pool: &PgPool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<WhatsAppInboundRow>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT id, sender_jid, message_type, payload, whatsapp_timestamp, received_at
            FROM whatsapp_inbound
            ORDER BY received_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let messages = rows
            .into_iter()
            .map(|r| WhatsAppInboundRow {
                id: r.get("id"),
                sender_jid: r.get("sender_jid"),
                message_type: r.get("message_type"),
                payload: r.get("payload"),
                whatsapp_timestamp: r.get("whatsapp_timestamp"),
                received_at: r.get("received_at"),
            })
            .collect();

        Ok(messages)
    }

    /// Retrieve paginated list of outbox items.
    pub async fn get_outbox_queue(
        pool: &PgPool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<WhatsAppOutboxRow>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT id, recipient_jid, message_type, payload, status, priority, attempt_count,
                   max_attempts, last_error, scheduled_at, created_at
            FROM whatsapp_outbox
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let items = rows
            .into_iter()
            .map(|r| WhatsAppOutboxRow {
                id: r.get("id"),
                recipient_jid: r.get("recipient_jid"),
                message_type: r.get("message_type"),
                payload: r.get("payload"),
                status: r.get("status"),
                priority: r.get("priority"),
                attempt_count: r.get("attempt_count"),
                max_attempts: r.get("max_attempts"),
                last_error: r.get("last_error"),
                scheduled_at: r.get("scheduled_at"),
                created_at: r.get("created_at"),
            })
            .collect();

        Ok(items)
    }

    /// Aggregate overall metrics for WhatsApp communication.
    pub async fn get_stats(pool: &PgPool) -> Result<WhatsAppStats, AppError> {
        let inbound_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM whatsapp_inbound")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

        let inbound_today: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM whatsapp_inbound WHERE received_at >= CURRENT_DATE",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        let outbox_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM whatsapp_outbox")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

        let pending_outbox: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM whatsapp_outbox WHERE status IN ('pending', 'processing')",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        let sent_outbox: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM whatsapp_outbox WHERE status = 'sent'")
                .fetch_one(pool)
                .await
                .unwrap_or(0);

        let failed_outbox: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM whatsapp_outbox WHERE status = 'failed'")
                .fetch_one(pool)
                .await
                .unwrap_or(0);

        Ok(WhatsAppStats {
            total_inbound: inbound_total,
            inbound_today,
            total_outbox: outbox_total,
            pending_outbox,
            sent_outbox,
            failed_outbox,
        })
    }

    /// Retrieve active WhatsApp user session for a given JID.
    pub async fn get_session_by_jid(
        pool: &PgPool,
        sender_jid: &str,
    ) -> Result<Option<WhatsAppUserSessionRow>, AppError> {
        let phone: String = sender_jid
            .split('@')
            .next()
            .unwrap_or("")
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect();

        let row = sqlx::query(
            r#"
            SELECT
                s.id, s.user_id, s.school_id, s.sender_jid, s.phone, s.role_code,
                s.expires_at, s.created_at, s.updated_at,
                s.ip_address, s.user_agent, s.device_fingerprint, s.last_active_at,
                s.is_revoked, s.suspicious_reason, s.login_token,
                u.display_name, u.username
            FROM whatsapp_user_sessions s
            JOIN users u ON s.user_id = u.id
            WHERE (s.sender_jid = $1 OR (s.phone != '' AND s.phone = $2))
            ORDER BY s.updated_at DESC
            LIMIT 1
            "#,
        )
        .bind(sender_jid)
        .bind(&phone)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| WhatsAppUserSessionRow {
            id: r.get("id"),
            user_id: r.get("user_id"),
            school_id: r.get("school_id"),
            sender_jid: r.get("sender_jid"),
            phone: r.get("phone"),
            role_code: r.get("role_code"),
            expires_at: r.get("expires_at"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            display_name: r.get("display_name"),
            username: r.get("username"),
            ip_address: r.get("ip_address"),
            user_agent: r.get("user_agent"),
            device_fingerprint: r.get("device_fingerprint"),
            last_active_at: r.get("last_active_at"),
            is_revoked: r.get("is_revoked"),
            suspicious_reason: r.get("suspicious_reason"),
            login_token: r.get("login_token"),
        }))
    }

    /// Retrieve all authorized capability keys for an active WhatsApp session from `whatsapp_capabilities`.
    pub async fn get_capabilities_by_session(
        pool: &PgPool,
        session_id: Uuid,
    ) -> Result<Vec<String>, AppError> {
        let rows = sqlx::query_scalar::<_, String>(
            r#"
            SELECT capability_key
            FROM whatsapp_capabilities
            WHERE session_id = $1
            ORDER BY capability_key ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// Slide session expiry forward (e.g. 7 days).
    pub async fn slide_session_expiry(
        pool: &PgPool,
        session_id: Uuid,
        duration_days: i64,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE whatsapp_user_sessions
            SET expires_at = NOW() + ($2 * INTERVAL '1 day'), updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(session_id)
        .bind(duration_days)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Create or renew an active WhatsApp user session with full security metadata and populate `whatsapp_capabilities`.
    pub async fn create_or_renew_session_with_meta(
        pool: &PgPool,
        user_id: Uuid,
        school_id: Uuid,
        sender_jid: &str,
        phone: &str,
        role_code: &str,
        capabilities: &[String],
        duration_days: i64,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
        device_fingerprint: Option<&str>,
        login_token: Option<&str>,
    ) -> Result<Uuid, AppError> {
        let mut tx = pool.begin().await?;

        let row = sqlx::query(
            r#"
            INSERT INTO whatsapp_user_sessions (
                user_id, school_id, sender_jid, phone, role_code, expires_at,
                ip_address, user_agent, device_fingerprint, login_token, is_revoked, last_active_at
            )
            VALUES ($1, $2, $3, $4, $5, NOW() + ($6 * INTERVAL '1 day'), $7, $8, $9, $10, false, NOW())
            ON CONFLICT (sender_jid) DO UPDATE SET
                user_id = EXCLUDED.user_id,
                school_id = EXCLUDED.school_id,
                phone = EXCLUDED.phone,
                role_code = EXCLUDED.role_code,
                expires_at = NOW() + ($6 * INTERVAL '1 day'),
                ip_address = EXCLUDED.ip_address,
                user_agent = EXCLUDED.user_agent,
                device_fingerprint = EXCLUDED.device_fingerprint,
                login_token = EXCLUDED.login_token,
                is_revoked = false,
                suspicious_reason = NULL,
                last_active_at = NOW(),
                updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(user_id)
        .bind(school_id)
        .bind(sender_jid)
        .bind(phone)
        .bind(role_code)
        .bind(duration_days)
        .bind(ip_address)
        .bind(user_agent)
        .bind(device_fingerprint)
        .bind(login_token)
        .fetch_one(&mut *tx)
        .await?;

        let session_id: Uuid = row.get("id");

        // Clear existing session capabilities
        sqlx::query("DELETE FROM whatsapp_capabilities WHERE session_id = $1")
            .bind(session_id)
            .execute(&mut *tx)
            .await?;

        // Batch insert capabilities
        for cap in capabilities {
            sqlx::query(
                r#"
                INSERT INTO whatsapp_capabilities (session_id, user_id, role_code, capability_key)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (session_id, capability_key) DO NOTHING
                "#,
            )
            .bind(session_id)
            .bind(user_id)
            .bind(role_code)
            .bind(cap)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(session_id)
    }

    /// Create or renew an active WhatsApp user session and populate `whatsapp_capabilities`.
    pub async fn create_or_renew_session(
        pool: &PgPool,
        user_id: Uuid,
        school_id: Uuid,
        sender_jid: &str,
        phone: &str,
        role_code: &str,
        capabilities: &[String],
        duration_days: i64,
    ) -> Result<Uuid, AppError> {
        Self::create_or_renew_session_with_meta(
            pool,
            user_id,
            school_id,
            sender_jid,
            phone,
            role_code,
            capabilities,
            duration_days,
            None,
            None,
            None,
            None,
        )
        .await
    }

    /// Record message activity timestamp on active session.
    pub async fn record_session_activity(pool: &PgPool, session_id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE whatsapp_user_sessions
            SET last_active_at = NOW(), updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(session_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Drop a session immediately due to suspicious behavior, token mismatch, or admin revocation.
    pub async fn drop_session(
        pool: &PgPool,
        sender_jid: &str,
        reason: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE whatsapp_user_sessions
            SET is_revoked = true, suspicious_reason = $2, updated_at = NOW()
            WHERE sender_jid = $1
            "#,
        )
        .bind(sender_jid)
        .bind(reason)
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM whatsapp_capabilities
            WHERE session_id IN (
                SELECT id FROM whatsapp_user_sessions WHERE sender_jid = $1
            )
            "#,
        )
        .bind(sender_jid)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Store a one-time link token generated by an authenticated ERP user on the web.
    pub async fn create_link_token(
        pool: &PgPool,
        token: &str,
        user_id: Uuid,
        school_id: Uuid,
        role_code: &str,
        ip_address: &str,
        user_agent: Option<&str>,
        device_info: serde_json::Value,
        expires_in_minutes: i64,
    ) -> Result<String, AppError> {
        sqlx::query(
            r#"
            INSERT INTO whatsapp_link_tokens (
                token, user_id, school_id, role_code, ip_address, user_agent, device_info, expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW() + ($8 * INTERVAL '1 minute'))
            "#,
        )
        .bind(token)
        .bind(user_id)
        .bind(school_id)
        .bind(role_code)
        .bind(ip_address)
        .bind(user_agent)
        .bind(device_info)
        .bind(expires_in_minutes)
        .execute(pool)
        .await?;

        Ok(token.to_string())
    }

    /// Find an active, unconsumed link token by code.
    pub async fn get_link_token(
        pool: &PgPool,
        token: &str,
    ) -> Result<Option<WhatsAppLinkTokenRow>, AppError> {
        let clean_token = token.trim();
        let row = sqlx::query_as::<
            _,
            (
                String,
                Uuid,
                Uuid,
                String,
                String,
                Option<String>,
                serde_json::Value,
                bool,
                Option<String>,
                Option<DateTime<Utc>>,
                DateTime<Utc>,
                DateTime<Utc>,
            ),
        >(
            r#"
            SELECT
                token, user_id, school_id, role_code, ip_address, user_agent,
                device_info, consumed, consumed_by_jid, consumed_at, expires_at, created_at
            FROM whatsapp_link_tokens
            WHERE UPPER(token) = UPPER($1)
            LIMIT 1
            "#,
        )
        .bind(clean_token)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| WhatsAppLinkTokenRow {
            token: r.0,
            user_id: r.1,
            school_id: r.2,
            role_code: r.3,
            ip_address: r.4,
            user_agent: r.5,
            device_info: r.6,
            consumed: r.7,
            consumed_by_jid: r.8,
            consumed_at: r.9,
            expires_at: r.10,
            created_at: r.11,
        }))
    }

    /// Mark a link token as consumed when linked in chat.
    pub async fn consume_link_token(
        pool: &PgPool,
        token: &str,
        sender_jid: &str,
    ) -> Result<bool, AppError> {
        let clean_token = token.trim();
        let res = sqlx::query(
            r#"
            UPDATE whatsapp_link_tokens
            SET consumed = true, consumed_by_jid = $2, consumed_at = NOW()
            WHERE UPPER(token) = UPPER($1) AND consumed = false AND expires_at > NOW()
            "#,
        )
        .bind(clean_token)
        .bind(sender_jid)
        .execute(pool)
        .await?;

        Ok(res.rows_affected() > 0)
    }

    /// Terminate a session (e.g. user typed 'logout').
    pub async fn terminate_session(pool: &PgPool, sender_jid: &str) -> Result<bool, AppError> {
        let res = sqlx::query("DELETE FROM whatsapp_user_sessions WHERE sender_jid = $1")
            .bind(sender_jid)
            .execute(pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Create or retrieve an unexpired verification token for web onboarding.
    pub async fn create_verification_token(
        pool: &PgPool,
        sender_jid: &str,
        phone: &str,
    ) -> Result<String, AppError> {
        let token = uuid::Uuid::new_v4().to_string().replace("-", "");

        sqlx::query(
            r#"
            INSERT INTO whatsapp_verification_tokens (token, sender_jid, phone, expires_at)
            VALUES ($1, $2, $3, NOW() + INTERVAL '30 minutes')
            "#,
        )
        .bind(&token)
        .bind(sender_jid)
        .bind(phone)
        .execute(pool)
        .await?;

        Ok(token)
    }

    /// Get verification token record.
    pub async fn get_verification_token(
        pool: &PgPool,
        token: &str,
    ) -> Result<Option<WhatsAppVerificationTokenRow>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT token, sender_jid, phone, otp_code, otp_expires_at, expires_at, verified, created_at
            FROM whatsapp_verification_tokens
            WHERE token = $1 AND expires_at > NOW()
            LIMIT 1
            "#,
        )
        .bind(token)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| WhatsAppVerificationTokenRow {
            token: r.get("token"),
            sender_jid: r.get("sender_jid"),
            phone: r.get("phone"),
            otp_code: r.get("otp_code"),
            otp_expires_at: r.get("otp_expires_at"),
            expires_at: r.get("expires_at"),
            verified: r.get("verified"),
            created_at: r.get("created_at"),
        }))
    }

    /// Set generated OTP for a verification token.
    pub async fn set_verification_otp(
        pool: &PgPool,
        token: &str,
        otp_code: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE whatsapp_verification_tokens
            SET otp_code = $1, otp_expires_at = NOW() + INTERVAL '10 minutes'
            WHERE token = $2
            "#,
        )
        .bind(otp_code)
        .bind(token)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Mark token as verified.
    pub async fn mark_token_verified(pool: &PgPool, token: &str) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE whatsapp_verification_tokens
            SET verified = true
            WHERE token = $1
            "#,
        )
        .bind(token)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Look up an existing user by matching phone number.
    pub async fn find_user_by_phone(
        pool: &PgPool,
        phone: &str,
    ) -> Result<Option<(Uuid, Uuid, String, String, Vec<String>)>, AppError> {
        let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
        let last_10 = if digits.len() >= 10 {
            &digits[digits.len() - 10..]
        } else {
            &digits
        };

        let row = sqlx::query(
            r#"
            SELECT
                u.id, u.school_id, u.username, u.display_name,
                COALESCE(array_agg(DISTINCT r.code::TEXT) FILTER (WHERE r.code IS NOT NULL), ARRAY[]::TEXT[]) as roles
            FROM users u
            LEFT JOIN user_roles ur ON ur.user_id = u.id
            LEFT JOIN roles r ON ur.role_id = r.id
            WHERE u.phone ILIKE '%' || $1
              AND u.status = 'ACTIVE'
            GROUP BY u.id, u.school_id, u.username, u.display_name
            LIMIT 1
            "#,
        )
        .bind(last_10)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| {
            (
                r.get("id"),
                r.get("school_id"),
                r.get("username"),
                r.get::<Option<String>, _>("display_name")
                    .unwrap_or_else(|| r.get("username")),
                r.get("roles"),
            )
        }))
    }
}
