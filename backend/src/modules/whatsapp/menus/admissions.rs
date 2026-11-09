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
/// - Checks if sender phone already has existing application or is a registered parent
/// - Pre-verifies WhatsApp number in `admission_otps`
/// - Generates the direct online admissions application URL with pre-verified token
pub async fn handle_admission_inquiry(
    pool: &PgPool,
    sender_phone: &str,
    _sender_jid: &str,
) -> Vec<String> {
    let app_url = std::env::var("APP_URL")
        .unwrap_or_else(|_| "https://my-frontend-kappa-five.vercel.app".to_string());
    let clean_url = app_url.trim_end_matches('/');

    let clean_phone: String = sender_phone
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    let phone_10 = if clean_phone.len() > 10 {
        clean_phone[clean_phone.len() - 10..].to_string()
    } else {
        clean_phone.clone()
    };

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

    if !is_open {
        return vec![format!(
            "ℹ️ *ERAVAYA Admissions Notice*\n\n\
            Admissions for *{school_name}* are currently *CLOSED*.\n\n\
            📅 *Intake Window*:\n\
            • Opens On: *{open_date}*\n\
            • Closes On: *{close_date}*\n\
            • Academic Year: *{ay_name}*\n\n\
            For special inquiries or upcoming admissions, please contact the school administrative desk."
        )];
    }

    // 3. Check for existing admissions associated with this phone number (support multiple children)
    let existing_admissions = sqlx::query_as::<
        _,
        (
            Uuid,
            Option<String>,
            String,
            String,
            String,
            Option<chrono::DateTime<Utc>>,
        ),
    >(
        r#"
        SELECT 
            a.id,
            a.application_number,
            a.status::text,
            aa.first_name,
            aa.last_name,
            a.submitted_at
        FROM admissions a
        JOIN admission_applicants aa ON aa.admission_id = a.id
        LEFT JOIN admission_guardians ag ON ag.admission_id = a.id
        WHERE a.school_id = $1
          AND (
              aa.phone = $2 OR aa.phone = $3
              OR ag.phone = $2 OR ag.phone = $3
          )
        ORDER BY a.created_at DESC
        LIMIT 5
        "#,
    )
    .bind(school_id)
    .bind(&clean_phone)
    .bind(&phone_10)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // 4. Check if sender is an existing registered user / parent in the school
    let existing_parent = sqlx::query_as::<
        _,
        (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        r#"
        SELECT display_name, first_name, last_name, email
        FROM users
        WHERE school_id = $1 AND (phone = $2 OR phone = $3)
        LIMIT 1
        "#,
    )
    .bind(school_id)
    .bind(&clean_phone)
    .bind(&phone_10)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    // 5. Check if parent already has enrolled student(s) in this school (Multi-child sibling check)
    let enrolled_children = sqlx::query_as::<
        _,
        (
            Option<String>,
            String,
            String,
            Option<String>,
            Option<String>,
        ),
    >(
        r#"
        SELECT 
            u.admission_number,
            u.first_name,
            u.last_name,
            c.name as class_name,
            s.name as section_name
        FROM users u
        JOIN student_guardians sg ON sg.student_id = u.id
        JOIN users g ON g.id = sg.guardian_id
        LEFT JOIN enrollments e ON e.student_id = u.id AND e.status = 'ACTIVE'
        LEFT JOIN class_sections cs ON cs.id = COALESCE(e.class_section_id, u.class_section_id)
        LEFT JOIN classes c ON c.id = cs.class_id
        LEFT JOIN sections s ON s.id = cs.section_id
        WHERE g.school_id = $1 AND (g.phone = $2 OR g.phone = $3)
        ORDER BY u.created_at ASC
        LIMIT 5
        "#,
    )
    .bind(school_id)
    .bind(&clean_phone)
    .bind(&phone_10)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let parent_name = existing_parent.as_ref().and_then(|p| {
        p.0.clone()
            .or_else(|| match (p.1.as_deref(), p.2.as_deref()) {
                (Some(f), Some(l)) if !f.is_empty() => Some(format!("{f} {l}").trim().to_string()),
                (Some(f), _) if !f.is_empty() => Some(f.to_string()),
                _ => None,
            })
    });
    let verified_email = existing_parent.as_ref().and_then(|p| p.3.clone());

    // 6. Generate a 24-hour pre-verified token for this WhatsApp number in admission_otps
    let token = format!("wa_verified_{}", Uuid::new_v4());
    let _ = sqlx::query(
        r#"
        INSERT INTO admission_otps (target, target_type, otp_code, verified, verification_token, expires_at)
        VALUES ($1, 'PHONE', 'WA_AUTO', true, $2, now() + INTERVAL '24 hours')
        "#,
    )
    .bind(&clean_phone)
    .bind(&token)
    .execute(pool)
    .await;

    // Also insert for 10-digit version if different
    if phone_10 != clean_phone {
        let _ = sqlx::query(
            r#"
            INSERT INTO admission_otps (target, target_type, otp_code, verified, verification_token, expires_at)
            VALUES ($1, 'PHONE', 'WA_AUTO', true, $2, now() + INTERVAL '24 hours')
            "#,
        )
        .bind(&phone_10)
        .bind(&token)
        .execute(pool)
        .await;
    }

    let mut apply_link = format!("{clean_url}/admissions/apply?token={token}&phone={clean_phone}");
    if let Some(ref em) = verified_email {
        if !em.trim().is_empty() {
            apply_link.push_str(&format!("&email={}", em.trim()));
        }
    }

    let mut response_messages = Vec::new();

    // If parent has enrolled children, summarize them
    if !enrolled_children.is_empty() {
        let mut child_lines = Vec::new();
        for (adm_no, f_name, l_name, cls, sec) in &enrolled_children {
            let class_str = match (cls, sec) {
                (Some(c), Some(s)) => format!("Class {c}-{s}"),
                (Some(c), None) => format!("Class {c}"),
                _ => "Class Assigned".to_string(),
            };
            let adm_str = adm_no.as_deref().unwrap_or("Enrolled");
            child_lines.push(format!(
                "• 🎓 *{f_name} {l_name}* ({class_str} | Adm No: `{adm_str}`)"
            ));
        }
        response_messages.push(format!(
            "👨‍👩‍👧 *Enrolled Children on Record ({school_name})*:\n\n{}\n\nWe are pleased to assist your family with further admissions!",
            child_lines.join("\n")
        ));
    }

    // If existing applications exist, summarize all of them (multi-child)
    if !existing_admissions.is_empty() {
        let mut app_lines = Vec::new();
        for (_, app_no, status, f_name, l_name, submitted_at) in &existing_admissions {
            let app_label = app_no.as_deref().unwrap_or("Pending Assignment");
            let date_str = submitted_at
                .map(|d| d.format("%d %b %Y").to_string())
                .unwrap_or_else(|| "Recently".to_string());
            let status_upper = status.to_uppercase();

            let status_badge = match status_upper.as_str() {
                "APPROVED" => "✅ *APPROVED* (Ready for office enrollment)",
                "ENROLLED" => "🎓 *ENROLLED*",
                "REJECTED" => "❌ *NOT ACCEPTED*",
                _ => "⏳ *UNDER REVIEW*",
            };

            app_lines.push(format!(
                "• *{f_name} {l_name}*\n  Application No: `{app_label}`\n  Status: {status_badge}\n  Submitted: {date_str}"
            ));
        }

        response_messages.push(format!(
            "📋 *Applications on Record*:\n\n{}\n\nTo apply for an additional child, please use the link below.",
            app_lines.join("\n\n")
        ));
    }

    let greeting = if let Some(name) = parent_name {
        format!("Welcome, *{name}*! 👋")
    } else {
        "Welcome to *ERAVAYA Admissions*! 👋".to_string()
    };

    let apply_action_label = if !enrolled_children.is_empty() || !existing_admissions.is_empty() {
        "➕ *Apply for Another Child / New Admission*"
    } else {
        "👉 *Start Online Admission Application*"
    };

    response_messages.push(format!(
        "🎓 *{school_name}*\n\
        {greeting}\n\n\
        Admissions are *OPEN* for Academic Year *{ay_name}*.\n\n\
        ✅ *Pre-Verified Mobile Number*:\n\
        Because you reached us directly on WhatsApp, your mobile number (*+{clean_phone}*) is *already verified*. You will bypass phone OTP on the portal!\n\n\
        {apply_action_label}:\n\
        {apply_link}\n\n\
        📸 *What you will need*:\n\
        • Student Passport Photo\n\
        • Applicant Date of Birth & Bio-data\n\
        • Guardian details & Residential Address\n\
        • Uploaded documents (as configured by school)\n\n\
        📄 *Document Delivery*:\n\
        Upon submission, your official box-box formatted Admission Form will be generated and a 24-hour secure ImageKit download link will be delivered to your email and WhatsApp!"
    ));

    response_messages
}
