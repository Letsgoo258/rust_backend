use lettre::{Message, SmtpTransport, Transport};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::Tls;
use std::env;

pub async fn send_email(to: &str, subject: &str, html_body: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let smtp_host = env::var("SMTP_HOST").unwrap_or_else(|_| "smtp.gmail.com".to_string());
    let smtp_port = env::var("SMTP_PORT").unwrap_or_else(|_| "465".to_string()).parse::<u16>().unwrap_or(465);
    let smtp_user = env::var("SMTP_USER").expect("SMTP_USER must be set");
    let smtp_pass = env::var("SMTP_PASS").expect("SMTP_PASS must be set");
    let smtp_from = env::var("SMTP_FROM").expect("SMTP_FROM must be set");

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
        Tls::Wrapper(lettre::transport::smtp::client::TlsParameters::builder(smtp_host.clone()).build()?)
    } else {
        Tls::Required(lettre::transport::smtp::client::TlsParameters::builder(smtp_host.clone()).build()?)
    };

    let mailer = SmtpTransport::builder_dangerous(&smtp_host)
        .port(smtp_port)
        .credentials(creds)
        .tls(tls)
        .build();

    // lettre SmtpTransport is synchronous, we must spawn it in a blocking thread
    tokio::task::spawn_blocking(move || {
        mailer.send(&email)
    }).await??;

    Ok(())
}
