use crate::modules::whatsapp::service::WhatsAppUserContext;
use sqlx::PgPool;

/// Detects if the incoming message is an exams request.
pub fn is_exams(text: &str) -> bool {
    let clean = text.trim().to_lowercase();
    clean.contains("exam")
        || clean.contains("exams")
        || clean.contains("marks")
        || clean.contains("result")
        || clean.contains("results")
        || clean == "3"
        || clean.contains("grade")
        || clean.contains("assessment")
        || clean.contains("test")
}

/// Handles exam queries: retrieves active academic year and terms.
pub async fn handle_exams_query(pool: &PgPool, context: &WhatsAppUserContext) -> String {
    let ay_row = sqlx::query_as::<_, (String, time::Date, time::Date)>(
        r#"SELECT name, start_date, end_date
           FROM academic_years
           WHERE school_id = $1 AND status = 'ACTIVE'
           ORDER BY start_date DESC
           LIMIT 1"#,
    )
    .bind(context.school_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let ay_text = if let Some((ay_name, start, end)) = ay_row {
        format!("*{}* ({} to {})", ay_name, start, end)
    } else {
        "2026-2027 Academic Session".to_string()
    };

    format!(
        "📝 *Examinations & Academic Calendar*\n\n\
        📅 *Current Academic Year*: {}\n\
        📌 *Status*: Regular classes and continuous assessments ongoing.\n\n\
        _Official examination dates and admit cards will be notified here and published on your dashboard._",
        ay_text
    )
}
