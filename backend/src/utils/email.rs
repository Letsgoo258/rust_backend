use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::Tls;
use lettre::{Message, SmtpTransport, Transport};
use std::env;

pub async fn send_email(
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
    let smtp_from = match env::var("SMTP_FROM") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => return Err("SMTP_FROM environment variable is not set".into()),
    };

    let email = Message::builder()
        .from(smtp_from.parse()?)
        .to(to.parse()?)
        .subject(subject)
        .header(lettre::message::header::ContentType::TEXT_HTML)
        .body(html_body.to_string())?;

    let creds = Credentials::new(smtp_user, smtp_pass);

    // For port 465, we must use Tls::Wrapper
    // For port 587, we would use Tls::Required
    let tls = if smtp_port == 465 {
        Tls::Wrapper(
            lettre::transport::smtp::client::TlsParameters::builder(smtp_host.clone()).build()?,
        )
    } else {
        Tls::Required(
            lettre::transport::smtp::client::TlsParameters::builder(smtp_host.clone()).build()?,
        )
    };

    let mailer = SmtpTransport::builder_dangerous(&smtp_host)
        .port(smtp_port)
        .credentials(creds)
        .tls(tls)
        .build();

    // lettre SmtpTransport is synchronous, we must spawn it in a blocking thread
    tokio::task::spawn_blocking(move || mailer.send(&email)).await??;

    Ok(())
}
