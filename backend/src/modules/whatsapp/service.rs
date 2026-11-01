use crate::error::AppError;
use crate::modules::whatsapp::menus;
#[allow(unused_imports)]
pub use crate::modules::whatsapp::menus::ai::AiResponsePayload;
use crate::modules::whatsapp::repository::WhatsAppRepository;
use crate::utils::capabilities::CapabilityController;
use crate::utils::whatsapp::send_whatsapp_message;
use chrono::Utc;
use sqlx::PgPool;
use std::collections::HashSet;
use tracing::{info, warn};
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct WhatsAppUserContext {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub school_id: Uuid,
    pub role: String,
    pub capabilities: HashSet<String>,
    pub phone: String,
    pub display_name: Option<String>,
}

impl WhatsAppUserContext {
    pub fn is_school_admin(&self) -> bool {
        self.role.eq_ignore_ascii_case("SUPER_ADMIN")
            || self.role.eq_ignore_ascii_case("SCHOOL_ADMIN")
            || self.role.eq_ignore_ascii_case("HEADMASTER")
            || self.role.eq_ignore_ascii_case("PRINCIPAL")
    }

    pub fn has_capability(&self, key: &str) -> bool {
        let key_norm = key.replace('.', ":");
        if key_norm == "attendance:record" || key_norm == "attendance:mark" {
            // School Admins are strictly prohibited from initial attendance recording
            if self.is_school_admin() {
                return false;
            }
        }
        if self.is_school_admin() {
            return true;
        }
        self.capabilities.contains(key)
            || self.capabilities.contains(&key_norm)
            || (key_norm == "attendance:mark" && self.capabilities.contains("attendance:record"))
            || (key_norm == "attendance:record" && self.capabilities.contains("attendance:mark"))
            || (key_norm == "attendance:edit"
                && (self.capabilities.contains("attendance:edit_class")
                    || self.capabilities.contains("attendance:edit_subject")))
            || (key_norm == "attendance:edit_class"
                && self.capabilities.contains("attendance:edit"))
            || (key_norm == "attendance:edit_subject"
                && self.capabilities.contains("attendance:edit"))
    }

    pub fn is_teacher(&self) -> bool {
        self.role.eq_ignore_ascii_case("TEACHER")
            || self.role.eq_ignore_ascii_case("CLASS_TEACHER")
            || self.role.eq_ignore_ascii_case("FACULTY")
            || self.role.eq_ignore_ascii_case("STAFF")
    }

    pub fn is_student_or_parent(&self) -> bool {
        self.role.eq_ignore_ascii_case("STUDENT")
            || self.role.eq_ignore_ascii_case("PARENT")
            || self.role.eq_ignore_ascii_case("GUARDIAN")
    }
}

pub struct WhatsAppMessageService;

impl WhatsAppMessageService {
    /// Detects if the user's message is a greeting.
    pub fn is_greeting(text: &str) -> bool {
        let clean: String = text
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect();
        let words: Vec<&str> = clean.split_whitespace().collect();

        if words.is_empty() {
            return false;
        }

        // Exact match or single-word greeting
        if words.len() == 1 {
            return matches!(
                words[0],
                "hi" | "hello" | "hey" | "heya" | "namaste" | "namaskar" | "hola" | "start"
            );
        }

        // Multi-word greetings
        let joined = words.join(" ");
        joined == "good morning"
            || joined == "good afternoon"
            || joined == "good evening"
            || joined.starts_with("hi ")
            || joined.starts_with("hello ")
            || joined.starts_with("hey ")
    }

    /// Detects if the user is asking for the help menu.
    pub fn is_help_request(text: &str) -> bool {
        let clean: String = text
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect();
        let trimmed = clean.trim();

        matches!(
            trimmed,
            "help" | "menu" | "options" | "help menu" | "commands" | "info"
        )
    }

    /// Formats the official ERAVAYA ERP welcome greeting.
    #[allow(dead_code)]
    pub fn get_welcome_greeting() -> &'static str {
        "👋 Hi! Welcome to ERAVAYA ERP.\n\nType *help* for the help menu or ask your query (e.g. attendance, fees, timetable, exams) to get assistance."
    }

    /// Formats the interactive help menu.
    #[allow(dead_code)]
    pub fn get_help_menu() -> &'static str {
        "📋 *ERAVAYA ERP Assistance Menu*\n\n1️⃣ *Attendance* - Check student & staff attendance status\n2️⃣ *Fees* - View pending fee dues and receipt links\n3️⃣ *Exams* - View exam dates, schedules, and report cards\n4️⃣ *Timetable* - View class and subject timetable\n5️⃣ *Contact* - School office phone and support desk\n\n💡 You can also ask directly in plain text:\n_e.g., \"What is the fee payment deadline?\"_"
    }

    pub fn is_logout(text: &str) -> bool {
        let clean = text.trim().to_lowercase();
        clean == "logout"
            || clean == "signout"
            || clean == "exit"
            || clean == "log out"
            || clean == "sign out"
            || clean == "6"
    }

    pub fn format_unregistered_invitation(_token: &str, _phone: &str) -> (String, String) {
        let app_url = std::env::var("APP_URL")
            .unwrap_or_else(|_| "https://my-frontend-kappa-five.vercel.app".to_string());
        let clean_url = app_url.trim_end_matches('/');
        let login_link = format!("{clean_url}/login?redirect=/auth/whatsapp-link");

        let intro = "🔒 *ERAVAYA ERP Authentication Required*\n\n\
            Welcome to ERAVAYA ERP! To protect student data and privacy, WhatsApp assistance is only available for active, logged-in users.\n\n\
            Please log in to your ERAVAYA account to connect WhatsApp:".to_string();

        (intro, login_link)
    }

    pub fn format_session_expired(_token: &str, _phone: &str) -> (String, String) {
        let app_url = std::env::var("APP_URL")
            .unwrap_or_else(|_| "https://my-frontend-kappa-five.vercel.app".to_string());
        let clean_url = app_url.trim_end_matches('/');
        let login_link = format!("{clean_url}/login?redirect=/auth/whatsapp-link");

        let intro = "⏰ *Session Expired or Inactive*\n\n\
            Your ERAVAYA WhatsApp session has expired for your security.\n\n\
            Please log in to your account to re-activate your session:"
            .to_string();

        (intro, login_link)
    }

    pub fn format_session_dropped(reason: &str) -> (String, String) {
        let app_url = std::env::var("APP_URL")
            .unwrap_or_else(|_| "https://my-frontend-kappa-five.vercel.app".to_string());
        let clean_url = app_url.trim_end_matches('/');
        let login_link = format!("{clean_url}/login?redirect=/auth/whatsapp-link");

        let intro = format!(
            "⚠️ *Session Dropped ({reason})*\n\n\
            For your security and anti-spoofing protection, this WhatsApp session has been terminated.\n\n\
            Please log in to ERAVAYA ERP to verify your credentials and reconnect:"
        );

        (intro, login_link)
    }

    /// Formats the authenticated menu customized by role:
    /// - Student / Parent: "1️⃣ Attendance - View your attendance record & standing"
    /// - Teacher: "1️⃣ Attendance - Record & mark daily class attendance"
    /// - Admin: "1️⃣ Attendance - View school-wide attendance overview"
    pub fn format_authenticated_menu(context: &WhatsAppUserContext) -> String {
        let name = context.display_name.as_deref().unwrap_or("User");
        let attendance_item = if context.is_teacher() {
            "1️⃣ *Attendance* - Record & mark daily class attendance"
        } else if context.is_student_or_parent() {
            "1️⃣ *Attendance* - View your attendance record & standing"
        } else {
            "1️⃣ *Attendance* - View school-wide attendance overview"
        };

        format!(
            "📋 *ERAVAYA ERP Menu*\n\
            👤 *Account*: {name} (*{}*)\n\n\
            {attendance_item}\n\
            2️⃣ *Fees* - View pending dues, fee breakdown & receipts\n\
            3️⃣ *Exams* - Check upcoming exam schedules & results\n\
            4️⃣ *Timetable* - View today's classes and schedule\n\
            5️⃣ *Profile* - View account permissions & session details\n\
            6️⃣ *Logout* - Sign out of WhatsApp\n\n\
            💡 You can also ask directly:\n\
            _e.g., \"What are my fee dues for this term?\"_",
            context.role
        )
    }

    /// Evaluates user intent and returns a list of individual messages to deliver.
    /// Unregistered and expired session replies return two separate messages:
    /// 1. Instructive explanation
    /// 2. Standalone URL so the user can easily tap or copy-paste it directly
    pub async fn generate_reply_messages(
        pool: &PgPool,
        sender_jid: &str,
        sender_alt: Option<&str>,
        incoming_text: &str,
    ) -> Vec<String> {
        let trimmed = incoming_text.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        // Determine phone: prefer sender_alt if it contains a PN, or digits from sender_jid
        let phone: String = if let Some(alt) = sender_alt {
            let alt_digits: String = alt.chars().filter(|c| c.is_ascii_digit()).collect();
            if !alt_digits.is_empty() && !alt.contains("@lid") {
                alt_digits
            } else {
                sender_jid
                    .split('@')
                    .next()
                    .unwrap_or("")
                    .chars()
                    .filter(|c| c.is_ascii_digit())
                    .collect()
            }
        } else {
            sender_jid
                .split('@')
                .next()
                .unwrap_or("")
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect()
        };

        // 0. Check for admission inquiries (publicly accessible to prospective parents/students without prior ERP login)
        if menus::admissions::is_admission_intent(trimmed) {
            info!(sender = %sender_jid, "Handling admission inquiry from WhatsApp user");
            return menus::admissions::handle_admission_inquiry(pool).await;
        }

        // 1. Check if the incoming message is a link token from ERAVAYA ERP (e.g. "ERA-123456")
        let upper_token = trimmed.to_uppercase();
        if upper_token.starts_with("ERA-")
            || (upper_token.len() >= 6 && upper_token.len() <= 20 && !upper_token.contains(' '))
        {
            if let Ok(Some(link_token)) =
                WhatsAppRepository::get_link_token(pool, &upper_token).await
            {
                let app_url = std::env::var("APP_URL")
                    .unwrap_or_else(|_| "https://my-frontend-kappa-five.vercel.app".to_string());
                let clean_url = app_url.trim_end_matches('/');
                let login_url = format!("{clean_url}/login?redirect=/auth/whatsapp-link");

                if link_token.consumed {
                    return vec![
                        "❌ *Link Code Already Used*\n\nThis linking code has already been consumed. Please log in to ERAVAYA ERP to generate a fresh link token:".to_string(),
                        login_url,
                    ];
                }

                if link_token.expires_at <= Utc::now() {
                    return vec![
                        "⏰ *Link Code Expired*\n\nThis linking code has expired (15-minute validity). Please log in to ERAVAYA ERP to generate a fresh code:".to_string(),
                        login_url,
                    ];
                }

                // Consume link token
                let _ = WhatsAppRepository::consume_link_token(pool, &link_token.token, sender_jid)
                    .await;

                // Resolve user capabilities
                let caps = CapabilityController::resolve_user_capabilities(
                    pool,
                    link_token.school_id,
                    Some(link_token.user_id),
                    &[link_token.role_code.clone()],
                )
                .await;

                let user_row = sqlx::query_as::<_, (Option<String>, String)>(
                    "SELECT display_name, username FROM users WHERE id = $1",
                )
                .bind(link_token.user_id)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();

                let display_name = user_row
                    .as_ref()
                    .and_then(|u| u.0.clone())
                    .or_else(|| user_row.as_ref().map(|u| u.1.clone()))
                    .unwrap_or_else(|| "User".to_string());

                let _ = WhatsAppRepository::create_or_renew_session_with_meta(
                    pool,
                    link_token.user_id,
                    link_token.school_id,
                    sender_jid,
                    &phone,
                    &link_token.role_code,
                    &caps,
                    7, // 7 days sliding session
                    Some(&link_token.ip_address),
                    link_token.user_agent.as_deref(),
                    None,
                    Some(&link_token.token),
                )
                .await;

                info!(
                    sender = %sender_jid,
                    user_id = %link_token.user_id,
                    role = %link_token.role_code,
                    ip = %link_token.ip_address,
                    "WhatsApp session successfully authenticated and linked with ERAVAYA ERP"
                );

                return vec![format!(
                    "🎉 *WhatsApp Connected Successfully!*\n\n\
                    Welcome, *{display_name}* (*{}*)!\n\
                    Your WhatsApp session is now active and securely linked to your ERAVAYA ERP account.\n\n\
                    🔒 *Security Details*:\n\
                    • Linked IP: `{}`\n\
                    • Session: Active (7-day rolling expiry)\n\n\
                    Type *help* or ask any query (e.g. attendance, fees, exams, timetable) to get started!",
                    link_token.role_code,
                    link_token.ip_address
                )];
            }
        }

        // 2. Check for active WhatsApp user session
        let session_opt = WhatsAppRepository::get_session_by_jid(pool, sender_jid)
            .await
            .unwrap_or(None);

        match session_opt {
            None => {
                // User is not logged in / not linked. Send 2 separate messages (intro + standalone login link).
                info!(sender = %sender_jid, "Unauthenticated user. Prompting ERP login");
                let (msg1, msg2) = Self::format_unregistered_invitation("", &phone);
                vec![msg1, msg2]
            }
            Some(session) if session.expires_at <= Utc::now() => {
                // Session is expired. Clear session and prompt to log in.
                let _ = sqlx::query("DELETE FROM whatsapp_user_sessions WHERE id = $1")
                    .bind(session.id)
                    .execute(pool)
                    .await;
                info!(sender = %sender_jid, "Expired session. Prompting ERP login");
                let (msg1, msg2) = Self::format_session_expired("", &phone);
                vec![msg1, msg2]
            }
            Some(session) if session.is_revoked => {
                // Suspicious / revoked session. Clear session and notify with specific security reason.
                let _ = sqlx::query("DELETE FROM whatsapp_user_sessions WHERE id = $1")
                    .bind(session.id)
                    .execute(pool)
                    .await;
                info!(sender = %sender_jid, "Revoked session. Prompting ERP login");
                let (msg1, msg2) = Self::format_session_dropped(
                    session
                        .suspicious_reason
                        .as_deref()
                        .unwrap_or("Security policy"),
                );
                vec![msg1, msg2]
            }
            Some(session) => {
                // Active session! Slide sliding expiration forward (+7 days) and update activity timestamp
                let _ = WhatsAppRepository::slide_session_expiry(pool, session.id, 7).await;
                let _ = WhatsAppRepository::record_session_activity(pool, session.id).await;

                // Load session capabilities from whatsapp_capabilities table
                let cap_list = WhatsAppRepository::get_capabilities_by_session(pool, session.id)
                    .await
                    .unwrap_or_default();
                let capabilities: HashSet<String> = cap_list.into_iter().collect();

                let context = WhatsAppUserContext {
                    session_id: session.id,
                    user_id: session.user_id,
                    school_id: session.school_id,
                    role: session.role_code.clone(),
                    capabilities,
                    phone: session.phone.clone(),
                    display_name: session.display_name.clone(),
                };

                // 0. Check if user is currently inside an active multi-step interactive workflow (e.g. attendance marking)
                if let Ok(Some((state_type, payload))) =
                    WhatsAppRepository::get_conversation_state(pool, session.id).await
                {
                    if Self::is_logout(trimmed)
                        || trimmed.eq_ignore_ascii_case("cancel")
                        || trimmed.eq_ignore_ascii_case("exit")
                        || trimmed.eq_ignore_ascii_case("abort")
                    {
                        let _ =
                            WhatsAppRepository::clear_conversation_state(pool, session.id).await;
                        info!(sender = %sender_jid, "User cancelled interactive WhatsApp conversation");
                        return vec![
                            "❌ *Attendance Cancelled*\n\nAttendance recording has been cancelled. Send *help* to view the menu.".to_string(),
                        ];
                    }

                    info!(sender = %sender_jid, state = %state_type, "Handling interactive WhatsApp conversation step");
                    return vec![
                        menus::attendance::handle_attendance_conversation_step(
                            pool,
                            &context,
                            trimmed,
                            &state_type,
                            &payload,
                        )
                        .await,
                    ];
                }

                // Check intent with capability authorization
                if Self::is_logout(trimmed) {
                    let _ = WhatsAppRepository::terminate_session(pool, sender_jid).await;
                    info!(sender = %sender_jid, "User logged out of WhatsApp session");
                    return vec!["🔒 *Signed Out Successfully*\n\nYour ERAVAYA WhatsApp session has been closed. Send any message when you wish to log in or link again.".to_string()];
                }

                if Self::is_help_request(trimmed) {
                    info!(sender = %sender_jid, role = %context.role, "Returning authenticated menu");
                    return vec![Self::format_authenticated_menu(&context)];
                }

                if Self::is_greeting(trimmed) {
                    let name = context.display_name.as_deref().unwrap_or("User");
                    return vec![format!(
                        "👋 Hello, *{name}*!\n\nWelcome back to ERAVAYA ERP (Logged in as *{}*).\nType *help* to view your options or ask any question.",
                        context.role
                    )];
                }

                // Explicit "take attendance" / "mark attendance" request
                if menus::attendance::is_take_attendance(trimmed) {
                    if context.is_school_admin() {
                        let app_url = std::env::var("APP_URL").unwrap_or_else(|_| {
                            "https://my-frontend-kappa-five.vercel.app".to_string()
                        });
                        let clean_url = app_url.trim_end_matches('/');
                        return vec![format!(
                            "🚫 *School Administrators Cannot Mark Attendance*\n\n\
                            Per strict institutional security standards, School Administrators and Super Admins cannot record initial roll-call. Attendance must be entered by designated Class Teachers or Subject Teachers.\n\n\
                            👉 View attendance registers, audit logs, and reports at:\n\
                            {clean_url}/dashboard/attendance"
                        )];
                    }
                    if !context.has_capability("attendance:record")
                        && !context.has_capability("attendance:mark")
                    {
                        warn!(sender = %sender_jid, role = %context.role, "Blocked attendance: missing attendance:record capability");
                        return vec![format!(
                            "🚫 *Permission Denied*\n\nYour account role (*{}*) lacks the required *attendance:record* capability to mark attendance. Please contact your administrator.",
                            context.role
                        )];
                    }
                    return vec![menus::attendance::start_attendance_flow(pool, &context).await];
                }

                // 1️⃣ Attendance Menu
                if menus::attendance::is_attendance(trimmed) {
                    if !context.has_capability("attendance:view")
                        && !context.has_capability("academics:view")
                        && !context.is_teacher()
                        && !context.is_school_admin()
                        && !context.is_student_or_parent()
                    {
                        warn!(sender = %sender_jid, role = %context.role, "Blocked attendance: missing attendance:view capability");
                        return vec![format!(
                            "🚫 *Permission Denied*\n\nYour account role (*{}*) does not have the *attendance:view* capability to view attendance.",
                            context.role
                        )];
                    }
                    return vec![menus::attendance::handle_attendance_query(pool, &context).await];
                }

                // 2️⃣ Fees Menu
                if menus::fees::is_fees(trimmed) {
                    if !context.has_capability("finance:view_fees") {
                        warn!(sender = %sender_jid, role = %context.role, "Blocked fees: missing finance:view_fees capability");
                        return vec![format!(
                            "🚫 *Permission Denied*\n\nYour account role (*{}*) does not have the *finance:view_fees* capability to view fee records.",
                            context.role
                        )];
                    }
                    return vec![menus::fees::handle_fees_query(pool, &context).await];
                }

                // 3️⃣ Timetable Menu
                if menus::timetable::is_timetable(trimmed) {
                    if !context.has_capability("academics:view") {
                        return vec![format!(
                            "🚫 *Permission Denied*\n\nYour account role (*{}*) does not have permission to view timetables.",
                            context.role
                        )];
                    }
                    return vec![menus::timetable::handle_timetable_query(pool, &context).await];
                }

                // 4️⃣ Exams Menu
                if menus::exams::is_exams(trimmed) {
                    if !context.has_capability("academics:view") {
                        return vec![format!(
                            "🚫 *Permission Denied*\n\nYour account role (*{}*) does not have permission to view exam schedules.",
                            context.role
                        )];
                    }
                    return vec![menus::exams::handle_exams_query(pool, &context).await];
                }

                // 5️⃣ Profile Menu
                if menus::profile::is_profile(trimmed) {
                    return vec![
                        menus::profile::handle_profile_query(pool, &context, &session).await,
                    ];
                }

                // Conversational query -> Route to AI / Fallback
                let history = WhatsAppRepository::get_recent_conversation(pool, sender_jid, 3)
                    .await
                    .unwrap_or_default();
                info!(sender = %sender_jid, role = %context.role, query = %trimmed, "Routing authorized WhatsApp query to AI/Fallback");
                vec![menus::ai::connect_to_ai(sender_jid, trimmed, &history).await]
            }
        }
    }

    /// Evaluates user intent and forms the reply string.
    #[allow(dead_code)]
    pub async fn generate_reply_text(
        pool: &PgPool,
        sender_jid: &str,
        incoming_text: &str,
    ) -> String {
        let msgs = Self::generate_reply_messages(pool, sender_jid, None, incoming_text).await;
        msgs.join("\n\n")
    }

    /// Process an incoming WhatsApp message, determine intent, and enqueue reply.
    pub async fn process_incoming(
        pool: &PgPool,
        sender_jid: &str,
        incoming_text: &str,
    ) -> Result<Option<String>, AppError> {
        let trimmed = incoming_text.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        // 1. Audit log into inbound table if not already saved
        let _ = WhatsAppRepository::record_inbound(
            pool,
            sender_jid,
            "text",
            serde_json::json!({ "text": trimmed }),
            Utc::now(),
        )
        .await;

        // 2. Generate replies
        let replies = Self::generate_reply_messages(pool, sender_jid, None, trimmed).await;
        if replies.is_empty() {
            return Ok(None);
        }

        // 3. Send each formatted reply back to sender JID via outbox queue
        for reply in &replies {
            let _ = send_whatsapp_message(pool, sender_jid, reply, None).await;
        }

        Ok(Some(replies.join("\n\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greeting_detection() {
        assert!(WhatsAppMessageService::is_greeting("hi"));
        assert!(WhatsAppMessageService::is_greeting("Hi"));
        assert!(WhatsAppMessageService::is_greeting("HI!"));
        assert!(WhatsAppMessageService::is_greeting("hello"));
        assert!(WhatsAppMessageService::is_greeting("hey"));
        assert!(WhatsAppMessageService::is_greeting("namaste"));
        assert!(WhatsAppMessageService::is_greeting("good morning"));
        assert!(WhatsAppMessageService::is_greeting("hi eravaya"));

        assert!(!WhatsAppMessageService::is_greeting(
            "what is the fee deadline?"
        ));
        assert!(!WhatsAppMessageService::is_greeting("attendance report"));
    }

    #[test]
    fn test_help_menu_detection() {
        assert!(WhatsAppMessageService::is_help_request("help"));
        assert!(WhatsAppMessageService::is_help_request("Help"));
        assert!(WhatsAppMessageService::is_help_request("menu"));
        assert!(WhatsAppMessageService::is_help_request("options"));
        assert!(WhatsAppMessageService::is_help_request("help menu"));

        assert!(!WhatsAppMessageService::is_help_request("hi"));
        assert!(!WhatsAppMessageService::is_help_request(
            "help me with fees"
        ));
    }

    #[test]
    fn test_welcome_greeting_content() {
        let greeting = WhatsAppMessageService::get_welcome_greeting();
        assert!(greeting.contains("Hi! Welcome to ERAVAYA ERP"));
        assert!(greeting.contains("help"));
    }

    #[test]
    fn test_fallback_message_content() {
        let fallback = menus::ai::format_fallback_message("Check my exam dates");
        assert!(fallback.contains("ERAVAYA ERP Assistant"));
        assert!(fallback.contains("Check my exam dates"));
        assert!(fallback.contains("help"));
    }

    #[test]
    fn test_whatsapp_user_context_admin_prohibition() {
        let admin = WhatsAppUserContext {
            session_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            school_id: Uuid::new_v4(),
            role: "SCHOOL_ADMIN".to_string(),
            capabilities: [
                "attendance:view",
                "attendance:bulk_edit",
                "attendance:delete",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            phone: "919999999999".to_string(),
            display_name: Some("Principal".to_string()),
        };

        assert!(admin.is_school_admin());
        assert!(admin.has_capability("attendance:view"));
        assert!(admin.has_capability("attendance:bulk_edit"));
        assert!(admin.has_capability("attendance:delete"));
        assert!(admin.has_capability("attendance.delete"));
        // Strictly blocked from recording / marking initial attendance!
        assert!(!admin.has_capability("attendance:record"));
        assert!(!admin.has_capability("attendance:mark"));
        assert!(!admin.has_capability("attendance.mark"));
    }

    #[test]
    fn test_whatsapp_user_context_teacher_capabilities() {
        let teacher = WhatsAppUserContext {
            session_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            school_id: Uuid::new_v4(),
            role: "TEACHER".to_string(),
            capabilities: [
                "attendance:view",
                "attendance:record",
                "attendance:edit_subject",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            phone: "919888888888".to_string(),
            display_name: Some("Teacher Anita".to_string()),
        };

        assert!(!teacher.is_school_admin());
        assert!(teacher.is_teacher());
        assert!(teacher.has_capability("attendance:record"));
        assert!(teacher.has_capability("attendance:mark"));
        assert!(teacher.has_capability("attendance.mark"));
        assert!(teacher.has_capability("attendance:edit"));
        assert!(teacher.has_capability("attendance.edit"));
        assert!(teacher.has_capability("attendance:view"));
        assert!(!teacher.has_capability("attendance:bulk_edit"));
        assert!(!teacher.has_capability("attendance:delete"));
    }
}
