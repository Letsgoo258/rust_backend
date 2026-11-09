use super::attendance::get_school_local_date;
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

    let local_date = get_school_local_date(pool, context.school_id).await;

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
               WHERE school_id = $1 AND date = $2"#,
        )
        .bind(context.school_id)
        .bind(local_date)
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

    let day_name = local_date.format("%A").to_string();
    let formatted_date = local_date.format("%d %b %Y").to_string();

    if is_teacher {
        // Look up teacher staff ID (matching school_id or user_id)
        let staff_opt = sqlx::query_as::<_, (uuid::Uuid,)>(
            r#"SELECT id FROM staff WHERE user_id = $1 AND (school_id = $2 OR school_id IS NULL) LIMIT 1"#,
        )
        .bind(context.user_id)
        .bind(context.school_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

        let staff_id = match staff_opt {
            Some((sid,)) => sid,
            None => {
                return format!(
                    "📅 *Teaching Schedule — {name}*\n\n\
                    ⚠️ *Staff Record Not Linked*\n\
                    Your user account is registered as a Teacher, but no linked Staff profile was found in the school roster.\n\n\
                    👉 *Please ask your School Administrator to verify your staff assignment in School Settings.*"
                );
            }
        };

        // Always ensure daily timetable projection is updated for today
        let _ = sqlx::query_scalar::<_, i32>("SELECT sync_daily_timetable_for_date($1)")
            .bind(local_date)
            .fetch_one(pool)
            .await;

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
                COALESCE(orig_u.display_name, orig_u.first_name, 'Teacher') as orig_teacher_name
               FROM current_day_timetable cdt
               JOIN subjects sub ON cdt.subject_id = sub.id
               JOIN class_sections cs ON cdt.class_section_id = cs.id
               JOIN classes c ON cs.class_id = c.id
               JOIN sections s ON cs.section_id = s.id
               LEFT JOIN staff orig_st ON cdt.original_teacher_id = orig_st.id
               LEFT JOIN users orig_u ON orig_st.user_id = orig_u.id
               WHERE cdt.school_id = $1 
                 AND (cdt.effective_teacher_id = $2 OR cdt.original_teacher_id = $2 OR cdt.co_teacher_ids @> ARRAY[$2::uuid])
                 AND cdt.date = $3
               ORDER BY cdt.period_number ASC"#,
        )
        .bind(context.school_id)
        .bind(staff_id)
        .bind(local_date)
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
                "📅 *Today's Teaching Schedule ({day_name}, {formatted_date}) — {name}*\n\n\
                {}\n\n\
                👉 *View Full Weekly Timetable Matrix*:\n\
                {clean_url}/dashboard/academics/timetable\n\n\
                _Updated live from ERAVAYA Daily Timetable._",
                slot_list
            );
        } else {
            // Check master timetable_slots to see teacher's weekly schedule across all days
            let weekly_slots =
                sqlx::query_as::<_, (i32, i32, String, String, String, String, String)>(
                    r#"SELECT 
                    ts.day_of_week,
                    ts.period_number,
                    to_char(ts.start_time, 'HH12:MI AM') as start_str,
                    to_char(ts.end_time, 'HH12:MI AM') as end_str,
                    sub.name as subject_name,
                    c.name as class_name,
                    s.name as section_name
                   FROM timetable_slots ts
                   JOIN subjects sub ON ts.subject_id = sub.id
                   JOIN class_sections cs ON ts.class_section_id = cs.id
                   JOIN classes c ON cs.class_id = c.id
                   JOIN sections s ON cs.section_id = s.id
                   WHERE ts.school_id = $1 
                     AND (ts.teacher_staff_id = $2 OR ts.co_teacher_staff_ids @> ARRAY[$2::uuid])
                     AND ts.status = 'ACTIVE'
                   ORDER BY ts.day_of_week ASC, ts.period_number ASC"#,
                )
                .bind(context.school_id)
                .bind(staff_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default();

            if !weekly_slots.is_empty() {
                let day_names = [
                    "",
                    "Monday",
                    "Tuesday",
                    "Wednesday",
                    "Thursday",
                    "Friday",
                    "Saturday",
                    "Sunday",
                ];
                let mut day_map: std::collections::BTreeMap<i32, Vec<String>> =
                    std::collections::BTreeMap::new();
                for (dow, p, _st, _et, sub, cls, sec) in weekly_slots {
                    day_map
                        .entry(dow)
                        .or_default()
                        .push(format!("P{p} ({sub} - {cls} {sec})"));
                }

                let mut weekly_summary = String::new();
                for (dow, items) in day_map {
                    let d_str = if (1..=7).contains(&dow) {
                        day_names[dow as usize]
                    } else {
                        "Day"
                    };
                    weekly_summary.push_str(&format!("• *{}*: {}\n", d_str, items.join(", ")));
                }

                return format!(
                    "📅 *Today's Teaching Schedule — {name}*\n\n\
                    🎉 *No teaching periods scheduled for today ({day_name}, {formatted_date})!*\n\
                    You have a free period schedule today or classes are not in session.\n\n\
                    📋 *Your Weekly Timetable Summary*:\n\
                    {}\n\
                    👉 *View Complete Timetable Matrix*:\n\
                    {clean_url}/dashboard/academics/timetable",
                    weekly_summary
                );
            } else {
                return format!(
                    "📅 *Teaching Schedule — {name}*\n\n\
                    ℹ️ *No teaching periods have been assigned to your timetable in the academic schedule.*\n\n\
                    👉 *Please contact your School Timetable Coordinator or Academic Administrator to assign your periods in School Settings > Academics > Timetable.*"
                );
            }
        }
    }

    // Student / Parent View: Look up student's class section
    // 1. Direct student user lookup
    let mut student_opt = sqlx::query_as::<_, (uuid::Uuid, String, String, String)>(
        r#"SELECT cs.id, c.name, s.name, u.first_name
           FROM users u
           JOIN enrollments e ON e.student_id = u.id AND e.status = 'ACTIVE'
           JOIN class_sections cs ON e.class_section_id = cs.id
           JOIN classes c ON cs.class_id = c.id
           JOIN sections s ON cs.section_id = s.id
           WHERE u.id = $1 AND u.school_id = $2
           LIMIT 1"#,
    )
    .bind(context.user_id)
    .bind(context.school_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    // 2. Parent user lookup: retrieve linked ward
    if student_opt.is_none() {
        student_opt = sqlx::query_as::<_, (uuid::Uuid, String, String, String)>(
            r#"SELECT cs.id, c.name, s.name, u.first_name
               FROM users u
               JOIN student_guardians sg ON sg.student_id = u.id
               JOIN users g ON g.id = sg.guardian_id OR sg.guardian_id IN (SELECT id FROM guardians WHERE user_id = g.id)
               JOIN enrollments e ON e.student_id = u.id AND e.status = 'ACTIVE'
               JOIN class_sections cs ON e.class_section_id = cs.id
               JOIN classes c ON cs.class_id = c.id
               JOIN sections s ON cs.section_id = s.id
               WHERE g.id = $1 AND u.school_id = $2
               ORDER BY u.created_at ASC
               LIMIT 1"#,
        )
        .bind(context.user_id)
        .bind(context.school_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    }

    if let Some((section_id, c_name, s_name, student_first_name)) = student_opt {
        // Ensure today's timetable is synced
        let _ = sqlx::query_scalar::<_, i32>("SELECT sync_daily_timetable_for_date($1)")
            .bind(local_date)
            .fetch_one(pool)
            .await;

        let today_slots = sqlx::query_as::<_, (i32, String, String, String, Option<String>, bool)>(
            r#"SELECT 
                cdt.period_number,
                to_char(cdt.start_time, 'HH12:MI AM') as start_str,
                to_char(cdt.end_time, 'HH12:MI AM') as end_str,
                sub.name as subject_name,
                COALESCE(eff_u.display_name, eff_u.first_name, 'Teacher') as teacher_name,
                cdt.is_substituted
               FROM current_day_timetable cdt
               JOIN subjects sub ON cdt.subject_id = sub.id
               LEFT JOIN staff eff_st ON cdt.effective_teacher_id = eff_st.id
               LEFT JOIN users eff_u ON eff_st.user_id = eff_u.id
               WHERE cdt.school_id = $1 AND cdt.class_section_id = $2 AND cdt.date = $3
               ORDER BY cdt.period_number ASC"#,
        )
        .bind(context.school_id)
        .bind(section_id)
        .bind(local_date)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        if !today_slots.is_empty() {
            let slot_list = today_slots
                .into_iter()
                .map(|(p, st, et, sub, teacher_opt, is_sub)| {
                    let teacher = teacher_opt.unwrap_or_else(|| "Teacher".into());
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

            let title_prefix = if context.is_student_or_parent() {
                format!("Today's Class Schedule for *{student_first_name}* — {c_name} (Section {s_name})")
            } else {
                format!("Today's Class Schedule — {c_name} (Section {s_name})")
            };

            return format!(
                "📅 *{title_prefix}*\n\
                📆 *Date*: {day_name}, {formatted_date}\n\n\
                {}\n\n\
                👉 *Student & Parent Portal*:\n\
                {clean_url}/dashboard\n\n\
                _Live updates from ERAVAYA School Timetable._",
                slot_list
            );
        } else {
            return format!(
                "📅 *Class Schedule — {c_name} (Section {s_name})*\n\n\
                🎉 *No classes scheduled for today ({day_name}, {formatted_date})!*\n\n\
                👉 *View Full Academic Calendar & Portal*:\n\
                {clean_url}/dashboard",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_timetable_keywords() {
        assert!(is_timetable("timetable"));
        assert!(is_timetable("Timetable"));
        assert!(is_timetable("Today's Schedule"));
        assert!(is_timetable("my routine"));
        assert!(is_timetable("4"));
        assert!(is_timetable("what period is next?"));
        assert!(is_timetable("class time"));
        assert!(is_timetable("today class"));
        assert!(!is_timetable("hello bot"));
        assert!(!is_timetable("admission status"));
    }
}
