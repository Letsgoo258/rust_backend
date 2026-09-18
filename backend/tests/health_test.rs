use eravaya::config::AppConfig;
use eravaya::db;
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};
use std::env;

async fn send_alert_email(error_message: &str) {
    let smtp_host = env::var("SMTP_HOST").unwrap_or_else(|_| "smtp.gmail.com".to_string());
    let smtp_port = env::var("SMTP_PORT").unwrap_or_else(|_| "465".to_string());
    let smtp_user = env::var("SMTP_USER").expect("SMTP_USER must be set for alerts");
    let smtp_pass = env::var("SMTP_PASS").expect("SMTP_PASS must be set for alerts");
    let smtp_from = env::var("SMTP_FROM").expect("SMTP_FROM must be set for alerts");

    let email = Message::builder()
        .from(smtp_from.parse().unwrap())
        .to(smtp_user.parse().unwrap()) // sending alert to the admin themselves
        .subject("🚨 ERAVAYA ERP Alert: Database Connection Failed")
        .header(ContentType::TEXT_PLAIN)
        .body(format!(
            "The database connection test failed in the CI/CD pipeline.\n\nError details:\n{}",
            error_message
        ))
        .unwrap();

    let creds = Credentials::new(smtp_user, smtp_pass);

    let mailer: AsyncSmtpTransport<Tokio1Executor> = if smtp_port == "465" {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_host)
            .unwrap()
            .credentials(creds)
            .build()
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp_host)
            .unwrap()
            .credentials(creds)
            .build()
    };

    match mailer.send(email).await {
        Ok(_) => println!("Alert email sent successfully!"),
        Err(e) => eprintln!("Could not send alert email: {:?}", e),
    }
}

#[tokio::test]
async fn test_live_database_connection() {
    dotenvy::dotenv().ok();

    let config = AppConfig::from_env();

    println!(
        "Attempting to connect to live DB at {}",
        config.database_url
    );

    let pool_result = db::init_pool(&config).await;

    match pool_result {
        Ok(pool) => {
            let ping_result = sqlx::query("SELECT 1").execute(&pool).await;

            match ping_result {
                Ok(_) => {
                    println!("Successfully connected and pinged the live database!");
                    assert!(true);
                }
                Err(e) => {
                    let err_msg = format!("Failed to execute ping query: {}", e);
                    send_alert_email(&err_msg).await;
                    panic!("{}", err_msg);
                }
            }
        }
        Err(e) => {
            let err_msg = format!("Failed to initialize database pool: {}", e);
            send_alert_email(&err_msg).await;
            panic!("{}", err_msg);
        }
    }
}
