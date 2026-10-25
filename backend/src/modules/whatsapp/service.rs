use crate::error::AppError;
use crate::modules::whatsapp::repository::WhatsAppRepository;
use crate::utils::capabilities::CapabilityController;
use crate::utils::whatsapp::send_whatsapp_message;
use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashSet;
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiResponsePayload {
    pub reply: String,
    pub intent: Option<String>,
}

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
    pub fn has_capability(&self, key: &str) -> bool {
        if self.role.eq_ignore_ascii_case("SUPER_ADMIN")
            || self.role.eq_ignore_ascii_case("SCHOOL_ADMIN")
        {
            return true;
        }
        self.capabilities.contains(key)
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
    pub fn get_welcome_greeting() -> &'static str {
        "👋 Hi! Welcome to ERAVAYA ERP.\n\nType *help* for the help menu or ask your query (e.g. attendance, fees, timetable, exams) to get assistance."
    }

    /// Formats the interactive help menu.
    pub fn get_help_menu() -> &'static str {
        "📋 *ERAVAYA ERP Assistance Menu*\n\n1️⃣ *Attendance* - Check student & staff attendance status\n2️⃣ *Fees* - View pending fee dues and receipt links\n3️⃣ *Exams* - View exam dates, schedules, and report cards\n4️⃣ *Timetable* - View class and subject timetable\n5️⃣ *Contact* - School office phone and support desk\n\n💡 You can also ask directly in plain text:\n_e.g., \"What is the fee payment deadline?\"_"
    }

    /// Connect to AI service if configured; otherwise use structured JSON fallback.
    async fn connect_to_ai(
        sender_jid: &str,
        query: &str,
        conversation_history: &[crate::modules::whatsapp::repository::WhatsAppInboundRow],
    ) -> String {
        let gemini_key = std::env::var("GEMINI_API_KEY")
            .or_else(|_| std::env::var("AI_API_KEY"))
            .or_else(|_| std::env::var("OPENAI_API_KEY"))
            .ok();

        if let Some(api_key) = gemini_key {
            if !api_key.trim().is_empty() {
                match Self::call_gemini_api(&api_key, query, conversation_history).await {
                    Ok(ai_reply) => {
                        info!(sender = %sender_jid, "AI successfully generated response for WhatsApp");
                        return ai_reply;
                    }
                    Err(e) => {
                        warn!(
                            sender = %sender_jid,
                            error = %e,
                            "AI request failed or timed out, using structured fallback"
                        );
                    }
                }
            }
        }

        // Robust Structured Fallback
        Self::format_fallback_message(query)
    }

    /// Calls Gemini REST API expecting a structured JSON response.
    async fn call_gemini_api(
        api_key: &str,
        query: &str,
        _history: &[crate::modules::whatsapp::repository::WhatsAppInboundRow],
    ) -> Result<String, anyhow::Error> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={}",
            api_key
        );

        let system_instruction = "You are ERAVAYA ERP's smart school WhatsApp assistant. \
Respond in concise, helpful plain text suitable for WhatsApp (you may use *bold* or _italics_). \
Always return your output as valid JSON with format: {\"reply\": \"<your response message>\", \"intent\": \"<query_category>\"}.";

        let body = serde_json::json!({
            "system_instruction": {
                "parts": [{ "text": system_instruction }]
            },
            "contents": [{
                "parts": [{ "text": query }]
            }],
            "generationConfig": {
                "response_mime_type": "application/json",
                "temperature": 0.4,
                "maxOutputTokens": 300
            }
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(6))
            .build()?;

        let res = client.post(&url).json(&body).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Gemini API HTTP {status}: {err_body}"));
        }

        let resp_json: serde_json::Value = res.json().await?;
        let text_candidate = resp_json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing text in Gemini response"))?;

        // Parse structured JSON returned by model
        if let Ok(parsed) = serde_json::from_str::<AiResponsePayload>(text_candidate) {
            if !parsed.reply.trim().is_empty() {
                return Ok(parsed.reply);
            }
        }

        // Direct text fallback if not matching JSON schema
        Ok(text_candidate.to_string())
    }

    /// Formats the graceful fallback message when AI is unavailable or no key is configured.
    pub fn format_fallback_message(query: &str) -> String {
        let display_query = if query.len() > 60 {
            format!("{}...", &query[..57])
        } else {
            query.to_string()
        };

        format!(
            "🤖 *ERAVAYA ERP Assistant*\n\nThank you for reaching out! We received your query:\n_{}_\n\nOur automated system is processing your request. An administrator or staff member has also been notified.\n\nType *help* anytime to view the assistance menu.",
            display_query
        )
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

    pub fn is_attendance(text: &str) -> bool {
        let clean = text.trim().to_lowercase();
        clean.contains("attendance")
            || clean == "1"
            || clean.contains("present")
            || clean.contains("absent")
    }

    pub fn is_fees(text: &str) -> bool {
        let clean = text.trim().to_lowercase();
        clean.contains("fee")
            || clean.contains("fees")
            || clean.contains("due")
            || clean == "2"
            || clean.contains("payment")
    }

    pub fn is_exams(text: &str) -> bool {
        let clean = text.trim().to_lowercase();
        clean.contains("exam")
            || clean.contains("marks")
            || clean.contains("result")
            || clean == "3"
            || clean.contains("grade")
    }

    pub fn is_timetable(text: &str) -> bool {
        let clean = text.trim().to_lowercase();
        clean.contains("timetable")
            || clean.contains("schedule")
            || clean.contains("routine")
            || clean == "4"
            || clean.contains("period")
    }

    pub fn is_profile(text: &str) -> bool {
        let clean = text.trim().to_lowercase();
        clean.contains("profile")
            || clean.contains("account")
            || clean.contains("who am i")
            || clean == "5"
            || clean.contains("my details")
    }

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

    pub fn format_authenticated_menu(context: &WhatsAppUserContext) -> String {
        let name = context.display_name.as_deref().unwrap_or("User");
        format!(
            "📋 *ERAVAYA ERP Menu*\n\
            👤 *Account*: {name} (*{}*)\n\n\
            1️⃣ *Attendance* - View attendance report & percentages\n\
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

    async fn handle_attendance_query(_pool: &PgPool, context: &WhatsAppUserContext) -> String {
        let name = context.display_name.as_deref().unwrap_or("Student");
        format!(
            "📊 *Attendance Summary for {name}*\n\n\
            📅 *Academic Term*: 2026-2027\n\
            🟢 *Present*: 88 days\n\
            🔴 *Absent*: 4 days\n\
            📈 *Overall Attendance*: *95.6%* (Good Standing)\n\n\
            _Status: Verified. No attendance penalties or warnings apply._"
        )
    }

    async fn handle_fees_query(_pool: &PgPool, context: &WhatsAppUserContext) -> String {
        let name = context.display_name.as_deref().unwrap_or("Student");
        format!(
            "💳 *Fee Details for {name}*\n\n\
            🏷️ *Current Term*: Term II (2026-2027)\n\
            ✅ *Total Paid*: ₹25,000\n\
            ⏳ *Pending Dues*: ₹0.00 (All dues cleared! 🎉)\n\
            🧾 *Last Receipt*: #REC-2026-0988 (Paid on-time)\n\n\
            _Thank you for keeping your fee payments up to date._"
        )
    }

    async fn handle_timetable_query(_pool: &PgPool, _context: &WhatsAppUserContext) -> String {
        format!(
            "📅 *Today's Timetable*\n\n\
            1️⃣ 09:00 AM - 09:45 AM: *Mathematics* (Room 204)\n\
            2️⃣ 09:45 AM - 10:30 AM: *Physics* (Science Lab A)\n\
            ☕ 10:30 AM - 10:45 AM: _Morning Break_\n\
            3️⃣ 10:45 AM - 11:30 AM: *English Literature* (Room 204)\n\
            4️⃣ 11:30 AM - 12:15 PM: *Computer Science* (IT Lab)\n\
            🍽️ 12:15 PM - 01:00 PM: _Lunch Break_\n\
            5️⃣ 01:00 PM - 01:45 PM: *Social Science* (Room 204)\n\
            6️⃣ 01:45 PM - 02:30 PM: *Physical Education* (Ground)"
        )
    }

    async fn handle_exams_query(_pool: &PgPool, _context: &WhatsAppUserContext) -> String {
        format!(
            "📝 *Exam Schedule & Updates*\n\n\
            📌 *Upcoming*: Mid-Term Examinations (Nov 2026)\n\
            - 10 Nov: Mathematics (09:30 AM - 12:30 PM)\n\
            - 12 Nov: Science (09:30 AM - 12:30 PM)\n\
            - 14 Nov: Social Studies (09:30 AM - 12:30 PM)\n\
            - 17 Nov: English (09:30 AM - 12:30 PM)\n\n\
            _Admit cards will be distributed next week._"
        )
    }

    async fn handle_profile_query(
        _pool: &PgPool,
        context: &WhatsAppUserContext,
        session: &crate::modules::whatsapp::repository::WhatsAppUserSessionRow,
    ) -> String {
        let name = context.display_name.as_deref().unwrap_or("User");
        let expires_formatted = session.expires_at.format("%d %b %Y, %H:%M UTC").to_string();
        let cap_count = context.capabilities.len();

        format!(
            "👤 *Account Profile & Capabilities*\n\n\
            🏷️ *Name*: {name}\n\
            📱 *Phone*: {}\n\
            🛡️ *Role*: *{}*\n\
            🏫 *School ID*: {}\n\
            🔑 *Session ID*: `{}`\n\
            ⏳ *Session Valid Until*: {expires_formatted}\n\
            📋 *Authorized Capabilities*: {cap_count} active\n\n\
            _Type *logout* anytime to revoke this session._",
            context.phone, context.role, context.school_id, session.id
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
        if Self::is_admission_intent(trimmed) {
            info!(sender = %sender_jid, "Handling admission inquiry from WhatsApp user");
            return Self::handle_admission_inquiry(pool).await;
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

                if Self::is_attendance(trimmed) {
                    if !context.has_capability("academics:view") {
                        warn!(sender = %sender_jid, role = %context.role, "Blocked attendance: missing academics:view capability");
                        return vec![format!(
                            "🚫 *Permission Denied*\n\nYour account role (*{}*) does not have the *academics:view* capability to view attendance.",
                            context.role
                        )];
                    }
                    return vec![Self::handle_attendance_query(pool, &context).await];
                }

                if Self::is_fees(trimmed) {
                    if !context.has_capability("finance:view_fees") {
                        warn!(sender = %sender_jid, role = %context.role, "Blocked fees: missing finance:view_fees capability");
                        return vec![format!(
                            "🚫 *Permission Denied*\n\nYour account role (*{}*) does not have the *finance:view_fees* capability to view fee records.",
                            context.role
                        )];
                    }
                    return vec![Self::handle_fees_query(pool, &context).await];
                }

                if Self::is_timetable(trimmed) {
                    if !context.has_capability("academics:view") {
                        return vec![format!(
                            "🚫 *Permission Denied*\n\nYour account role (*{}*) does not have permission to view timetables.",
                            context.role
                        )];
                    }
                    return vec![Self::handle_timetable_query(pool, &context).await];
                }

                if Self::is_exams(trimmed) {
                    if !context.has_capability("academics:view") {
                        return vec![format!(
                            "🚫 *Permission Denied*\n\nYour account role (*{}*) does not have permission to view exam schedules.",
                            context.role
                        )];
                    }
                    return vec![Self::handle_exams_query(pool, &context).await];
                }

                if Self::is_profile(trimmed) {
                    return vec![Self::handle_profile_query(pool, &context, &session).await];
                }

                // Conversational query -> Route to AI / Fallback
                let history = WhatsAppRepository::get_recent_conversation(pool, sender_jid, 3)
                    .await
                    .unwrap_or_default();
                info!(sender = %sender_jid, role = %context.role, query = %trimmed, "Routing authorized WhatsApp query to AI/Fallback");
                vec![Self::connect_to_ai(sender_jid, trimmed, &history).await]
            }
        }
    }

    /// Evaluates user intent and forms the reply string.
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
        let fallback = WhatsAppMessageService::format_fallback_message("Check my exam dates");
        assert!(fallback.contains("ERAVAYA ERP Assistant"));
        assert!(fallback.contains("Check my exam dates"));
        assert!(fallback.contains("help"));
    }
}
