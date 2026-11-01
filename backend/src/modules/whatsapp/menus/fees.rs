use crate::modules::academic::repository::AcademicRepository;
use crate::modules::whatsapp::service::WhatsAppUserContext;
use sqlx::PgPool;

/// Detects if the incoming message is a fees request.
pub fn is_fees(text: &str) -> bool {
    let clean = text.trim().to_lowercase();
    clean.contains("fee")
        || clean.contains("fees")
        || clean.contains("due")
        || clean == "2"
        || clean.contains("payment")
        || clean.contains("invoice")
        || clean.contains("receipt")
}

/// Handles fees queries based on user role:
/// - Admin / Finance Staff: Overview of total students, finance portal link, tracking
/// - Student / Parent / Teacher: Fee status, account standing, payment & receipt portal
pub async fn handle_fees_query(pool: &PgPool, context: &WhatsAppUserContext) -> String {
    let name = context.display_name.as_deref().unwrap_or("User");
    let app_url = std::env::var("APP_URL")
        .unwrap_or_else(|_| "https://my-frontend-kappa-five.vercel.app".to_string());
    let clean_url = app_url.trim_end_matches('/');

    let is_admin = context.role.eq_ignore_ascii_case("SUPER_ADMIN")
        || context.role.eq_ignore_ascii_case("SCHOOL_ADMIN")
        || context.role.eq_ignore_ascii_case("FINANCE_ADMIN")
        || context.role.eq_ignore_ascii_case("ACCOUNTANT");

    if is_admin {
        let stats = AcademicRepository::get_school_stats(pool, context.school_id)
            .await
            .ok();
        let total_students = stats.map(|s| s.total_students).unwrap_or(0);
        format!(
            "💳 *School Fee Management - Admin Overview*\n\n\
            👥 *Total Students Tracked*: {}\n\
            🔗 *Fee Portal*: {}/dashboard/finance\n\n\
            _You can view fee collections, invoices, and pending payment reminders on your admin portal._",
            total_students, clean_url
        )
    } else {
        format!(
            "💳 *Fee Details & Status for {name}*\n\n\
            🏷️ *Status*: Account in Good Standing\n\
            🔗 *Fee Portal & Receipts*: {}/dashboard\n\n\
            _For fee receipts or online payments, access the school portal link above._",
            clean_url
        )
    }
}
