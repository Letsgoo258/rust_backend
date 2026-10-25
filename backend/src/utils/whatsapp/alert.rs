use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertReason {
    LoggedOut,
    TemporaryBan { until: DateTime<Utc> },
    KeyRotationFailed(String),
    HeartbeatDisconnected,
    ManualTest,
}

impl AlertReason {
    pub fn title(&self) -> &'static str {
        match self {
            AlertReason::LoggedOut => "WhatsApp Session Expired / Device Unlinked",
            AlertReason::TemporaryBan { .. } => "WhatsApp Account Temporarily Banned",
            AlertReason::KeyRotationFailed(_) => "WhatsApp Cryptographic Key Rotation Failed",
            AlertReason::HeartbeatDisconnected => "WhatsApp Service Disconnected",
            AlertReason::ManualTest => "WhatsApp Alert System Diagnostic Test",
        }
    }

    pub fn description(&self) -> String {
        match self {
            AlertReason::LoggedOut => {
                "The WhatsApp Web/Multi-Device session was disconnected or unlinked by WhatsApp. Outbound automated messages (attendance, fee reminders, exam notifications) are currently paused until an administrator re-scans the pairing QR code.".to_string()
            }
            AlertReason::TemporaryBan { until } => {
                format!(
                    "WhatsApp has placed a temporary rate-limit ban on this phone number until {}. The outgoing message queue is paused to protect the account reputation.",
                    until.format("%Y-%m-%d %H:%M:%S UTC")
                )
            }
            AlertReason::KeyRotationFailed(err) => {
                format!(
                    "The automatic Signal pre-key rotation failed with error: {}. Future end-to-end encrypted session establishment may be impaired if unaddressed.",
                    err
                )
            }
            AlertReason::HeartbeatDisconnected => {
                "The WhatsApp client connection has been lost and auto-reconnect was unable to restore the session.".to_string()
            }
            AlertReason::ManualTest => {
                "This is a verified test email sent by an administrator from the ERAVAYA School Settings dashboard. Your alert notification pipeline is operating normally.".to_string()
            }
        }
    }
}

pub struct WhatsAppAlertManager {
    pool: PgPool,
    last_alert_at: Arc<RwLock<Option<DateTime<Utc>>>>,
    cooldown_secs: i64,
}

impl WhatsAppAlertManager {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            last_alert_at: Arc::new(RwLock::new(None)),
            cooldown_secs: 3600, // 1 hour cooldown between alerts
        }
    }

    pub fn with_cooldown(pool: PgPool, cooldown_secs: i64) -> Self {
        Self {
            pool,
            last_alert_at: Arc::new(RwLock::new(None)),
            cooldown_secs,
        }
    }

    /// Retrieve administrator email addresses from database or fallback environment variables.
    pub async fn resolve_admin_emails(&self) -> Vec<String> {
        let mut emails = Vec::new();

        // 1. Query School Admins and Super Admins from database
        let query_res: Result<Vec<String>, _> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT u.email 
            FROM users u
            JOIN user_roles ur ON u.id = ur.user_id
            JOIN roles r ON ur.role_id = r.id
            WHERE r.code IN ('SCHOOL_ADMIN', 'SUPER_ADMIN')
              AND u.email IS NOT NULL 
              AND TRIM(u.email) != ''
            LIMIT 10
            "#,
        )
        .fetch_all(&self.pool)
        .await;

        match query_res {
            Ok(db_emails) => {
                for email in db_emails {
                    let trimmed = email.trim().to_string();
                    if !trimmed.is_empty() && !emails.contains(&trimmed) {
                        emails.push(trimmed);
                    }
                }
            }
            Err(e) => {
                warn!("Could not fetch admin emails from DB: {e:#}");
            }
        }

        // 2. Fallback to environment variables
        if let Ok(alert_email) = std::env::var("WHATSAPP_ALERT_EMAIL") {
            for email in alert_email.split(',') {
                let trimmed = email.trim().to_string();
                if !trimmed.is_empty() && !emails.contains(&trimmed) {
                    emails.push(trimmed);
                }
            }
        }
        if let Ok(admin_email) = std::env::var("ADMIN_EMAIL") {
            let trimmed = admin_email.trim().to_string();
            if !trimmed.is_empty() && !emails.contains(&trimmed) {
                emails.push(trimmed);
            }
        }

        emails
    }

    /// Dispatches a critical email alert if not on cooldown.
    pub async fn send_alert(&self, reason: AlertReason, force: bool) -> anyhow::Result<usize> {
        let now = Utc::now();

        // Check cooldown unless forced
        if !force {
            let mut last_at_guard = self.last_alert_at.write().await;
            if let Some(last_at) = *last_at_guard {
                let elapsed = (now - last_at).num_seconds();
                if elapsed < self.cooldown_secs {
                    info!(
                        "Skipping WhatsApp critical alert email (cooldown active: {}s elapsed / {}s required)",
                        elapsed, self.cooldown_secs
                    );
                    return Ok(0);
                }
            }
            *last_at_guard = Some(now);
        }

        let recipients = self.resolve_admin_emails().await;
        if recipients.is_empty() {
            warn!("No admin email recipients found to receive WhatsApp critical alert!");
            return Ok(0);
        }

        let app_url = std::env::var("APP_URL")
            .unwrap_or_else(|_| "https://my-frontend-kappa-five.vercel.app".to_string());
        let settings_url = format!(
            "{}/dashboard/school-setting?tab=whatsapp",
            app_url.trim_end_matches('/')
        );

        let subject = format!("🚨 CRITICAL: {}", reason.title());
        let html_body = build_alert_html(&reason, &settings_url, now);

        let mut sent_count = 0;
        for recipient in &recipients {
            info!("Dispatching critical WhatsApp alert email to {}", recipient);
            match crate::utils::email::send_email(recipient, &subject, &html_body).await {
                Ok(_) => {
                    sent_count += 1;
                }
                Err(e) => {
                    error!(recipient = %recipient, error = %e, "Failed to send WhatsApp alert email");
                }
            }
        }

        Ok(sent_count)
    }
}

fn build_alert_html(reason: &AlertReason, settings_url: &str, timestamp: DateTime<Utc>) -> String {
    let title = reason.title();
    let desc = reason.description();
    let formatted_time = timestamp.format("%B %d, %Y at %H:%M:%S UTC").to_string();

    let is_test = matches!(reason, AlertReason::ManualTest);
    let border_color = if is_test { "#2563eb" } else { "#dc2626" };
    let badge_bg = if is_test { "#dbeafe" } else { "#fee2e2" };
    let badge_text = if is_test { "#1d4ed8" } else { "#991b1b" };
    let badge_label = if is_test {
        "DIAGNOSTIC NOTICE"
    } else {
        "ACTION REQUIRED"
    };

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title}</title>
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; background-color: #f8fafc; margin: 0; padding: 24px; color: #1e293b;">
  <div style="max-width: 600px; margin: 0 auto; background: #ffffff; border-radius: 12px; border: 1px solid #e2e8f0; overflow: hidden; box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.05);">
    <div style="background-color: {border_color}; padding: 18px 24px; color: #ffffff;">
      <h1 style="margin: 0; font-size: 20px; font-weight: 700; letter-spacing: -0.02em;">ERAVAYA Alert System</h1>
    </div>

    <div style="padding: 24px;">
      <div style="display: inline-block; background-color: {badge_bg}; color: {badge_text}; font-size: 11px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.05em; padding: 4px 10px; border-radius: 9999px; margin-bottom: 16px;">
        {badge_label}
      </div>

      <h2 style="margin: 0 0 12px 0; font-size: 18px; color: #0f172a; font-weight: 600;">{title}</h2>

      <p style="margin: 0 0 20px 0; font-size: 15px; line-height: 1.6; color: #334155;">
        {desc}
      </p>

      <div style="background-color: #f1f5f9; border-radius: 8px; padding: 14px 18px; margin-bottom: 24px; font-size: 13px; color: #475569;">
        <div style="margin-bottom: 6px;"><strong>Incident Timestamp:</strong> {formatted_time}</div>
        <div style="margin-bottom: 6px;"><strong>Service:</strong> Embedded In-Process WhatsApp Service</div>
        <div><strong>Impact:</strong> Outgoing notifications paused until re-authenticated</div>
      </div>

      <div style="text-align: center; margin: 30px 0;">
        <a href="{settings_url}" style="background-color: #0f172a; color: #ffffff; padding: 12px 24px; font-size: 14px; font-weight: 600; text-decoration: none; border-radius: 8px; display: inline-block;">
          Open WhatsApp Settings & Scan QR
        </a>
      </div>

      <p style="margin: 0; font-size: 12px; color: #94a3b8; text-align: center; line-height: 1.5;">
        You are receiving this automated alert because you are listed as an administrator in the ERAVAYA school management system.
      </p>
    </div>
  </div>
</body>
</html>"#
    )
}
