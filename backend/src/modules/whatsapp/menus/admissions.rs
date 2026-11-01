use chrono::{Datelike, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Detects if the incoming message is an admissions inquiry.
pub fn is_admission_intent(text: &str) -> bool {
    let clean = text.trim().to_lowercase();
    clean.contains("admission")
        || clean.contains("admissions")
        || clean.contains("apply")
        || clean.contains("enrol")
        || clean.contains("enroll")
        || clean.contains("intake")
        || clean.contains("new student")
        || clean.contains("register child")
}

/// Handles public admission inquiries dynamically:
/// - Resolves the primary active school
/// - Fetches `admission_settings` from `school_settings` table
/// - Generates the direct online admissions application URL
pub async fn handle_admission_inquiry(pool: &PgPool) -> Vec<String> {
    let app_url = std::env::var("APP_URL")
        .unwrap_or_else(|_| "https://my-frontend-kappa-five.vercel.app".to_string());
    let clean_url = app_url.trim_end_matches('/');
    let apply_link = format!("{clean_url}/admissions/apply");

    // 1. Resolve primary active school
    let school_row = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM schools WHERE status = 'ACTIVE' ORDER BY created_at ASC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let (school_id, school_name) = match school_row {
        Some((id, name)) => (id, name),
        None => (Uuid::nil(), "ERAVAYA School".to_string()),
    };

    // 2. Fetch admission settings from school_settings
    let setting_val = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM school_settings WHERE school_id = $1 AND key = 'admission_settings'",
    )
    .bind(school_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let current_year = Utc::now().date_naive().year();
    let default_ay = format!("{}-{}", current_year, current_year + 1);

    let (is_open, open_date, close_date, ay_name) = if let Some(val) = setting_val {
        let open_d = val["open_date"]
            .as_str()
            .unwrap_or("2026-01-01")
            .to_string();
        let close_d = val["close_date"]
            .as_str()
            .unwrap_or("2026-12-31")
            .to_string();
        let ay = val["academic_year_name"]
            .as_str()
            .unwrap_or(&default_ay)
            .to_string();

        let today = Utc::now().date_naive();
        let in_date = match (
            chrono::NaiveDate::parse_from_str(&open_d, "%Y-%m-%d"),
            chrono::NaiveDate::parse_from_str(&close_d, "%Y-%m-%d"),
        ) {
            (Ok(o), Ok(c)) => today >= o && today <= c,
            _ => true,
        };

        (in_date, open_d, close_d, ay)
    } else {
        (
            true,
            format!("{current_year}-01-01"),
            format!("{current_year}-12-31"),
            default_ay,
        )
    };

    if is_open {
        vec![
            format!(
                "🎓 *Welcome to ERAVAYA Admissions!*\n\n\
                Admissions for *{school_name}* are officially *OPEN* for the Academic Year *{ay_name}*.\n\n\
                We warmly welcome prospective students and parents. Please access our online admission portal below:"
            ),
            apply_link,
            "📋 *How to Complete Your Application*:\n\
            1️⃣ Verify your *Mobile Phone* via WhatsApp OTP.\n\
            2️⃣ Verify your *Email Address* via Email OTP.\n\
            3️⃣ Enter Applicant (Student) & Guardian details.\n\
            4️⃣ Fill in school-specific questions & documents.\n\
            5️⃣ Complete the security captcha & submit.\n\n\
            Your unique *Application Number* will be generated instantly upon submission!".to_string(),
        ]
    } else {
        vec![format!(
            "ℹ️ *ERAVAYA Admissions Notice*\n\n\
            Admissions for *{school_name}* are currently *CLOSED*.\n\n\
            📅 *Intake Window*:\n\
            • Opens On: *{open_date}*\n\
            • Closes On: *{close_date}*\n\
            • Academic Year: *{ay_name}*\n\n\
            For special inquiries or upcoming admissions, please contact the school administrative desk."
        )]
    }
}
