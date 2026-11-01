use crate::modules::whatsapp::service::WhatsAppUserContext;
use sqlx::PgPool;

/// Detects if the incoming message is a timetable request.
pub fn is_timetable(text: &str) -> bool {
    let clean = text.trim().to_lowercase();
    clean.contains("timetable")
        || clean.contains("schedule")
        || clean.contains("routine")
        || clean == "4"
        || clean.contains("period")
        || clean.contains("class time")
        || clean.contains("today class")
}

/// Handles timetable queries:
/// - School Admin / Super Admin: Today's active periods count, active substitutions count, and portal link
/// - Teacher: Today's personal teaching schedule with section, room, and substitute notices
/// - Student / Parent: Today's class section timetable with subject, timing, and substitute alerts
pub async fn handle_timetable_query(pool: &PgPool, context: &WhatsAppUserContext) -> String {
    let name = context.display_name.as_deref().unwrap_or("User");
    let app_url = std::env::var("APP_URL")
        .unwrap_or_else(|_| "https://my-frontend-kappa-five.vercel.app".to_string());
    let clean_url = app_url.trim_end_matches('/');

    let is_admin = context.role.eq_ignore_ascii_case("SUPER_ADMIN")
        || context.role.eq_ignore_ascii_case("SCHOOL_ADMIN")
        || context.role.eq_ignore_ascii_case("HEADMASTER")
        || context.role.eq_ignore_ascii_case("PRINCIPAL");

    let is_teacher = context.is_teacher();

    if is_admin {
        // Admin overview: current day timetable metrics
        let stats = sqlx::query_as::<_, (i64, i64)>(
            r#"SELECT 
                COUNT(*)::bigint as total_slots,
                COUNT(CASE WHEN is_substituted = true THEN 1 END)::bigint as total_substitutions
               FROM current_day_timetable
               WHERE school_id = $1 AND date = CURRENT_DATE"#,
        )
        .bind(context.school_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or((0, 0));

        let class_rows = sqlx::query_as::<_, (String, String)>(
            r#"SELECT c.name, s.name
               FROM class_sections cs
               JOIN classes c ON cs.class_id = c.id
               JOIN sections s ON cs.section_id = s.id
               WHERE c.school_id = $1 AND cs.status = 'ACTIVE'
               ORDER BY c.name, s.name
               LIMIT 6"#,
        )
        .bind(context.school_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let class_list = if !class_rows.is_empty() {
            class_rows
                .into_iter()
                .map(|(c, s)| format!("• {c} (Section {s})"))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            "• Standard Grade Sections Active".to_string()
        };

        return format!(
            "📅 *Today's School Timetable & Substitution Dashboard*\n\n\
            📊 *Today's Overview*:\n\
            • Total Scheduled Periods: {}\n\
            • Active Substitutions Today: {}\n\n\
            🏫 *Active Classes & Sections*:\n\
            {}\n\n\
            👉 *Manage & Modify Master Timetable*:\n\
            {clean_url}/dashboard/academics/timetable\n\n\
            _Real-time timetable synchronizer resets daily at 01:30 AM._",
            stats.0, stats.1, class_list
        );
    }

    if is_teacher {
        // Look up teacher staff ID
        let staff_opt = sqlx::query_as::<_, (uuid::Uuid,)>(
            r#"SELECT id FROM staff WHERE user_id = $1 AND school_id = $2 LIMIT 1"#,
        )
        .bind(context.user_id)
        .bind(context.school_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

        if let Some((staff_id,)) = staff_opt {
            let today_slots = sqlx::query_as::<
                _,
                (
                    i32,
                    String,
                    String,
                    String,
                    String,
                    String,
                    bool,
                    Option<String>,
                ),
            >(
                r#"SELECT 
                    cdt.period_number,
                    to_char(cdt.start_time, 'HH12:MI AM') as start_str,
                    to_char(cdt.end_time, 'HH12:MI AM') as end_str,
                    sub.name as subject_name,
                    c.name as class_name,
                    s.name as section_name,
                    cdt.is_substituted,
                    orig_u.name as orig_teacher_name
                   FROM current_day_timetable cdt
                   JOIN subjects sub ON cdt.subject_id = sub.id
                   JOIN class_sections cs ON cdt.class_section_id = cs.id
                   JOIN classes c ON cs.class_id = c.id
                   JOIN sections s ON cs.section_id = s.id
                   JOIN staff orig_st ON cdt.original_teacher_id = orig_st.id
                   JOIN users orig_u ON orig_st.user_id = orig_u.id
                   WHERE cdt.school_id = $1 AND cdt.effective_teacher_id = $2 AND cdt.date = CURRENT_DATE
                   ORDER BY cdt.period_number ASC"#,
            )
            .bind(context.school_id)
            .bind(staff_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            if !today_slots.is_empty() {
                let slot_list = today_slots
                    .into_iter()
                    .map(|(p, st, et, sub, cls, sec, is_sub, orig_teacher)| {
                        if is_sub {
                            let orig = orig_teacher.unwrap_or_else(|| "Staff".into());
                            format!("• *Period {p}* ({st} - {et}): {sub} — {cls} {sec}\n  *(🔄 Substitution for {orig})*")
                        } else {
                            format!("• *Period {p}* ({st} - {et}): {sub} — {cls} {sec}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");

                return format!(
                    "📅 *Today's Teaching Schedule - {name}*\n\n\
                    {}\n\n\
                    👉 *View Weekly Timetable Matrix*:\n\
                    {clean_url}/dashboard/academics/timetable\n\n\
                    _Updated live from ERAVAYA Daily Timetable._",
                    slot_list
                );
            } else {
                return format!(
                    "📅 *Today's Teaching Schedule - {name}*\n\n\
                    🎉 *No teaching periods scheduled for today!* You have a free period schedule or today is a non-instructional day.\n\n\
                    👉 *Check Full Weekly Schedule*:\n\
                    {clean_url}/dashboard/academics/timetable",
                );
            }
        }
    }

    // Student / Parent View: Look up student's class section
    let student_opt = sqlx::query_as::<_, (uuid::Uuid, String, String)>(
        r#"SELECT cs.id, c.name, s.name
           FROM students stu
           JOIN class_sections cs ON stu.class_section_id = cs.id
           JOIN classes c ON cs.class_id = c.id
           JOIN sections s ON cs.section_id = s.id
           WHERE (stu.user_id = $1 OR stu.id = $1) AND stu.school_id = $2
           LIMIT 1"#,
    )
    .bind(context.user_id)
    .bind(context.school_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if let Some((section_id, c_name, s_name)) = student_opt {
        let today_slots = sqlx::query_as::<_, (i32, String, String, String, String, bool)>(
            r#"SELECT 
                cdt.period_number,
                to_char(cdt.start_time, 'HH12:MI AM') as start_str,
                to_char(cdt.end_time, 'HH12:MI AM') as end_str,
                sub.name as subject_name,
                eff_u.name as teacher_name,
                cdt.is_substituted
               FROM current_day_timetable cdt
               JOIN subjects sub ON cdt.subject_id = sub.id
               JOIN staff eff_st ON cdt.effective_teacher_id = eff_st.id
               JOIN users eff_u ON eff_st.user_id = eff_u.id
               WHERE cdt.school_id = $1 AND cdt.class_section_id = $2 AND cdt.date = CURRENT_DATE
               ORDER BY cdt.period_number ASC"#,
        )
        .bind(context.school_id)
        .bind(section_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        if !today_slots.is_empty() {
            let slot_list = today_slots
                .into_iter()
                .map(|(p, st, et, sub, teacher, is_sub)| {
                    if is_sub {
                        format!(
                            "• *Period {p}* ({st} - {et}): {sub}\n  👨‍🏫 {teacher} *(Substitute)*"
                        )
                    } else {
                        format!("• *Period {p}* ({st} - {et}): {sub}\n  👨‍🏫 {teacher}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n\n");

            return format!(
                "📅 *Today's Class Schedule — {c_name} (Section {s_name})*\n\n\
                {}\n\n\
                👉 *Student Portal*:\n\
                {clean_url}/dashboard\n\n\
                _Live updates from ERAVAYA School Timetable._",
                slot_list
            );
        }
    }

    // Default Fallback
    format!(
        "📅 *ERAVAYA School Timetable*\n\n\
        ⏰ *Standard Timings*: 09:00 AM – 03:30 PM\n\n\
        👉 *Log in to view complete class timetables and live teacher substitutions*:\n\
        {clean_url}/dashboard/academics/timetable\n\n\
        _Need assistance? Reach out to your school administrative office._"
    )
}
