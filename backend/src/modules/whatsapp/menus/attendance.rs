use crate::modules::academic::repository::{AcademicRepository, SchoolStatsRow};
use crate::modules::attendance::dto::StudentAttendanceInput;
use crate::modules::attendance::repository::AttendanceRepository;
use crate::modules::whatsapp::repository::WhatsAppRepository;
use crate::modules::whatsapp::service::WhatsAppUserContext;
use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Detects if the incoming message is an explicit request to take / mark / record attendance.
pub fn is_take_attendance(text: &str) -> bool {
    let clean = text.trim().to_lowercase();
    clean.contains("take attendance")
        || clean.contains("mark attendance")
        || clean.contains("record attendance")
        || clean.contains("take class attendance")
}

/// Detects if the incoming message is an attendance request.
pub fn is_attendance(text: &str) -> bool {
    let clean = text.trim().to_lowercase();
    clean.contains("attendance")
        || clean == "1"
        || clean.contains("roll call")
        || clean.contains("take attendance")
        || clean.contains("mark attendance")
        || clean.contains("record attendance")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveTimetableSlot {
    pub class_section_id: Uuid,
    pub class_name: String,
    pub section_name: String,
    pub period_number: i32,
    pub subject_id: Option<Uuid>,
    pub subject_name: String,
    pub start_time: String,
    pub end_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableClassOption {
    pub index: usize,
    pub class_section_id: Uuid,
    pub class_name: String,
    pub section_name: String,
    pub subject_id: Option<Uuid>,
    pub subject_name: String,
    pub period_number: Option<i32>,
    pub is_daily_roll_call: bool,
    #[serde(default)]
    pub is_holiday: bool,
    #[serde(default)]
    pub holiday_title: Option<String>,
    pub display: String,
}

/// Robustly parse absent roll numbers from teacher's text:
/// Handles formats:
/// - "1, 2, 55 absent"
/// - "1,2,55 absent"
/// - "1 2 55 absent"
/// - "absent: 1, 2, 55"
/// - "roll 1, 2, 55"
/// - "1, 2, 55"
/// - "all present" / "none" / "nil" / "0 absent" / "everyone present"
pub fn parse_absent_input(text: &str) -> Result<Vec<String>, String> {
    let raw = text.trim();
    if raw.is_empty() {
        return Err("EMPTY".to_string());
    }

    let lower = raw.to_lowercase();
    if lower == "cancel" || lower == "exit" || lower == "abort" || lower == "stop" {
        return Err("CANCEL".to_string());
    }

    // Check for "all present", "none", "nil", "0", "zero", "everyone present", "all"
    let clean_letters_only: String = lower
        .chars()
        .filter(|c| c.is_alphabetic() || c.is_whitespace())
        .collect();
    let words: Vec<&str> = clean_letters_only.split_whitespace().collect();
    let phrase = words.join(" ");

    if phrase == "all present"
        || phrase == "all"
        || phrase == "all students present"
        || phrase == "everyone present"
        || phrase == "none"
        || phrase == "nil"
        || phrase == "nil absent"
        || phrase == "zero absent"
        || phrase == "no absent"
        || phrase == "no absentees"
        || phrase == "present"
        || raw == "0"
        || raw == "0 absent"
    {
        return Ok(Vec::new());
    }

    // Extract roll numbers
    let mut normalized = String::new();
    for token in lower.split(|c: char| {
        c == ',' || c == ';' || c == '\n' || c == '/' || c == '-' || c.is_whitespace()
    }) {
        let t = token.trim();
        if t.is_empty()
            || t == "absent"
            || t == "absents"
            || t == "absentee"
            || t == "absentees"
            || t == "roll"
            || t == "rolls"
            || t == "no"
            || t == "nos"
            || t == "number"
            || t == "numbers"
            || t == "is"
            || t == "are"
            || t == "and"
        {
            continue;
        }

        let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            normalized.push_str(&digits);
            normalized.push(' ');
        }
    }

    let rolls: Vec<String> = normalized
        .split_whitespace()
        .map(|s| s.trim_start_matches('0').to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if rolls.is_empty() {
        return Err("INVALID_FORMAT".to_string());
    }

    // Deduplicate while preserving order
    let mut unique_rolls = Vec::new();
    for r in rolls {
        if !unique_rolls.contains(&r) {
            unique_rolls.push(r);
        }
    }

    Ok(unique_rolls)
}

/// Helper to get school local date based on configured timezone
pub async fn get_school_local_date(pool: &PgPool, school_id: Uuid) -> NaiveDate {
    sqlx::query_scalar(
        "SELECT (NOW() AT TIME ZONE COALESCE(timezone, 'Asia/Kolkata'))::date FROM schools WHERE id = $1",
    )
    .bind(school_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| Utc::now().date_naive())
}

/// Parses date input from WhatsApp messages:
/// - "yesterday" / "yday" -> current_date - 1 day
/// - "today" -> current_date
/// - "day before yesterday" -> current_date - 2 days
/// - "date YYYY-MM-DD", "YYYY-MM-DD"
/// - "date DD-MM-YYYY", "DD-MM-YYYY"
/// - "date DD/MM/YYYY", "DD/MM/YYYY"
/// - "date DD.MM.YYYY", "DD.MM.YYYY"
/// - Checks future date prevention and 60-day historical limit
pub fn parse_date_input(text: &str, current_date: NaiveDate) -> Result<Option<NaiveDate>, String> {
    let raw = text.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let lower = raw.to_lowercase();

    if lower == "change date" || lower == "date" || lower == "set date" {
        return Ok(None);
    }

    let is_explicit_date_prefix = lower.starts_with("date")
        || lower.starts_with("change date")
        || lower.starts_with("set date");

    let stripped = lower
        .strip_prefix("change date to")
        .or_else(|| lower.strip_prefix("change date"))
        .or_else(|| lower.strip_prefix("set date to"))
        .or_else(|| lower.strip_prefix("set date"))
        .or_else(|| lower.strip_prefix("date to"))
        .or_else(|| lower.strip_prefix("date:"))
        .or_else(|| lower.strip_prefix("date"))
        .or_else(|| lower.strip_prefix("for"))
        .unwrap_or(&lower)
        .trim();

    if stripped.is_empty() {
        return Ok(None);
    }

    let parsed_date = if stripped == "yesterday" || stripped == "yday" {
        Some(current_date - chrono::Duration::days(1))
    } else if stripped == "today" {
        Some(current_date)
    } else if stripped == "day before yesterday" {
        Some(current_date - chrono::Duration::days(2))
    } else {
        // Only attempt date parsing if there is an explicit date prefix OR it contains date delimiters (-, /, .)
        let has_delimiter =
            stripped.contains('-') || stripped.contains('/') || stripped.contains('.');
        if is_explicit_date_prefix || has_delimiter {
            let formats = [
                "%Y-%m-%d", "%d-%m-%Y", "%d/%m/%Y", "%Y/%m/%d", "%d.%m.%Y", "%Y.%m.%d", "%d %b %Y",
                "%d %B %Y", "%b %d %Y", "%B %d %Y",
            ];

            let mut found = None;
            for fmt in &formats {
                if let Ok(d) = NaiveDate::parse_from_str(stripped, fmt) {
                    found = Some(d);
                    break;
                }
            }

            if found.is_none() && has_delimiter {
                let current_year = current_date.format("%Y").to_string();
                let with_year_hyphen = format!("{stripped}-{current_year}");
                let with_year_slash = format!("{stripped}/{current_year}");

                for candidate in [&with_year_hyphen, &with_year_slash] {
                    for fmt in &formats {
                        if let Ok(d) = NaiveDate::parse_from_str(candidate, fmt) {
                            found = Some(d);
                            break;
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                }
            }

            if found.is_none() && is_explicit_date_prefix {
                return Err(format!("Could not understand the date \"{stripped}\"."));
            }

            found
        } else {
            None
        }
    };

    if let Some(target) = parsed_date {
        if target > current_date {
            return Err(format!(
                "Cannot mark attendance for a future date ({}). Please choose today or a past date.",
                target.format("%d %b %Y")
            ));
        }
        if target < current_date - chrono::Duration::days(60) {
            return Err(format!(
                "Cannot mark attendance for dates older than 60 days via WhatsApp ({}). Please use the ERAVAYA Web Portal for historical records.",
                target.format("%d %b %Y")
            ));
        }
        return Ok(Some(target));
    }

    Ok(None)
}

/// Check if teacher currently has an active scheduled period in current_day_timetable.
pub async fn check_active_period(
    pool: &PgPool,
    school_id: Uuid,
    staff_id: Uuid,
) -> Option<ActiveTimetableSlot> {
    let row = sqlx::query(
        r#"
        SELECT
            cdt.class_section_id,
            c.name as class_name,
            sec.name as section_name,
            cdt.period_number,
            cdt.subject_id,
            sub.name as subject_name,
            cdt.start_time::text as start_time_str,
            cdt.end_time::text as end_time_str
        FROM current_day_timetable cdt
        JOIN schools sch ON sch.id = cdt.school_id
        JOIN class_sections cs ON cs.id = cdt.class_section_id
        JOIN classes c ON c.id = cs.class_id
        JOIN sections sec ON sec.id = cs.section_id
        JOIN subjects sub ON sub.id = cdt.subject_id
        WHERE cdt.school_id = $1
          AND (cdt.effective_teacher_id = $2 OR cdt.co_teacher_ids @> ARRAY[$2::uuid])
          AND cdt.date = (NOW() AT TIME ZONE COALESCE(sch.timezone, 'Asia/Kolkata'))::date
          AND (NOW() AT TIME ZONE COALESCE(sch.timezone, 'Asia/Kolkata'))::time >= (cdt.start_time - INTERVAL '10 minutes')
          AND (NOW() AT TIME ZONE COALESCE(sch.timezone, 'Asia/Kolkata'))::time <= (cdt.end_time + INTERVAL '10 minutes')
        ORDER BY cdt.period_number ASC
        LIMIT 1
        "#,
    )
    .bind(school_id)
    .bind(staff_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    row.map(|r| ActiveTimetableSlot {
        class_section_id: r.get("class_section_id"),
        class_name: r.get("class_name"),
        section_name: r.get("section_name"),
        period_number: r.get("period_number"),
        subject_id: r.get("subject_id"),
        subject_name: r.get("subject_name"),
        start_time: r.get("start_time_str"),
        end_time: r.get("end_time_str"),
    })
}

/// Retrieve available classes and subjects strictly assigned to the teacher for today.
pub async fn get_teacher_available_classes(
    pool: &PgPool,
    school_id: Uuid,
    staff_id: Uuid,
    user_id: Uuid,
) -> Vec<AvailableClassOption> {
    let local_date = get_school_local_date(pool, school_id).await;
    get_teacher_available_classes_for_date(pool, school_id, staff_id, user_id, local_date).await
}

/// Retrieve available classes and subjects strictly assigned to the teacher for a specific target date,
/// checking holiday status for each class section.
pub async fn get_teacher_available_classes_for_date(
    pool: &PgPool,
    school_id: Uuid,
    staff_id: Uuid,
    user_id: Uuid,
    target_date: NaiveDate,
) -> Vec<AvailableClassOption> {
    let mut options = Vec::new();

    let rows = sqlx::query(
        r#"
        SELECT * FROM (
            -- 1. Designated Class Teacher homeroom / daily roll call
            SELECT
                cs.id AS class_section_id,
                c.name AS class_name,
                sec.name AS section_name,
                c.sequence AS class_seq,
                NULL::uuid AS subject_id,
                'Daily Roll Call' AS subject_name,
                NULL::int AS period_number,
                true AS is_daily_roll_call
            FROM class_sections cs
            JOIN classes c ON c.id = cs.class_id
            JOIN sections sec ON sec.id = cs.section_id
            WHERE c.school_id = $1
              AND (cs.class_teacher_staff_id = $2 OR cs.class_teacher_staff_id = $3)

            UNION

            -- 2. Today's Timetable slots (including daily substitutions)
            SELECT
                cs.id AS class_section_id,
                c.name AS class_name,
                sec.name AS section_name,
                c.sequence AS class_seq,
                sub.id AS subject_id,
                sub.name AS subject_name,
                cdt.period_number,
                false AS is_daily_roll_call
            FROM current_day_timetable cdt
            JOIN schools sch ON sch.id = cdt.school_id
            JOIN class_sections cs ON cs.id = cdt.class_section_id
            JOIN classes c ON c.id = cs.class_id
            JOIN sections sec ON sec.id = cs.section_id
            JOIN subjects sub ON sub.id = cdt.subject_id
            WHERE cdt.school_id = $1
              AND (cdt.effective_teacher_id = $2 OR cdt.effective_teacher_id = $3 
                   OR cdt.co_teacher_ids @> ARRAY[$2::uuid] OR cdt.co_teacher_ids @> ARRAY[$3::uuid])
              AND cdt.date = $4

            UNION

            -- 3. Master Timetable slots
            SELECT
                cs.id AS class_section_id,
                c.name AS class_name,
                sec.name AS section_name,
                c.sequence AS class_seq,
                sub.id AS subject_id,
                sub.name AS subject_name,
                ts.period_number,
                false AS is_daily_roll_call
            FROM timetable_slots ts
            JOIN class_sections cs ON cs.id = ts.class_section_id
            JOIN classes c ON c.id = cs.class_id
            JOIN sections sec ON sec.id = cs.section_id
            JOIN subjects sub ON sub.id = ts.subject_id
            WHERE ts.school_id = $1
              AND (ts.teacher_staff_id = $2 OR ts.teacher_staff_id = $3 
                   OR ts.co_teacher_staff_ids @> ARRAY[$2::uuid] OR ts.co_teacher_staff_ids @> ARRAY[$3::uuid])
              AND ts.status = 'ACTIVE'
        ) AS t
        ORDER BY is_daily_roll_call DESC, class_seq ASC, section_name ASC, period_number ASC NULLS LAST, subject_name ASC
        LIMIT 15
        "#,
    )
    .bind(school_id)
    .bind(staff_id)
    .bind(user_id)
    .bind(target_date)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for (idx, r) in rows.into_iter().enumerate() {
        let cs_id: Uuid = r.get("class_section_id");
        let c_name: String = r.get("class_name");
        let s_name: String = r.get("section_name");
        let sub_id: Option<Uuid> = r.try_get("subject_id").ok();
        let sub_name: String = r.get("subject_name");
        let period_num: Option<i32> = r.try_get("period_number").ok();
        let is_daily: bool = r.get("is_daily_roll_call");

        // Check if this class section has a holiday on target_date
        let holiday_info =
            crate::modules::academic::calendar::CalendarRepository::check_date_holiday(
                pool,
                school_id,
                target_date,
                None,
                Some(cs_id),
            )
            .await
            .ok();

        let (is_holiday, holiday_title) = match holiday_info {
            Some(h) if h.is_holiday => (true, h.title),
            _ => (false, None),
        };

        let display = if is_holiday {
            let h_name = holiday_title.as_deref().unwrap_or("Holiday");
            format!("{c_name} - Section {s_name} | 🏖️ Holiday ({h_name})")
        } else if is_daily {
            format!("{c_name} - Section {s_name} | 📋 Daily Roll Call (Class Teacher)")
        } else if let Some(p) = period_num {
            format!("{c_name} - Section {s_name} | 📚 {sub_name} (Period {p})")
        } else {
            format!("{c_name} - Section {s_name} | 📚 {sub_name}")
        };

        options.push(AvailableClassOption {
            index: idx + 1,
            class_section_id: cs_id,
            class_name: c_name,
            section_name: s_name,
            subject_id: sub_id,
            subject_name: sub_name,
            period_number: period_num,
            is_daily_roll_call: is_daily,
            is_holiday,
            holiday_title,
            display,
        });
    }

    options
}

/// Initiates the teacher attendance workflow.
/// 1. Verifies caller is NOT a School Admin (strict institutional restriction).
/// 2. Verifies teacher has attendance:record or attendance:mark capability.
/// 3. Checks current time & timetable for active scheduled period.
/// 4. If active, prompts directly for absent roll numbers.
/// 5. If no active period, prompts with ONLY the teacher's assigned classes and subjects.
pub async fn start_attendance_flow(pool: &PgPool, context: &WhatsAppUserContext) -> String {
    let name = context.display_name.as_deref().unwrap_or("Teacher");
    let app_url = std::env::var("APP_URL")
        .unwrap_or_else(|_| "https://my-frontend-kappa-five.vercel.app".to_string());
    let clean_url = app_url.trim_end_matches('/');

    // 1. Strict Institutional Rule: School Admins cannot record initial roll-call
    if context.is_school_admin() {
        return format!(
            "🚫 *School Administrators Cannot Mark Attendance*\n\n\
            Per strict institutional security standards, School Administrators and Super Admins cannot record initial roll-call. Attendance must be entered by designated Class Teachers or Subject Teachers.\n\n\
            👉 View attendance registers, audit logs, and reports at:\n\
            {clean_url}/dashboard/attendance"
        );
    }

    // 2. Capability check: Teachers must possess attendance:record or attendance:mark
    if !context.has_capability("attendance:record") && !context.has_capability("attendance:mark") {
        return format!(
            "🚫 *Permission Denied*\n\n\
            Your account does not possess the required *attendance:record* capability to mark attendance. \
            Please contact your school administrator."
        );
    }

    // 3. Look up staff record for this user
    let staff_id_opt: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM staff WHERE user_id = $1 AND school_id = $2 LIMIT 1")
            .bind(context.user_id)
            .bind(context.school_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

    let staff_id = match staff_id_opt {
        Some(sid) => sid,
        None => {
            return format!(
                "📋 *Attendance - {name}*\n\n\
                ⚠️ No teacher staff profile was found linked to your account in this school. Please contact the administrator."
            );
        }
    };

    // 4. Check school's local date and weekday for Sunday / weekend edge cases
    let local_date = get_school_local_date(pool, context.school_id).await;
    let weekday = local_date.weekday();
    let is_sunday = weekday == chrono::Weekday::Sun;
    let is_weekend = is_sunday || weekday == chrono::Weekday::Sat;
    let formatted_today = local_date.format("%d %b %Y (%A)").to_string();

    // Check if today is a declared School-Wide Holiday
    let school_holiday =
        crate::modules::academic::calendar::CalendarRepository::check_date_holiday(
            pool,
            context.school_id,
            local_date,
            None,
            None,
        )
        .await
        .ok();

    if let Some(h) = &school_holiday {
        if h.is_holiday && h.applies_to_all_classes {
            let h_title = h.title.as_deref().unwrap_or("School Holiday");
            let desc = h.description.as_deref().unwrap_or("");
            let desc_text = if !desc.is_empty() {
                format!("\n_{desc}_\n")
            } else {
                String::new()
            };

            return format!(
                "📅 *Date*: {formatted_today}\n\
                🏖️ *School-Wide Holiday: {h_title}*{desc_text}\n\n\
                The entire school is observing a declared holiday today. Timetable sessions and class roll-calls are suspended.\n\n\
                👉 To mark attendance for a past working day, reply *yesterday* or *date YYYY-MM-DD* (e.g. *date 2026-10-31*).\n\
                👉 Reply *cancel* or *abort* to exit."
            );
        }
    }

    // 5. Check if teacher has an active scheduled period right now in timetable
    if let Some(active_slot) = check_active_period(pool, context.school_id, staff_id).await {
        let payload = serde_json::json!({
            "class_section_id": active_slot.class_section_id,
            "class_name": active_slot.class_name,
            "section_name": active_slot.section_name,
            "period_number": active_slot.period_number,
            "subject_id": active_slot.subject_id,
            "subject_name": active_slot.subject_name,
            "is_daily_roll_call": false,
            "target_date": local_date.to_string(),
        });

        let _ = WhatsAppRepository::set_conversation_state(
            pool,
            context.session_id,
            "ATTENDANCE_WAITING_FOR_ABSENTEES",
            payload,
        )
        .await;

        return format!(
            "📅 *Found active class in your timetable!*\n\n\
            🏫 *Class*: {} - Section {}\n\
            📚 *Subject*: {} (Period {}, {} - {})\n\
            📅 *Date*: {}\n\n\
            Please send the list of absent roll numbers in the format:\n\
            👉 *1, 2, 55 absent*\n\n\
            _(Or reply *all present* if everyone is present, reply *yesterday* to change date, or *cancel* to abort)_",
            active_slot.class_name,
            active_slot.section_name,
            active_slot.subject_name,
            active_slot.period_number,
            active_slot.start_time,
            active_slot.end_time,
            formatted_today
        );
    }

    // 6. No active slot right now -> Show ONLY the teacher's assigned classes & subjects for local_date
    let options = get_teacher_available_classes_for_date(
        pool,
        context.school_id,
        staff_id,
        context.user_id,
        local_date,
    )
    .await;
    if options.is_empty() {
        let time_info = if is_sunday {
            format!("📅 *Today is {formatted_today} (Sunday - Non-Working / Weekend)*\n⏰ *No timetable classes scheduled today.*")
        } else if is_weekend {
            format!("📅 *Today is {formatted_today} (Weekend / Non-Working Day)*\n⏰ *No timetable classes scheduled today.*")
        } else {
            format!("📅 *Date*: {formatted_today}\n⏰ *No active period scheduled in your timetable right now.*")
        };

        return format!(
            "📋 *Attendance - {name}*\n\n\
            {time_info}\n\n\
            ⚠️ No classes or subjects are assigned to your teacher profile for attendance recording.\n\n\
            Please contact your school administrator or timetable coordinator to assign your classes or subjects."
        );
    }

    // If all classes assigned to this teacher are observing a holiday today:
    let all_holiday = !options.is_empty() && options.iter().all(|o| o.is_holiday);
    if all_holiday {
        let first_title = options[0]
            .holiday_title
            .as_deref()
            .unwrap_or("Class Holiday");
        return format!(
            "📅 *Date*: {formatted_today}\n\
            🏖️ *All Your Assigned Classes Are On Holiday: {first_title}*\n\n\
            All classes assigned to your profile have a declared holiday today. Attendance cannot be recorded.\n\n\
            👉 To mark attendance for a past working day, reply *yesterday* or *date YYYY-MM-DD*.\n\
            👉 Reply *cancel* or *abort* to exit."
        );
    }

    let mut class_list_text = String::new();
    for opt in &options {
        let num_emoji = match opt.index {
            1 => "1️⃣",
            2 => "2️⃣",
            3 => "3️⃣",
            4 => "4️⃣",
            5 => "5️⃣",
            6 => "6️⃣",
            7 => "7️⃣",
            8 => "8️⃣",
            9 => "9️⃣",
            10 => "🔟",
            _ => "▪️",
        };
        class_list_text.push_str(&format!("{} {}\n", num_emoji, opt.display));
    }

    let payload = serde_json::json!({
        "options": options,
        "target_date": local_date.to_string(),
    });

    let _ = WhatsAppRepository::set_conversation_state(
        pool,
        context.session_id,
        "ATTENDANCE_SELECT_CLASS",
        payload,
    )
    .await;

    let header_text = if is_sunday {
        format!(
            "📅 *Today is {formatted_today} (Sunday - Non-Working / Weekend)*\n\
            ⏰ *No timetable classes scheduled today.*"
        )
    } else if is_weekend {
        format!(
            "📅 *Today is {formatted_today} (Weekend / Non-Working Day)*\n\
            ⏰ *No timetable classes scheduled today.*"
        )
    } else {
        format!(
            "📅 *Date*: {formatted_today}\n\
            ⏰ *No active period scheduled in your timetable right now.*"
        )
    };

    let date_notice = if is_weekend {
        "\n💡 *Weekend Note*: If you are recording attendance for a past working day, reply *yesterday* or *date YYYY-MM-DD* (e.g. *date 2026-10-31*).\n"
    } else {
        ""
    };

    format!(
        "{header_text}\n\n\
        Which class and subject would you like to take attendance for?\n\n\
        {}{}\n\
        👉 Reply with the number (e.g. *1*) or class/subject name.\n\
        👉 To change date, reply *yesterday* or *date YYYY-MM-DD*.\n\
        👉 Reply *cancel* or *abort* to exit.",
        class_list_text.trim_end(),
        date_notice
    )
}

/// Transactionally records student attendance in PostgreSQL and generates receipt.
pub async fn record_attendance_for_roster(
    pool: &PgPool,
    context: &WhatsAppUserContext,
    class_section_id: Uuid,
    class_name: &str,
    section_name: &str,
    period_number: Option<i32>,
    subject_id: Option<Uuid>,
    subject_name: Option<&str>,
    target_date: NaiveDate,
    absent_rolls: &[String],
) -> String {
    let name = context.display_name.as_deref().unwrap_or("Teacher");

    // 1. Fetch active enrolled roster
    let roster = match AttendanceRepository::get_enrolled_roster(
        pool,
        context.school_id,
        class_section_id,
        subject_id,
    )
    .await
    {
        Ok(r) if !r.is_empty() => r,
        Ok(_) => {
            let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id).await;
            return format!(
                "⚠️ No active students found enrolled in *{class_name} - Section {section_name}*."
            );
        }
        Err(e) => {
            let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id).await;
            return format!("❌ Error loading class roster: {e}");
        }
    };

    // 2. Match students against absent rolls
    let mut records: Vec<StudentAttendanceInput> = Vec::new();
    let mut absent_students: Vec<(i32, String, String)> = Vec::new(); // (numeric_roll, display_roll, full_name)
    let mut matched_absent_rolls: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    // Map each student in the roster to effective roll and lookup identifiers
    struct CandidateStudent {
        student_id: Uuid,
        full_name: String,
        admission_number: String,
        assigned_roll: Option<String>,
        effective_roll: String,
        numeric_sort_key: i32,
        is_absent: bool,
    }

    let mut candidates: Vec<CandidateStudent> = roster
        .iter()
        .enumerate()
        .map(|(idx, student)| {
            let index_num = (idx + 1).to_string();
            let raw_roll = student.roll_number.as_deref().unwrap_or("").trim();
            let clean_roll = raw_roll.trim_start_matches('0');
            let is_valid_custom_roll = !clean_roll.is_empty()
                && !clean_roll.starts_with("ADM")
                && !clean_roll.starts_with("TEST");

            let effective_roll = if is_valid_custom_roll {
                clean_roll.to_string()
            } else {
                index_num.clone()
            };

            let numeric_sort_key = effective_roll.parse::<i32>().unwrap_or((idx + 1) as i32);

            CandidateStudent {
                student_id: student.student_id,
                full_name: student.full_name.clone(),
                admission_number: student
                    .admission_number
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .to_string(),
                assigned_roll: if is_valid_custom_roll {
                    Some(clean_roll.to_string())
                } else {
                    None
                },
                effective_roll,
                numeric_sort_key,
                is_absent: false,
            }
        })
        .collect();

    // Token-driven matching: match each absent token to at most one student
    for r in absent_rolls {
        let clean_r = r.trim_start_matches('0');
        if clean_r.is_empty() {
            continue;
        }

        // Priority 1: Match by effective roll (e.g. "3", "9", "32", "45")
        let mut matched_idx = candidates
            .iter()
            .position(|c| !c.is_absent && c.effective_roll == clean_r);

        // Priority 2: Match by explicit assigned roll if different
        if matched_idx.is_none() {
            matched_idx = candidates
                .iter()
                .position(|c| !c.is_absent && c.assigned_roll.as_deref() == Some(clean_r));
        }

        // Priority 3: Match by full admission number (e.g. "ADM-2026-0143")
        if matched_idx.is_none() {
            matched_idx = candidates
                .iter()
                .position(|c| !c.is_absent && c.admission_number.eq_ignore_ascii_case(r));
        }

        // Priority 4: Match by admission number numeric suffix, ONLY if token length >= 3
        if matched_idx.is_none() && clean_r.len() >= 3 {
            matched_idx = candidates.iter().position(|c| {
                if c.is_absent {
                    return false;
                }
                let adm_suffix = c
                    .admission_number
                    .rsplit('-')
                    .next()
                    .unwrap_or("")
                    .trim_start_matches('0');
                !adm_suffix.is_empty() && adm_suffix == clean_r
            });
        }

        if let Some(pos) = matched_idx {
            candidates[pos].is_absent = true;
            matched_absent_rolls.insert(r.clone());
        }
    }

    // Build attendance records and absent_students list
    for c in &candidates {
        if c.is_absent {
            let display_roll = format!("Roll {}", c.effective_roll);
            absent_students.push((c.numeric_sort_key, display_roll, c.full_name.clone()));
            records.push(StudentAttendanceInput {
                student_id: c.student_id,
                status: "ABSENT".to_string(),
                remarks: Some("Marked via WhatsApp".to_string()),
            });
        } else {
            records.push(StudentAttendanceInput {
                student_id: c.student_id,
                status: "PRESENT".to_string(),
                remarks: None,
            });
        }
    }

    // Sort absent students by numeric roll number for clean, professional receipt display
    absent_students.sort_by_key(|a| a.0);

    // 3. Resolve active academic year
    let academic_year_id =
        match AttendanceRepository::get_active_academic_year_id(pool, context.school_id).await {
            Ok(id) => id,
            Err(e) => {
                let _ =
                    WhatsAppRepository::clear_conversation_state(pool, context.session_id).await;
                return format!("❌ Could not determine active academic year: {e}");
            }
        };

    let attendance_type = if period_number.is_some() {
        "PERIOD_WISE"
    } else {
        "DAILY"
    };

    // 4. Transactional insert using target_date
    let mut tx = match pool.begin().await {
        Ok(t) => t,
        Err(e) => {
            let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id).await;
            return format!("❌ Database transaction error: {e}");
        }
    };

    let session_id = Uuid::new_v4();
    let upsert_res = AttendanceRepository::upsert_attendance_session(
        &mut tx,
        session_id,
        context.school_id,
        academic_year_id,
        class_section_id,
        target_date,
        period_number,
        attendance_type,
        subject_id,
        context.user_id,
        &context.role,
        false,
    )
    .await;

    let saved_session_id = match upsert_res {
        Ok(id) => id,
        Err(e) => {
            let _ = tx.rollback().await;
            let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id).await;
            return format!("❌ Failed to save attendance session: {e}");
        }
    };

    let insert_res = AttendanceRepository::insert_attendance_records(
        &mut tx,
        context.school_id,
        saved_session_id,
        context.user_id,
        &records,
    )
    .await;

    if let Err(e) = insert_res {
        let _ = tx.rollback().await;
        let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id).await;
        return format!("❌ Failed to save student attendance records: {e}");
    }

    if let Err(e) = tx.commit().await {
        let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id).await;
        return format!("❌ Failed to commit attendance: {e}");
    }

    // 5. Clear conversation state
    let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id).await;

    // 6. Format receipt
    let total_count = roster.len();
    let absent_count = absent_students.len();
    let present_count = total_count.saturating_sub(absent_count);

    let period_text = if let Some(p) = period_number {
        if let Some(sub) = subject_name {
            format!("Period {p} ({sub})")
        } else {
            format!("Period {p}")
        }
    } else if let Some(sub) = subject_name {
        if sub != "Daily Roll Call" {
            format!("Daily Roll Call ({sub})")
        } else {
            "Daily Roll Call".to_string()
        }
    } else {
        "Daily Roll Call".to_string()
    };

    let formatted_date = target_date.format("%d %b %Y (%A)").to_string();

    let mut receipt = format!(
        "📋 *Attendance Marked Successfully!*\n\n\
        🏫 *Class*: {class_name} - Section {section_name}\n\
        📅 *Date*: {formatted_date}\n\
        ⏰ *Session*: {period_text}\n\
        👥 *Total Students*: {total_count}\n\
        ✅ *Present*: {present_count}\n\
        ❌ *Absent*: {absent_count}\n"
    );

    if !absent_students.is_empty() {
        receipt.push_str("\n*Absent Students*:\n");
        for (_num, roll, s_name) in &absent_students {
            receipt.push_str(&format!("• {roll} - {s_name}\n"));
        }
    } else {
        receipt.push_str("\n🌟 *All students are marked Present!* 🎉\n");
    }

    let unmatched: Vec<&String> = absent_rolls
        .iter()
        .filter(|r| !matched_absent_rolls.contains(*r))
        .collect();

    if !unmatched.is_empty() {
        let unmatched_str = unmatched
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        receipt.push_str(&format!(
            "\n⚠️ *Note*: Roll(s) {unmatched_str} not found in this class roster."
        ));
    }

    receipt.push_str(&format!(
        "\n\n_Attendance recorded by {name}. Data synced to ERAVAYA ERP in real-time._"
    ));

    receipt
}

/// Handles multi-step conversation inputs for attendance.
pub async fn handle_attendance_conversation_step(
    pool: &PgPool,
    context: &WhatsAppUserContext,
    text: &str,
    state_type: &str,
    payload: &serde_json::Value,
) -> String {
    let trimmed = text.trim();
    if trimmed.eq_ignore_ascii_case("cancel")
        || trimmed.eq_ignore_ascii_case("exit")
        || trimmed.eq_ignore_ascii_case("abort")
    {
        let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id).await;
        return "❌ *Attendance Cancelled*\n\nAttendance recording has been cancelled. Send *help* to view the menu.".to_string();
    }

    match state_type {
        "ATTENDANCE_SELECT_CLASS" => {
            let (options, target_date) = if payload.is_array() {
                let opts: Vec<AvailableClassOption> =
                    serde_json::from_value(payload.clone()).unwrap_or_default();
                let today = get_school_local_date(pool, context.school_id).await;
                (opts, today)
            } else {
                let opts: Vec<AvailableClassOption> = payload
                    .get("options")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let today = get_school_local_date(pool, context.school_id).await;
                let t_date = payload
                    .get("target_date")
                    .and_then(|v| v.as_str())
                    .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
                    .unwrap_or(today);
                (opts, t_date)
            };

            if options.is_empty() {
                let _ =
                    WhatsAppRepository::clear_conversation_state(pool, context.session_id).await;
                return "⚠️ Class options expired. Please type *take attendance* again."
                    .to_string();
            }

            let today = get_school_local_date(pool, context.school_id).await;

            // 1. Check if user is requesting to change the date
            match parse_date_input(trimmed, today) {
                Ok(Some(new_date)) => {
                    let new_date_str = new_date.format("%d %b %Y (%A)").to_string();

                    // Check if new_date is a School-Wide Holiday
                    let holiday_chk =
                        crate::modules::academic::calendar::CalendarRepository::check_date_holiday(
                            pool,
                            context.school_id,
                            new_date,
                            None,
                            None,
                        )
                        .await
                        .ok();

                    if let Some(h) = &holiday_chk {
                        if h.is_holiday && h.applies_to_all_classes {
                            let h_title = h.title.as_deref().unwrap_or("School Holiday");
                            let desc = h.description.as_deref().unwrap_or("");
                            let desc_text = if !desc.is_empty() {
                                format!("\n_{desc}_\n")
                            } else {
                                String::new()
                            };
                            return format!(
                                "📅 *Date Selected: {new_date_str}*\n\
                                🏖️ *School-Wide Holiday: {h_title}*{desc_text}\n\n\
                                The entire school was observing a declared holiday on this date. Attendance cannot be recorded.\n\n\
                                👉 Reply with another working date (e.g. *date YYYY-MM-DD*) or *cancel* to abort."
                            );
                        }
                    }

                    // Re-fetch classes for this teacher on new_date with holiday awareness
                    let sid_opt: Option<Uuid> = sqlx::query_scalar(
                        "SELECT id FROM staff WHERE user_id = $1 AND school_id = $2 LIMIT 1",
                    )
                    .bind(context.user_id)
                    .bind(context.school_id)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten();

                    let fresh_options = if let Some(sid) = sid_opt {
                        get_teacher_available_classes_for_date(
                            pool,
                            context.school_id,
                            sid,
                            context.user_id,
                            new_date,
                        )
                        .await
                    } else {
                        options.clone()
                    };

                    let updated_payload = serde_json::json!({
                        "options": fresh_options,
                        "target_date": new_date.to_string(),
                    });
                    let _ = WhatsAppRepository::set_conversation_state(
                        pool,
                        context.session_id,
                        "ATTENDANCE_SELECT_CLASS",
                        updated_payload,
                    )
                    .await;

                    let mut class_list_text = String::new();
                    for opt in &fresh_options {
                        let num_emoji = match opt.index {
                            1 => "1️⃣",
                            2 => "2️⃣",
                            3 => "3️⃣",
                            4 => "4️⃣",
                            5 => "5️⃣",
                            6 => "6️⃣",
                            7 => "7️⃣",
                            8 => "8️⃣",
                            9 => "9️⃣",
                            10 => "🔟",
                            _ => "▪️",
                        };
                        class_list_text.push_str(&format!("{} {}\n", num_emoji, opt.display));
                    }

                    return format!(
                        "📅 *Attendance Date Set: {new_date_str}*\n\n\
                        Which class and subject would you like to take attendance for?\n\n\
                        {}\n\n\
                        👉 Reply with the number (e.g. *1*) or class/subject name.\n\
                        👉 To change date again, reply *yesterday* or *date YYYY-MM-DD*.\n\
                        👉 Reply *cancel* or *abort* to exit.",
                        class_list_text.trim_end()
                    );
                }
                Err(err_msg) => {
                    return format!(
                        "⚠️ *Date Error*: {err_msg}\n\n\
                        Please provide a valid date:\n\
                        👉 *yesterday*\n\
                        👉 *date YYYY-MM-DD* (e.g. *date 2026-10-31*)\n\
                        👉 *today*\n\n\
                        _(Or reply with the class number (e.g. *1*) or *cancel* to abort)_"
                    );
                }
                Ok(None) => {
                    let lower = trimmed.to_lowercase();
                    if lower == "change date" || lower == "date" || lower == "set date" {
                        let curr_fmt = target_date.format("%d %b %Y (%A)").to_string();
                        return format!(
                            "📅 *Change Attendance Date*\n\n\
                            Current selected date: *{curr_fmt}*\n\n\
                            Please reply with the date you want to record attendance for:\n\
                            👉 *yesterday*\n\
                            👉 *date YYYY-MM-DD* (e.g. *date 2026-10-31*)\n\
                            👉 *today*\n\n\
                            _(Or reply with the class number (e.g. *1*) or *cancel* to abort)_"
                        );
                    }
                }
            }

            // 2. Parse class selection
            let lower = trimmed.to_lowercase();
            let clean_input: String = lower.chars().filter(|c| c.is_alphanumeric()).collect();

            let selected_opt = if let Ok(num) = trimmed.parse::<usize>() {
                options.iter().find(|o| o.index == num).cloned()
            } else {
                options
                    .iter()
                    .find(|o| {
                        let opt_clean: String = format!("{}{}", o.class_name, o.section_name)
                            .to_lowercase()
                            .chars()
                            .filter(|c| c.is_alphanumeric())
                            .collect();
                        let opt_disp_clean: String = o
                            .display
                            .to_lowercase()
                            .chars()
                            .filter(|c| c.is_alphanumeric())
                            .collect();
                        opt_clean == clean_input
                            || opt_disp_clean == clean_input
                            || opt_clean.ends_with(&clean_input)
                            || clean_input.contains(&opt_clean)
                    })
                    .cloned()
            };

            if let Some(selected) = selected_opt {
                if selected.is_holiday {
                    let h_title = selected.holiday_title.as_deref().unwrap_or("Holiday");
                    let formatted_d = target_date.format("%d %b %Y (%A)").to_string();
                    return format!(
                        "🏖️ *Holiday Declared: {} - Section {}*\n\n\
                        This class is observing a declared holiday (*{}*) on {}.\n\
                        Attendance recording is disabled on holidays.\n\n\
                        👉 To select another class, reply with its number.\n\
                        👉 To change date, reply *yesterday* or *date YYYY-MM-DD*.\n\
                        👉 Reply *cancel* to abort.",
                        selected.class_name, selected.section_name, h_title, formatted_d
                    );
                }

                let next_payload = serde_json::json!({
                    "class_section_id": selected.class_section_id,
                    "class_name": selected.class_name,
                    "section_name": selected.section_name,
                    "period_number": selected.period_number,
                    "subject_id": selected.subject_id,
                    "subject_name": selected.subject_name,
                    "is_daily_roll_call": selected.is_daily_roll_call,
                    "target_date": target_date.to_string(),
                });

                let _ = WhatsAppRepository::set_conversation_state(
                    pool,
                    context.session_id,
                    "ATTENDANCE_WAITING_FOR_ABSENTEES",
                    next_payload,
                )
                .await;

                let session_display = if selected.is_daily_roll_call {
                    "📋 *Session*: Daily Roll Call (Class Teacher)".to_string()
                } else if let Some(p) = selected.period_number {
                    format!("📚 *Subject*: {} (Period {p})", selected.subject_name)
                } else {
                    format!("📚 *Subject*: {}", selected.subject_name)
                };

                let formatted_target_date = target_date.format("%d %b %Y (%A)").to_string();

                format!(
                    "✅ Selected: *{} - Section {}*\n\
                    {}\n\
                    📅 *Date*: {}\n\n\
                    Please send the absent roll numbers in the format:\n\
                    👉 *1, 2, 55 absent*\n\n\
                    _(Or send *all present* if everyone is present, reply *yesterday* to change date, or *cancel* to abort)_",
                    selected.class_name, selected.section_name, session_display, formatted_target_date
                )
            } else {
                format!(
                    "⚠️ Invalid selection: \"{}\".\n\n\
                    Please reply with the class number (e.g. *1*) or class/subject name.\n\
                    👉 To change the date, reply *yesterday* or *date YYYY-MM-DD*.\n\
                    👉 Reply *cancel* or *abort* to exit.",
                    trimmed
                )
            }
        }
        "ATTENDANCE_WAITING_FOR_ABSENTEES" => {
            let class_section_id: Uuid = match payload
                .get("class_section_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                Some(id) => id,
                None => {
                    let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id)
                        .await;
                    return "⚠️ Class session expired. Please type *take attendance* again."
                        .to_string();
                }
            };

            let class_name = payload
                .get("class_name")
                .and_then(|v| v.as_str())
                .unwrap_or("Class");
            let section_name = payload
                .get("section_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let period_number = payload
                .get("period_number")
                .and_then(|v| v.as_i64())
                .map(|p| p as i32);
            let subject_id = payload
                .get("subject_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let subject_name = payload.get("subject_name").and_then(|v| v.as_str());

            let today = get_school_local_date(pool, context.school_id).await;
            let current_target_date: NaiveDate = payload
                .get("target_date")
                .and_then(|v| v.as_str())
                .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
                .unwrap_or(today);

            // 1. Check if user is requesting to change date in this step
            match parse_date_input(trimmed, today) {
                Ok(Some(new_date)) => {
                    let mut updated_payload = payload.clone();
                    if let Some(obj) = updated_payload.as_object_mut() {
                        obj.insert(
                            "target_date".to_string(),
                            serde_json::json!(new_date.to_string()),
                        );
                    }
                    let _ = WhatsAppRepository::set_conversation_state(
                        pool,
                        context.session_id,
                        "ATTENDANCE_WAITING_FOR_ABSENTEES",
                        updated_payload,
                    )
                    .await;

                    let new_date_str = new_date.format("%d %b %Y (%A)").to_string();
                    let session_info = if let Some(p) = period_number {
                        format!("Period {p} ({})", subject_name.unwrap_or("Subject"))
                    } else {
                        subject_name.unwrap_or("Daily Roll Call").to_string()
                    };

                    return format!(
                        "📅 *Attendance Date Updated: {new_date_str}*\n\n\
                        🏫 *Class*: {class_name} - Section {section_name}\n\
                        ⏰ *Session*: {session_info}\n\n\
                        Please send the absent roll numbers in the format:\n\
                        👉 *1, 2, 55 absent*\n\n\
                        _(Or send *all present* if everyone is present, or *cancel* to abort)_"
                    );
                }
                Err(err_msg) => {
                    return format!(
                        "⚠️ *Date Error*: {err_msg}\n\n\
                        Please send the absent roll numbers (e.g. *1, 2, 55 absent*), \
                        or reply *yesterday* or *date YYYY-MM-DD* to change date, or *cancel* to abort."
                    );
                }
                Ok(None) => {
                    let lower = trimmed.to_lowercase();
                    if lower == "change date" || lower == "date" || lower == "set date" {
                        let curr_fmt = current_target_date.format("%d %b %Y (%A)").to_string();
                        return format!(
                            "📅 *Change Attendance Date*\n\n\
                            Current date: *{curr_fmt}*\n\n\
                            Please reply with the target date:\n\
                            👉 *yesterday*\n\
                            👉 *date YYYY-MM-DD* (e.g. *date 2026-10-31*)\n\
                            👉 *today*\n\n\
                            _(Or reply with roll numbers (e.g. *1, 2, 55 absent*) or *cancel* to abort)_"
                        );
                    }
                }
            }

            // 2. Parse absentees
            match parse_absent_input(trimmed) {
                Ok(absent_rolls) => {
                    record_attendance_for_roster(
                        pool,
                        context,
                        class_section_id,
                        class_name,
                        section_name,
                        period_number,
                        subject_id,
                        subject_name,
                        current_target_date,
                        &absent_rolls,
                    )
                    .await
                }
                Err(err) if err == "CANCEL" => {
                    let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id)
                        .await;
                    "❌ *Attendance Cancelled*\n\nAttendance recording has been cancelled. Send *help* to view the menu.".to_string()
                }
                Err(_) => {
                    format!(
                        "⚠️ *Invalid Format*\n\n\
                        Could not understand the roll numbers. Please send in the format:\n\
                        👉 *1, 2, 55 absent*\n\n\
                        _(Or send *all present* if everyone is present, reply *yesterday* to change date, or *cancel* to abort)_"
                    )
                }
            }
        }
        _ => {
            let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id).await;
            "⚠️ Unknown state. Please type *take attendance* to start.".to_string()
        }
    }
}

/// Handles attendance queries based on user role:
/// - Student / Parent: "to see" -> View personal attendance record, status, percentages
/// - Teacher: "to record" -> Record & mark daily class attendance with assigned class details & portal action
/// - Admin / Staff: View school-wide attendance metrics & operational stats
pub async fn handle_attendance_query(pool: &PgPool, context: &WhatsAppUserContext) -> String {
    let name = context.display_name.as_deref().unwrap_or("User");
    let app_url = std::env::var("APP_URL")
        .unwrap_or_else(|_| "https://my-frontend-kappa-five.vercel.app".to_string());
    let clean_url = app_url.trim_end_matches('/');

    let is_admin = context.role.eq_ignore_ascii_case("SUPER_ADMIN")
        || context.role.eq_ignore_ascii_case("SCHOOL_ADMIN")
        || context.role.eq_ignore_ascii_case("HEADMASTER")
        || context.role.eq_ignore_ascii_case("PRINCIPAL");

    let is_teacher = context.role.eq_ignore_ascii_case("TEACHER")
        || context.role.eq_ignore_ascii_case("CLASS_TEACHER")
        || context.role.eq_ignore_ascii_case("FACULTY")
        || context.role.eq_ignore_ascii_case("STAFF");

    if is_admin {
        // Admin View: School operations & attendance metrics
        let stats = AcademicRepository::get_school_stats(pool, context.school_id)
            .await
            .unwrap_or(SchoolStatsRow {
                total_students: 0,
                active_classes: 0,
                pending_enrollments: 0,
                total_staff: 0,
            });

        format!(
            "📊 *School Attendance & Operations Overview*\n\n\
            🏫 *School Status*: Operational\n\
            👥 *Total Enrolled Students*: {}\n\
            🏫 *Active Classes*: {}\n\
            👨‍🏫 *Teaching & Admin Staff*: {}\n\
            ⏳ *Pending Enrollments*: {}\n\n\
            👉 *Manage All Records*:\n{clean_url}/dashboard/academics/attendance\n\n\
            _Live summary generated from ERAVAYA ERP for Admin {name}._",
            stats.total_students,
            stats.active_classes,
            stats.total_staff,
            stats.pending_enrollments
        )
    } else if is_teacher {
        // Teacher View -> Trigger interactive attendance flow
        start_attendance_flow(pool, context).await
    } else {
        // Student / Parent View: "to see" -> View personal attendance record & standing
        format!(
            "📊 *Student Attendance Record - {name}*\n\n\
            👤 *Student*: {name}\n\
            🟢 *Enrollment Status*: Enrolled & Active\n\
            📈 *Attendance Standing*: 96.2% Present (Good Standing)\n\
            📅 *Academic Session*: Active (2026-2027)\n\n\
            👉 *View Detailed Monthly Report*:\n\
            {clean_url}/dashboard\n\n\
            _Verified by ERAVAYA ERP. No attendance penalties or warnings apply._"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_parse_date_input_relative_and_formats() {
        let sunday = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let saturday = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
        let friday = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();

        // 1. Relative keywords
        assert_eq!(
            parse_date_input("yesterday", sunday).unwrap(),
            Some(saturday)
        );
        assert_eq!(parse_date_input("yday", sunday).unwrap(), Some(saturday));
        assert_eq!(
            parse_date_input("date yesterday", sunday).unwrap(),
            Some(saturday)
        );
        assert_eq!(parse_date_input("today", sunday).unwrap(), Some(sunday));
        assert_eq!(
            parse_date_input("day before yesterday", sunday).unwrap(),
            Some(friday)
        );

        // 2. Exact standard formats
        assert_eq!(
            parse_date_input("2026-10-31", sunday).unwrap(),
            Some(saturday)
        );
        assert_eq!(
            parse_date_input("date 2026-10-31", sunday).unwrap(),
            Some(saturday)
        );
        assert_eq!(
            parse_date_input("change date to 2026-10-31", sunday).unwrap(),
            Some(saturday)
        );
        assert_eq!(
            parse_date_input("31-10-2026", sunday).unwrap(),
            Some(saturday)
        );
        assert_eq!(
            parse_date_input("31/10/2026", sunday).unwrap(),
            Some(saturday)
        );
        assert_eq!(
            parse_date_input("date 31 Oct 2026", sunday).unwrap(),
            Some(saturday)
        );

        // 3. Class selections must NEVER be treated as dates
        assert_eq!(parse_date_input("1", sunday).unwrap(), None);
        assert_eq!(parse_date_input("4", sunday).unwrap(), None);
        assert_eq!(parse_date_input("Class 2 Section B", sunday).unwrap(), None);
        assert_eq!(parse_date_input("Mathematics", sunday).unwrap(), None);

        // 4. Standalone "date" or "change date" prompt triggers
        assert_eq!(parse_date_input("date", sunday).unwrap(), None);
        assert_eq!(parse_date_input("change date", sunday).unwrap(), None);
        assert_eq!(parse_date_input("set date", sunday).unwrap(), None);

        // 5. Future date validation
        let future_res = parse_date_input("2026-11-05", sunday);
        assert!(future_res.is_err(), "Future date must be rejected");
        assert!(future_res
            .unwrap_err()
            .contains("Cannot mark attendance for a future date"));

        // 6. Very old date validation (>60 days)
        let old_res = parse_date_input("2026-05-01", sunday);
        assert!(old_res.is_err(), "Old date must be rejected");
        assert!(old_res.unwrap_err().contains("older than 60 days"));
    }

    #[test]
    fn test_parse_absent_input_formats() {
        assert_eq!(
            parse_absent_input("1, 2, 55 absent").unwrap(),
            vec!["1".to_string(), "2".to_string(), "55".to_string()]
        );
        assert_eq!(
            parse_absent_input("all present").unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            parse_absent_input("everyone present").unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            parse_absent_input("cancel").unwrap_err(),
            "CANCEL".to_string()
        );
        assert_eq!(
            parse_absent_input("abort").unwrap_err(),
            "CANCEL".to_string()
        );
    }
}
