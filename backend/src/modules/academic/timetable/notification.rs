use chrono::NaiveDate;
use sqlx::PgPool;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::modules::academic::timetable::repository::TimetableRepository;
use crate::utils::whatsapp::QueueManager;

/// Formats a phone string into a valid WhatsApp JID (e.g. "919876543210@s.whatsapp.net")
fn format_whatsapp_jid(phone: &str) -> Option<String> {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let normalized = if digits.len() == 10
        && (digits.starts_with('6')
            || digits.starts_with('7')
            || digits.starts_with('8')
            || digits.starts_with('9'))
    {
        format!("91{digits}")
    } else {
        digits
    };
    Some(format!("{normalized}@s.whatsapp.net"))
}

/// Enqueues both Email and WhatsApp notifications whenever a class substitution is processed.
pub async fn queue_substitution_notifications(
    pool: &PgPool,
    school_id: Uuid,
    sub_id: Uuid,
    slot_id: Uuid,
    substitute_staff_id: Uuid,
    date: NaiveDate,
    reason: Option<&str>,
) {
    // 1. Fetch substitution contextual details from repository
    let ctx = match TimetableRepository::get_substitution_notification_context(
        pool,
        school_id,
        slot_id,
        substitute_staff_id,
    )
    .await
    {
        Ok(Some(c)) => c,
        Ok(None) => {
            warn!(slot_id = %slot_id, "Cannot find timetable slot context to queue substitution notifications");
            return;
        }
        Err(e) => {
            error!(error = %e, "Database error querying substitution context for notifications");
            return;
        }
    };

    let period_number = ctx.period_number;
    let start_short = &ctx.start_time[..ctx.start_time.len().min(5)];
    let end_short = &ctx.end_time[..ctx.end_time.len().min(5)];
    let class_name = ctx.class_name;
    let section_name = ctx.section_name;
    let subject_name = ctx.subject_name;
    let subject_code = ctx.subject_code;
    let school_name = ctx.school_name;

    let sub_user_id = ctx.sub_user_id;
    let sub_teacher_name = ctx.sub_teacher_name;
    let sub_email = ctx.sub_email;
    let sub_phone = ctx.sub_phone;

    let orig_user_id = ctx.orig_user_id;
    let orig_teacher_name = ctx.orig_teacher_name;
    let orig_email = ctx.orig_email;
    let orig_phone = ctx.orig_phone;

    let display_reason = reason.unwrap_or("Academic substitution arrangement");

    // ─── 2. WhatsApp Queue (whatsapp_outbox) ──────────────────────────────────
    let wa_queue = QueueManager::new(pool.clone());

    // A. WhatsApp to Substitute Teacher
    if let Some(ref phone) = sub_phone {
        if let Some(jid) = format_whatsapp_jid(phone) {
            let wa_text = format!(
                "🔔 *CLASS SUBSTITUTION ASSIGNMENT*\n\
                *School*: {school_name}\n\n\
                Hello {sub_teacher_name},\n\
                You have been assigned to substitute for *{orig_teacher_name}*:\n\n\
                📅 *Date*: {date}\n\
                ⏰ *Period*: Period {period_number} ({start_short} - {end_short})\n\
                🏫 *Class*: {class_name} - Section {section_name}\n\
                📚 *Subject*: {subject_name} ({subject_code})\n\
                📝 *Reason*: {display_reason}\n\n\
                Please report to the classroom promptly. Thank you!\n\
                — *Eravaya Academic Operations*"
            );
            let wa_payload = serde_json::json!({
                "text": wa_text,
                "skip_typing": true,
            });
            let idem_key = format!("sub-wa-sub-{sub_id}-{date}");
            match wa_queue
                .enqueue(&jid, "text", wa_payload, 10, Some(&idem_key))
                .await
            {
                Ok(qid) => {
                    info!(queue_id = %qid, jid = %jid, "Enqueued WhatsApp notification to substitute faculty")
                }
                Err(e) => {
                    warn!(error = %e, "Failed to enqueue WhatsApp notification to substitute")
                }
            }
        }
    }

    // B. WhatsApp to Original Teacher
    if let Some(ref phone) = orig_phone {
        if let Some(jid) = format_whatsapp_jid(phone) {
            let wa_text = format!(
                "ℹ️ *SUBSTITUTION ARRANGED*\n\
                *School*: {school_name}\n\n\
                Hello {orig_teacher_name},\n\
                Your scheduled period has been covered by *{sub_teacher_name}*:\n\n\
                📅 *Date*: {date}\n\
                ⏰ *Period*: Period {period_number} ({start_short} - {end_short})\n\
                🏫 *Class*: {class_name} - Section {section_name}\n\
                📚 *Subject*: {subject_name}\n\
                👤 *Substitute*: {sub_teacher_name}\n\
                📝 *Reason*: {display_reason}\n\n\
                Your class coverage is confirmed.\n\
                — *Eravaya Academic Operations*"
            );
            let wa_payload = serde_json::json!({
                "text": wa_text,
                "skip_typing": true,
            });
            let idem_key = format!("sub-wa-orig-{sub_id}-{date}");
            match wa_queue
                .enqueue(&jid, "text", wa_payload, 10, Some(&idem_key))
                .await
            {
                Ok(qid) => {
                    info!(queue_id = %qid, jid = %jid, "Enqueued WhatsApp notification to original faculty")
                }
                Err(e) => {
                    warn!(error = %e, "Failed to enqueue WhatsApp notification to original faculty")
                }
            }
        }
    }

    // ─── 3. Email Queue (notifications table) & Real-time Async Dispatch ───────

    // A. Substitute Teacher Email
    let sub_email_summary = format!(
        "Class Substitution Assigned: Period {period_number} ({class_name}-{section_name}) on {date}"
    );
    let sub_notif_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        INSERT INTO notifications (user_id, message, notification_type, status, priority)
        VALUES ($1, $2, 'EMAIL'::notification_type, 'PENDING'::notification_status, 1)
        RETURNING id
        "#,
    )
    .bind(sub_user_id)
    .bind(&sub_email_summary)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    let sub_subject = format!(
        "[{school_name}] Class Substitution Assigned: Period {period_number} ({class_name}-{section_name})"
    );
    let sub_html = generate_substitution_email_html(
        &school_name,
        "Class Substitution Assigned",
        &sub_teacher_name,
        &format!("{date}"),
        period_number,
        start_short,
        end_short,
        &class_name,
        &section_name,
        &subject_name,
        &subject_code,
        &orig_teacher_name,
        display_reason,
        true,
    );

    // B. Original Teacher Email
    let orig_email_summary = format!(
        "Substitution Arranged: Period {period_number} ({class_name}-{section_name}) covered by {sub_teacher_name}"
    );
    let orig_notif_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        INSERT INTO notifications (user_id, message, notification_type, status, priority)
        VALUES ($1, $2, 'EMAIL'::notification_type, 'PENDING'::notification_status, 1)
        RETURNING id
        "#,
    )
    .bind(orig_user_id)
    .bind(&orig_email_summary)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    let orig_subject = format!(
        "[{school_name}] Substitution Confirmed: Period {period_number} ({class_name}-{section_name}) covered by {sub_teacher_name}"
    );
    let orig_html = generate_substitution_email_html(
        &school_name,
        "Substitution Confirmed",
        &orig_teacher_name,
        &format!("{date}"),
        period_number,
        start_short,
        end_short,
        &class_name,
        &section_name,
        &subject_name,
        &subject_code,
        &sub_teacher_name,
        display_reason,
        false,
    );

    // Spawn non-blocking background dispatch for real-time delivery via SMTP
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        // Dispatch to Substitute
        if let Some(target_email) = sub_email {
            if !target_email.trim().is_empty() {
                match crate::utils::email::send_email(&target_email, &sub_subject, &sub_html).await
                {
                    Ok(_) => {
                        info!(email = %target_email, "Substitution alert email delivered to substitute");
                        if let Some(nid) = sub_notif_id {
                            let _ = sqlx::query(
                                "UPDATE notifications SET status = 'SENT'::notification_status, updated_at = NOW() WHERE id = $1"
                            )
                            .bind(nid)
                            .execute(&pool_clone)
                            .await;
                        }
                    }
                    Err(e) => {
                        warn!(email = %target_email, error = %e, "Failed to deliver substitution email to substitute");
                        if let Some(nid) = sub_notif_id {
                            let _ = sqlx::query(
                                "UPDATE notifications SET status = 'FAILED'::notification_status, retry_count = retry_count + 1, updated_at = NOW() WHERE id = $1"
                            )
                            .bind(nid)
                            .execute(&pool_clone)
                            .await;
                        }
                    }
                }
            }
        }

        // Dispatch to Original
        if let Some(target_email) = orig_email {
            if !target_email.trim().is_empty() {
                match crate::utils::email::send_email(&target_email, &orig_subject, &orig_html)
                    .await
                {
                    Ok(_) => {
                        info!(email = %target_email, "Substitution confirmation email delivered to original faculty");
                        if let Some(nid) = orig_notif_id {
                            let _ = sqlx::query(
                                "UPDATE notifications SET status = 'SENT'::notification_status, updated_at = NOW() WHERE id = $1"
                            )
                            .bind(nid)
                            .execute(&pool_clone)
                            .await;
                        }
                    }
                    Err(e) => {
                        warn!(email = %target_email, error = %e, "Failed to deliver substitution email to original faculty");
                        if let Some(nid) = orig_notif_id {
                            let _ = sqlx::query(
                                "UPDATE notifications SET status = 'FAILED'::notification_status, retry_count = retry_count + 1, updated_at = NOW() WHERE id = $1"
                            )
                            .bind(nid)
                            .execute(&pool_clone)
                            .await;
                        }
                    }
                }
            }
        }
    });

    info!(
        sub_id = %sub_id,
        substitute = %sub_teacher_name,
        original = %orig_teacher_name,
        "Email & WhatsApp substitution queues dispatched successfully"
    );
}

/// Helper to render an HTML email template for substitutions
fn generate_substitution_email_html(
    school_name: &str,
    heading: &str,
    recipient_name: &str,
    date: &str,
    period_number: i32,
    start_time: &str,
    end_time: &str,
    class_name: &str,
    section_name: &str,
    subject_name: &str,
    subject_code: &str,
    counterpart_teacher_name: &str,
    reason: &str,
    is_substitute: bool,
) -> String {
    let counterpart_label = if is_substitute {
        "Original Faculty"
    } else {
        "Assigned Substitute"
    };

    let action_note = if is_substitute {
        "You have been assigned to cover this period. Please report to the class on time."
    } else {
        "Your class coverage has been confirmed. No further action is required from you."
    };

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>{heading}</title>
</head>
<body style="margin: 0; padding: 24px; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; background-color: #f8fafc; color: #1e293b;">
  <div style="max-width: 580px; margin: 0 auto; background-color: #ffffff; border: 1px solid #e2e8f0; border-radius: 16px; overflow: hidden; box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.05);">
    <div style="background: linear-gradient(135deg, #4f46e5 0%, #3b82f6 100%); padding: 24px 28px; color: #ffffff;">
      <div style="font-size: 11px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.08em; opacity: 0.9; margin-bottom: 4px;">{school_name}</div>
      <h1 style="margin: 0; font-size: 20px; font-weight: 800;">{heading}</h1>
    </div>
    
    <div style="padding: 28px;">
      <p style="font-size: 14px; margin-top: 0; margin-bottom: 20px; color: #334155;">
        Dear <strong>{recipient_name}</strong>,<br>
        {action_note}
      </p>

      <table style="width: 100%; border-collapse: separate; border-spacing: 0; border: 1px solid #e2e8f0; border-radius: 12px; overflow: hidden; font-size: 13px; margin-bottom: 24px;">
        <tr style="background-color: #f8fafc;">
          <td style="padding: 10px 14px; font-weight: 700; color: #64748b; border-bottom: 1px solid #e2e8f0; width: 35%;">Date</td>
          <td style="padding: 10px 14px; font-weight: 700; color: #0f172a; border-bottom: 1px solid #e2e8f0;">{date}</td>
        </tr>
        <tr>
          <td style="padding: 10px 14px; font-weight: 700; color: #64748b; border-bottom: 1px solid #e2e8f0;">Period</td>
          <td style="padding: 10px 14px; font-weight: 700; color: #4f46e5; border-bottom: 1px solid #e2e8f0;">Period {period_number} <span style="font-size: 11px; font-weight: 500; color: #64748b;">({start_time} - {end_time})</span></td>
        </tr>
        <tr style="background-color: #f8fafc;">
          <td style="padding: 10px 14px; font-weight: 700; color: #64748b; border-bottom: 1px solid #e2e8f0;">Class & Section</td>
          <td style="padding: 10px 14px; font-weight: 600; color: #0f172a; border-bottom: 1px solid #e2e8f0;">{class_name} - Section {section_name}</td>
        </tr>
        <tr>
          <td style="padding: 10px 14px; font-weight: 700; color: #64748b; border-bottom: 1px solid #e2e8f0;">Subject</td>
          <td style="padding: 10px 14px; font-weight: 600; color: #0f172a; border-bottom: 1px solid #e2e8f0;">{subject_name} <span style="font-size: 11px; color: #64748b;">({subject_code})</span></td>
        </tr>
        <tr style="background-color: #f8fafc;">
          <td style="padding: 10px 14px; font-weight: 700; color: #64748b; border-bottom: 1px solid #e2e8f0;">{counterpart_label}</td>
          <td style="padding: 10px 14px; font-weight: 600; color: #0f172a; border-bottom: 1px solid #e2e8f0;">{counterpart_teacher_name}</td>
        </tr>
        <tr>
          <td style="padding: 10px 14px; font-weight: 700; color: #64748b;">Reason / Notes</td>
          <td style="padding: 10px 14px; color: #475569;">{reason}</td>
        </tr>
      </table>

      <div style="background-color: #f0fdf4; border: 1px solid #bbf7d0; border-radius: 8px; padding: 12px 16px; font-size: 12px; color: #166534; margin-bottom: 24px;">
        ✓ This schedule has been updated automatically in the master timetable and daily projection matrix.
      </div>

      <div style="border-top: 1px solid #f1f5f9; padding-top: 16px; font-size: 11px; color: #94a3b8; text-align: center;">
        Automated notification dispatched by Eravaya ERP Academic Desk. Please do not reply directly to this email.
      </div>
    </div>
  </div>
</body>
</html>"#
    )
}
