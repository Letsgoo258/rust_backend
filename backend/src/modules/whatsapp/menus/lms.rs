use crate::modules::auth::backend::User;
use crate::modules::lms::application::LmsService;
use crate::modules::lms::dto::*;
use crate::modules::whatsapp::repository::WhatsAppRepository;
use crate::modules::whatsapp::service::WhatsAppUserContext;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Detects if the incoming message is an explicit request to create/assign homework or LMS tasks.
pub fn is_lms_task_command(text: &str) -> bool {
    let clean = text
        .trim()
        .trim_start_matches(|c: char| c == ':' || c == '/' || c == '!' || c == '.')
        .trim()
        .to_lowercase();

    clean.starts_with("assign homework")
        || clean.starts_with("assign task")
        || clean.starts_with("assign assignment")
        || clean.starts_with("create homework")
        || clean.starts_with("create task")
        || clean.starts_with("create assignment")
        || clean.starts_with("give homework")
        || clean == "assign"
        || clean == "homework"
        || clean == "lms"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LmsConversationDraft {
    pub step: String,
    pub class_section_id: Option<Uuid>,
    pub class_name: Option<String>,
    pub section_name: Option<String>,
    pub subject_id: Option<Uuid>,
    pub subject_name: Option<String>,
    pub attendance_session_id: Option<Uuid>,
    pub attendance_date: Option<String>,
    pub attendance_period: Option<i32>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub special_message: Option<String>,
    pub deadline_at: Option<String>,
    pub is_evaluation: bool,
    pub max_marks: Option<f64>,
    pub include_in_results: bool,
    pub attachment_url: Option<String>,
    pub is_gdrive: bool,
    pub notify_whatsapp: bool,
    pub notify_email: bool,
}

impl Default for LmsConversationDraft {
    fn default() -> Self {
        Self {
            step: "WAITING_CLASS".into(),
            class_section_id: None,
            class_name: None,
            section_name: None,
            subject_id: None,
            subject_name: None,
            attendance_session_id: None,
            attendance_date: None,
            attendance_period: None,
            title: None,
            description: None,
            special_message: None,
            deadline_at: None,
            is_evaluation: false,
            max_marks: None,
            include_in_results: false,
            attachment_url: None,
            is_gdrive: false,
            notify_whatsapp: true,
            notify_email: false,
        }
    }
}

/// Start LMS conversational task creation flow
pub async fn start_lms_flow(
    pool: &PgPool,
    context: &WhatsAppUserContext,
    _incoming_text: &str,
) -> String {
    // Check if teacher has teaching capabilities
    if !context.is_teacher() && !context.is_school_admin() {
        return "🚫 *Permission Denied*\n\nCreating academic tasks is reserved for teachers and academic coordinators.".into();
    }

    // Fetch classes taught by this teacher
    let classes = fetch_teacher_classes(pool, context.school_id, context.user_id).await;
    if classes.is_empty() {
        return "ℹ️ *No classes found assigned to your profile.*\n\nPlease ask your school administrator to configure your teaching assignments.".into();
    }

    let mut draft = LmsConversationDraft::default();
    draft.step = "WAITING_CLASS".into();

    let mut msg = "📚 *Create LMS Academic Task*\n\nWhich class and section would you like to assign this task to?\n\n".to_string();
    for (idx, c) in classes.iter().enumerate() {
        msg.push_str(&format!(
            "{}. *{} {}*\n",
            idx + 1,
            c.class_name,
            c.section_name
        ));
    }
    msg.push_str("\n_Reply with the number or class name (e.g., '1' or 'Class 8 A'). Type 'cancel' to exit._");

    let payload = serde_json::to_value(&draft).unwrap_or_default();
    let _ =
        WhatsAppRepository::set_conversation_state(pool, context.session_id, "LMS_FLOW", payload)
            .await;

    msg
}

/// Handle multi-step LMS task creation flow
pub async fn handle_lms_conversation_step(
    pool: &PgPool,
    context: &WhatsAppUserContext,
    incoming_text: &str,
    payload: &serde_json::Value,
) -> String {
    let trimmed = incoming_text.trim();
    if trimmed.eq_ignore_ascii_case("cancel") {
        let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id).await;
        return "❌ *LMS Task Creation Cancelled*\n\nTask draft has been discarded. Send *help* to view the main menu.".into();
    }

    let mut draft: LmsConversationDraft =
        serde_json::from_value(payload.clone()).unwrap_or_default();

    match draft.step.as_str() {
        "WAITING_CLASS" => {
            let classes = fetch_teacher_classes(pool, context.school_id, context.user_id).await;
            let selected = if let Ok(idx) = trimmed.parse::<usize>() {
                if idx >= 1 && idx <= classes.len() {
                    Some(&classes[idx - 1])
                } else {
                    None
                }
            } else {
                classes.iter().find(|c| {
                    let full = format!("{} {}", c.class_name, c.section_name).to_lowercase();
                    full.contains(&trimmed.to_lowercase())
                        || (c
                            .class_name
                            .to_lowercase()
                            .contains(&trimmed.to_lowercase())
                            && c.section_name.to_lowercase() == trimmed.to_lowercase())
                })
            };

            let cs = match selected {
                Some(c) => c,
                None => {
                    return "⚠️ Could not match the class. Please select a valid number from the list above, or reply 'cancel'.".into();
                }
            };

            draft.class_section_id = Some(cs.class_section_id);
            draft.class_name = Some(cs.class_name.clone());
            draft.section_name = Some(cs.section_name.clone());

            // Fetch subjects for this class
            let subjects = fetch_class_subjects(pool, context.school_id, cs.class_section_id).await;
            if subjects.len() == 1 {
                draft.subject_id = Some(subjects[0].0);
                draft.subject_name = Some(subjects[0].1.clone());
                return advance_to_attendance_session(pool, context, &mut draft).await;
            } else if subjects.is_empty() {
                draft.subject_id = None;
                draft.subject_name = Some("General".into());
                return advance_to_attendance_session(pool, context, &mut draft).await;
            }

            draft.step = "WAITING_SUBJECT".into();
            save_draft(pool, context, &draft).await;

            let mut msg = format!(
                "Selected: *{} {}*\n\nWhich subject is this task for?\n\n",
                cs.class_name, cs.section_name
            );
            for (idx, (_, s_name)) in subjects.iter().enumerate() {
                msg.push_str(&format!("{}. *{}*\n", idx + 1, s_name));
            }
            msg.push_str("\n_Reply with the number or subject name._");
            msg
        }

        "WAITING_SUBJECT" => {
            let cs_id = draft.class_section_id.unwrap();
            let subjects = fetch_class_subjects(pool, context.school_id, cs_id).await;
            let selected = if let Ok(idx) = trimmed.parse::<usize>() {
                if idx >= 1 && idx <= subjects.len() {
                    Some(&subjects[idx - 1])
                } else {
                    None
                }
            } else {
                subjects
                    .iter()
                    .find(|(_, name)| name.to_lowercase().contains(&trimmed.to_lowercase()))
            };

            match selected {
                Some((id, name)) => {
                    draft.subject_id = Some(*id);
                    draft.subject_name = Some(name.clone());
                    advance_to_attendance_session(pool, context, &mut draft).await
                }
                None => "⚠️ Subject not recognized. Please reply with a valid number from the list, or 'cancel'.".into(),
            }
        }

        "WAITING_ATTENDANCE_CONFIRM" => {
            let clean = trimmed.to_lowercase();
            if clean == "yes" || clean == "y" || clean == "1" || clean == "confirm" {
                draft.step = "WAITING_TITLE".into();
                save_draft(pool, context, &draft).await;
                "👍 Attendance session linked!\n\n📝 *What is the title of this task?*\n_e.g., Mathematics Worksheet 4 or Chapter 3 Exercises_".into()
            } else if clean == "no" || clean == "n" || clean == "2" {
                // Fetch other recent attendance sessions
                let sessions = fetch_recent_sessions(
                    pool,
                    context.school_id,
                    draft.class_section_id.unwrap(),
                    draft.subject_id,
                )
                .await;

                if sessions.is_empty() {
                    let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id)
                        .await;
                    return "⚠️ No other attendance sessions found for this class. Please take attendance first in the Attendance menu before creating tasks.".into();
                }

                draft.step = "WAITING_ATTENDANCE_SELECT".into();
                save_draft(pool, context, &draft).await;

                let mut msg = "Please select the attendance session to link:\n\n".to_string();
                for (idx, sess) in sessions.iter().enumerate() {
                    msg.push_str(&format!(
                        "{}. *{}* (Period: {})\n",
                        idx + 1,
                        sess.date.format("%d %b %Y"),
                        sess.period_number
                            .map(|p| p.to_string())
                            .unwrap_or_else(|| "Daily".into())
                    ));
                }
                msg.push_str("\n_Reply with the session number._");
                msg
            } else {
                "Please reply with *Yes* to link this session, or *No* to choose another session."
                    .into()
            }
        }

        "WAITING_ATTENDANCE_SELECT" => {
            let sessions = fetch_recent_sessions(
                pool,
                context.school_id,
                draft.class_section_id.unwrap(),
                draft.subject_id,
            )
            .await;

            let selected = if let Ok(idx) = trimmed.parse::<usize>() {
                if idx >= 1 && idx <= sessions.len() {
                    Some(&sessions[idx - 1])
                } else {
                    None
                }
            } else {
                None
            };

            match selected {
                Some(s) => {
                    draft.attendance_session_id = Some(s.id);
                    draft.attendance_date = Some(s.date.to_string());
                    draft.attendance_period = s.period_number;
                    draft.step = "WAITING_TITLE".into();
                    save_draft(pool, context, &draft).await;
                    format!(
                        "👍 Linked session: *{}*!\n\n📝 *What is the title of this task?*\n_e.g., 'Chapter 4 Review Questions'_",
                        s.date.format("%d %b %Y")
                    )
                }
                None => {
                    "⚠️ Invalid choice. Please reply with a valid number from the session list."
                        .into()
                }
            }
        }

        "WAITING_TITLE" => {
            if trimmed.len() < 3 {
                return "Please provide a descriptive title (at least 3 characters).".into();
            }
            draft.title = Some(trimmed.to_string());
            draft.step = "WAITING_DESCRIPTION".into();
            save_draft(pool, context, &draft).await;

            "📄 *Task Description / Questions*\n\nPlease paste or type the task instructions or questions:\n_e.g., 'Complete questions 1 to 5 from page 42 of the textbook. Show all working clearly.'_".into()
        }

        "WAITING_DESCRIPTION" => {
            if trimmed.len() < 5 {
                return "Please provide clear task instructions or questions.".into();
            }
            draft.description = Some(trimmed.to_string());
            draft.step = "WAITING_SPECIAL_MESSAGE".into();
            save_draft(pool, context, &draft).await;

            "📌 *Special Message / Quick Note (Optional)*\n\nAny short reminder for students or parents? (e.g., 'Bring notebook tomorrow' or 'Submit handwritten')\n\n_Reply with your message, or reply 'skip'._".into()
        }

        "WAITING_SPECIAL_MESSAGE" => {
            if !trimmed.eq_ignore_ascii_case("skip") && !trimmed.eq_ignore_ascii_case("none") {
                draft.special_message = Some(trimmed.to_string());
            }
            draft.step = "WAITING_DEADLINE".into();
            save_draft(pool, context, &draft).await;

            "⏰ *Task Deadline*\n\nWhen is this task due?\n1. *Tomorrow*\n2. *Day after tomorrow*\n3. *Next week (7 days)*\n4. *No deadline*\n\n_Or reply with a specific date in YYYY-MM-DD format._".into()
        }

        "WAITING_DEADLINE" => {
            let now = Utc::now();
            let deadline_str = match trimmed {
                "1" => Some(
                    (now + chrono::Duration::days(1))
                        .format("%Y-%m-%d 17:00:00Z")
                        .to_string(),
                ),
                "2" => Some(
                    (now + chrono::Duration::days(2))
                        .format("%Y-%m-%d 17:00:00Z")
                        .to_string(),
                ),
                "3" => Some(
                    (now + chrono::Duration::days(7))
                        .format("%Y-%m-%d 17:00:00Z")
                        .to_string(),
                ),
                "4" => None,
                _ => {
                    if let Ok(d) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
                        Some(format!("{} 17:00:00Z", d))
                    } else {
                        Some(
                            (now + chrono::Duration::days(1))
                                .format("%Y-%m-%d 17:00:00Z")
                                .to_string(),
                        )
                    }
                }
            };

            draft.deadline_at = deadline_str;
            draft.step = "WAITING_EVALUATION".into();
            save_draft(pool, context, &draft).await;

            "📊 *Is this task for evaluation / marks?*\n\n1. *No* (Normal homework / practice)\n2. *Yes* (Enter marks & score later)\n\n_Reply 1 or 2._".into()
        }

        "WAITING_EVALUATION" => {
            if trimmed == "2"
                || trimmed.eq_ignore_ascii_case("yes")
                || trimmed.eq_ignore_ascii_case("y")
            {
                draft.is_evaluation = true;
                draft.step = "WAITING_MAX_MARKS".into();
                save_draft(pool, context, &draft).await;
                "💯 *Maximum Marks*\n\nWhat are the maximum marks available for this task? (e.g., '20' or '100')".into()
            } else {
                draft.is_evaluation = false;
                draft.step = "WAITING_CONFIRMATION".into();
                save_draft(pool, context, &draft).await;
                format_summary(&draft)
            }
        }

        "WAITING_MAX_MARKS" => {
            let max = trimmed.parse::<f64>().unwrap_or(100.0);
            draft.max_marks = Some(max);
            draft.step = "WAITING_CONFIRMATION".into();
            save_draft(pool, context, &draft).await;

            format_summary(&draft)
        }

        "WAITING_CONFIRMATION" => {
            let clean = trimmed.to_lowercase();
            if clean == "confirm" || clean == "yes" || clean == "publish" || clean == "1" {
                publish_from_draft(pool, context, &draft).await
            } else {
                "To publish this task, reply *Confirm*. To cancel, reply *Cancel*.".into()
            }
        }

        _ => {
            let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id).await;
            "Conversation reset. Send *assign homework* to start again.".into()
        }
    }
}

async fn advance_to_attendance_session(
    pool: &PgPool,
    context: &WhatsAppUserContext,
    draft: &mut LmsConversationDraft,
) -> String {
    let cs_id = draft.class_section_id.unwrap();
    let sub_id = draft.subject_id;

    // Check teacher's most recent relevant attendance session
    let default_sess =
        crate::modules::lms::repository::LmsRepository::get_default_attendance_session(
            pool,
            context.school_id,
            context.user_id,
            cs_id,
            sub_id,
        )
        .await
        .unwrap_or(None);

    if let Some(sess) = default_sess {
        draft.attendance_session_id = Some(sess.id);
        draft.attendance_date = Some(sess.date.to_string());
        draft.attendance_period = sess.period_number;
        draft.step = "WAITING_ATTENDANCE_CONFIRM".into();
        save_draft(pool, context, draft).await;

        let period_str = sess
            .period_number
            .map(|p| format!(" (Period {})", p))
            .unwrap_or_default();
        format!(
            "Class: *{} {}*\nSubject: *{}*\n\n\
            📅 Found your recent attendance session: *{}*{} marked by *{}*.\n\n\
            Do you want to link this task to that session?\n\
            1. *Yes* (Recommended)\n\
            2. *No* (Choose another session)\n\n\
            _Reply 'Yes' or 'No'._",
            draft.class_name.as_deref().unwrap_or(""),
            draft.section_name.as_deref().unwrap_or(""),
            draft.subject_name.as_deref().unwrap_or("General"),
            sess.date.format("%d %b %Y"),
            period_str,
            sess.marked_by_name.as_deref().unwrap_or("Teacher")
        )
    } else {
        // No session found
        let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id).await;
        format!(
            "⚠️ *No Attendance Session Found*\n\n\
            Every published LMS task must be linked to an attendance session for {} {}.\n\n\
            👉 Please take attendance first using *take attendance* or the attendance portal, then assign homework.",
            draft.class_name.as_deref().unwrap_or(""),
            draft.section_name.as_deref().unwrap_or("")
        )
    }
}

fn format_summary(draft: &LmsConversationDraft) -> String {
    let deadline_display = draft.deadline_at.as_deref().unwrap_or("No deadline");

    let eval_display = if draft.is_evaluation {
        format!("Yes (Max marks: {})", draft.max_marks.unwrap_or(100.0))
    } else {
        "No (Homework / Practice)".into()
    };

    format!(
        "📋 *Confirm LMS Task Publication*\n\n\
        • *Class*: {} {}\n\
        • *Subject*: {}\n\
        • *Title*: {}\n\
        • *Deadline*: {}\n\
        • *Evaluation*: {}\n\
        • *Attendance Session*: Linked ({})\n\
        • *WhatsApp Notification*: Active\n\n\
        *Description*:\n{}\n\n\
        {}\
        Reply *Confirm* to publish to students now, or *Cancel* to discard.",
        draft.class_name.as_deref().unwrap_or(""),
        draft.section_name.as_deref().unwrap_or(""),
        draft.subject_name.as_deref().unwrap_or("General"),
        draft.title.as_deref().unwrap_or(""),
        deadline_display,
        eval_display,
        draft.attendance_date.as_deref().unwrap_or("Active"),
        draft.description.as_deref().unwrap_or(""),
        draft
            .special_message
            .as_deref()
            .map(|m| format!("⚠️ *Special Note*: {}\n\n", m))
            .unwrap_or_default()
    )
}

async fn publish_from_draft(
    pool: &PgPool,
    context: &WhatsAppUserContext,
    draft: &LmsConversationDraft,
) -> String {
    let user_dummy = User {
        id: context.user_id,
        school_id: context.school_id,
        username: "whatsapp_bot".into(),
        password_hash: "".into(),
        display_name: context.display_name.clone(),
        roles: vec![context.role.clone()],
        capabilities: context.capabilities.iter().cloned().collect(),
        email_verified: true,
        phone_verified: true,
        school_status: "ACTIVE".into(),
        school_operable: true,
        school_onboarding_step: 4,
        is_class_teacher: false,
        has_assigned_class_teacher: false,
    };

    let deadline_dt = draft.deadline_at.as_deref().and_then(|d| {
        DateTime::parse_from_rfc3339(d)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    });

    let eval_cfg = if draft.is_evaluation {
        Some(CreateEvaluationConfig {
            evaluation_title: draft.title.clone(),
            maximum_marks: draft.max_marks.unwrap_or(100.0),
            pass_marks: Some(35.0),
            evaluation_category: Some("HOMEWORK".into()),
            evaluation_date: None,
            include_in_results: Some(draft.include_in_results),
            result_component_id: None,
        })
    } else {
        None
    };

    let req = CreateLmsTaskRequest {
        academic_year_id: None,
        title: draft
            .title
            .clone()
            .unwrap_or_else(|| "Untitled Task".into()),
        description: draft.description.clone().unwrap_or_default(),
        special_message: draft.special_message.clone(),
        task_type: Some("HOMEWORK".into()),
        class_section_id: draft.class_section_id.unwrap(),
        subject_id: draft.subject_id,
        teaching_group_id: None,
        attendance_session_id: draft.attendance_session_id,
        assigned_at: Some(Utc::now()),
        deadline_at: deadline_dt,
        add_to_calendar: Some(true),
        notify_whatsapp: Some(true),
        notify_email: Some(false),
        is_evaluation: Some(draft.is_evaluation),
        evaluation_config: eval_cfg,
        attachments: None,
        publish_now: Some(true),
        status: Some("PUBLISHED".into()),
    };

    match LmsService::create_task(pool, &user_dummy, req).await {
        Ok(task) => {
            let _ = WhatsAppRepository::clear_conversation_state(pool, context.session_id).await;
            format!(
                "🎉 *Task Published Successfully!*\n\n\
                *Title*: {}\n\
                *Class*: {} {}\n\
                *Recipients*: {} students notified\n\
                *Task ID*: {}\n\n\
                Students and parents have received their notifications on WhatsApp.",
                task.title,
                task.class_name.as_deref().unwrap_or(""),
                task.section_name.as_deref().unwrap_or(""),
                task.recipient_count,
                task.id
            )
        }
        Err(e) => {
            format!("❌ *Failed to publish task*: {:?}\n\nYou can reply *Confirm* to retry, or *Cancel*.", e)
        }
    }
}

async fn save_draft(pool: &PgPool, context: &WhatsAppUserContext, draft: &LmsConversationDraft) {
    let payload = serde_json::to_value(draft).unwrap_or_default();
    let _ =
        WhatsAppRepository::set_conversation_state(pool, context.session_id, "LMS_FLOW", payload)
            .await;
}

#[derive(Debug)]
struct ClassOption {
    class_section_id: Uuid,
    class_name: String,
    section_name: String,
}

async fn fetch_teacher_classes(
    pool: &PgPool,
    school_id: Uuid,
    teacher_id: Uuid,
) -> Vec<ClassOption> {
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT cs.id as class_section_id, c.name as class_name, sec.name as section_name
        FROM class_sections cs
        JOIN classes c ON cs.class_id = c.id
        JOIN sections sec ON cs.section_id = sec.id
        LEFT JOIN staff st ON st.user_id = $2 AND st.school_id = $1
        WHERE c.school_id = $1
          AND (
            cs.class_teacher_staff_id = $2
            OR (st.id IS NOT NULL AND cs.class_teacher_staff_id = st.id)
            OR EXISTS (
                SELECT 1 FROM timetable_slots ts
                WHERE ts.class_section_id = cs.id
                  AND (ts.teacher_id = $2 OR (st.id IS NOT NULL AND ts.teacher_id = st.id) OR $2 = ANY(ts.co_teacher_staff_ids))
            )
          )
        ORDER BY c.name, sec.name ASC
        "#
    )
    .bind(school_id)
    .bind(teacher_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        // Fallback for school admins or teachers without explicit slots: all school classes
        let all_rows = sqlx::query(
            r#"
            SELECT cs.id as class_section_id, c.name as class_name, sec.name as section_name
            FROM class_sections cs
            JOIN classes c ON cs.class_id = c.id
            JOIN sections sec ON cs.section_id = sec.id
            WHERE c.school_id = $1
            ORDER BY c.name, sec.name ASC
            "#,
        )
        .bind(school_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        return all_rows
            .into_iter()
            .map(|r| ClassOption {
                class_section_id: r.get("class_section_id"),
                class_name: r.get("class_name"),
                section_name: r.get("section_name"),
            })
            .collect();
    }

    rows.into_iter()
        .map(|r| ClassOption {
            class_section_id: r.get("class_section_id"),
            class_name: r.get("class_name"),
            section_name: r.get("section_name"),
        })
        .collect()
}

async fn fetch_class_subjects(
    pool: &PgPool,
    school_id: Uuid,
    class_section_id: Uuid,
) -> Vec<(Uuid, String)> {
    let rows = sqlx::query(
        r#"
        SELECT s.id, s.name
        FROM subjects s
        WHERE s.school_id = $1
          AND (
            s.class_id = (SELECT class_id FROM class_sections WHERE id = $2)
            OR EXISTS (SELECT 1 FROM timetable_slots ts WHERE ts.class_section_id = $2 AND ts.subject_id = s.id)
          )
        ORDER BY s.name ASC
        "#
    )
    .bind(school_id)
    .bind(class_section_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.into_iter()
        .map(|r| (r.get("id"), r.get("name")))
        .collect()
}

struct SessionOption {
    id: Uuid,
    date: NaiveDate,
    period_number: Option<i32>,
}

async fn fetch_recent_sessions(
    pool: &PgPool,
    school_id: Uuid,
    class_section_id: Uuid,
    subject_id: Option<Uuid>,
) -> Vec<SessionOption> {
    let rows = sqlx::query(
        r#"
        SELECT id, date, period_number
        FROM attendance_sessions
        WHERE school_id = $1 AND class_section_id = $2
          AND ($3::uuid IS NULL OR subject_id = $3 OR attendance_type = 'DAILY')
        ORDER BY date DESC, period_number DESC NULLS LAST
        LIMIT 5
        "#,
    )
    .bind(school_id)
    .bind(class_section_id)
    .bind(subject_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.into_iter()
        .map(|r| SessionOption {
            id: r.get("id"),
            date: r.get("date"),
            period_number: r.try_get("period_number").ok(),
        })
        .collect()
}
