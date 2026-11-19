use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::env;

pub async fn send_email(
    to: &str,
    subject: &str,
    html_body: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    send_email_with_sender(None, to, subject, html_body).await
}

pub async fn send_email_with_sender(
    from_name: Option<&str>,
    to: &str,
    subject: &str,
    html_body: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let smtp_host = env::var("SMTP_HOST").unwrap_or_else(|_| "smtp.gmail.com".to_string());
    let smtp_port = env::var("SMTP_PORT")
        .unwrap_or_else(|_| "465".to_string())
        .parse::<u16>()
        .unwrap_or(465);
    let smtp_user = match env::var("SMTP_USER") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => return Err("SMTP_USER environment variable is not set".into()),
    };
    let smtp_pass = match env::var("SMTP_PASS") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => return Err("SMTP_PASS environment variable is not set".into()),
    };
    let raw_from = match env::var("SMTP_FROM") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => return Err("SMTP_FROM environment variable is not set".into()),
    };
    let clean_from = raw_from.trim().trim_matches('"').trim_matches('\'');
    let from_header_value = match from_name {
        Some(name) if !name.trim().is_empty() => {
            let email_addr = if let Some(start) = clean_from.find('<') {
                if let Some(end) = clean_from.find('>') {
                    &clean_from[start + 1..end]
                } else {
                    clean_from
                }
            } else {
                clean_from
            };
            format!("{} <{}>", name.trim(), email_addr.trim())
        }
        _ => clean_from.to_string(),
    };

    let email = Message::builder()
        .from(from_header_value.parse()?)
        .to(to.parse()?)
        .subject(subject)
        .header(lettre::message::header::ContentType::TEXT_HTML)
        .body(html_body.to_string())?;

    let creds = Credentials::new(smtp_user.clone(), smtp_pass);

    let mailer: AsyncSmtpTransport<Tokio1Executor> = if smtp_port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_host)?
            .port(smtp_port)
            .credentials(creds)
            .build()
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp_host)?
            .port(smtp_port)
            .credentials(creds)
            .build()
    };

    println!("============================================================");
    println!("📧 [EMAIL DISPATCHED TO REAL SERVICE]");
    println!("To: {}", to);
    println!("Subject: {}", subject);
    println!(
        "SMTP Relay: {}:{} (User: {})",
        smtp_host, smtp_port, smtp_user
    );
    println!("============================================================");

    match mailer.send(email).await {
        Ok(_) => {
            tracing::info!(target: "email", to = %to, subject = %subject, "Email delivered successfully via SMTP");
            Ok(())
        }
        Err(e) => {
            eprintln!(
                "❌ [EMAIL DELIVERY ERROR] Failed to deliver to {}: {:?}",
                to, e
            );
            tracing::error!(target: "email", to = %to, subject = %subject, error = ?e, "Failed to deliver email via SMTP");
            Err(Box::new(e))
        }
    }
}
