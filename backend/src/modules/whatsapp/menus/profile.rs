use crate::modules::whatsapp::repository::WhatsAppUserSessionRow;
use crate::modules::whatsapp::service::WhatsAppUserContext;
use sqlx::PgPool;

/// Detects if the incoming message is a profile request.
pub fn is_profile(text: &str) -> bool {
    let clean = text.trim().to_lowercase();
    clean.contains("profile")
        || clean.contains("account")
        || clean.contains("who am i")
        || clean == "5"
        || clean.contains("my details")
        || clean.contains("my info")
}

/// Handles user profile and session queries.
pub async fn handle_profile_query(
    pool: &PgPool,
    context: &WhatsAppUserContext,
    session: &WhatsAppUserSessionRow,
) -> String {
    let name = context.display_name.as_deref().unwrap_or("User");
    let expires_formatted = session.expires_at.format("%d %b %Y, %H:%M UTC").to_string();
    let cap_count = context.capabilities.len();

    let school_name: String =
        sqlx::query_scalar!("SELECT name FROM schools WHERE id = $1", context.school_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "ERAVAYA School".to_string());

    format!(
        "👤 *Account Profile & Capabilities*\n\n\
        🏷️ *Name*: {name}\n\
        📱 *Phone*: {}\n\
        🛡️ *Role*: *{}*\n\
        🏫 *School*: {school_name}\n\
        🔑 *Session ID*: `{}`\n\
        ⏳ *Session Valid Until*: {expires_formatted}\n\
        📋 *Authorized Capabilities*: {cap_count} active\n\n\
        _Type *logout* anytime to revoke this session._",
        context.phone, context.role, session.id
    )
}
