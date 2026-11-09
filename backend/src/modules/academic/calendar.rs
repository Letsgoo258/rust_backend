use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCalendarEntryRequest {
    pub date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub entry_type: Option<String>, // 'HOLIDAY', 'SPECIAL_WORKING_DAY', 'EVENT', 'EXAMINATION'
    pub title: String,
    pub description: Option<String>,
    pub is_working_day: Option<bool>,
    pub applies_to_all_classes: Option<bool>,
    pub class_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEntryItem {
    pub id: Uuid,
    pub school_id: Uuid,
    pub academic_year_id: Uuid,
    pub date: NaiveDate,
    pub entry_type: String,
    pub title: String,
    pub description: Option<String>,
    pub is_working_day: bool,
    pub applies_to_all_classes: bool,
    pub class_ids: Vec<Uuid>,
    pub class_names: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarHolidayCheckResult {
    pub date: NaiveDate,
    pub is_holiday: bool,
    pub is_working_day: bool,
    pub title: Option<String>,
    pub description: Option<String>,
    pub entry_type: Option<String>,
    pub applies_to_all_classes: bool,
    pub affected_class_ids: Vec<Uuid>,
    pub affected_class_names: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ListCalendarQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub class_id: Option<Uuid>,
    pub year: Option<i32>,
    pub month: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CheckHolidayQuery {
    pub date: String,
    pub class_id: Option<Uuid>,
    pub class_section_id: Option<Uuid>,
}

pub struct CalendarRepository;

impl CalendarRepository {
    /// Retrieve all calendar entries within optional date window, populating class names
    pub async fn list_entries(
        pool: &PgPool,
        school_id: Uuid,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
        class_id: Option<Uuid>,
    ) -> Result<Vec<CalendarEntryItem>, AppError> {
        // 1. Fetch class lookup map for naming
        let class_rows = sqlx::query("SELECT id, name FROM classes WHERE school_id = $1")
            .bind(school_id)
            .fetch_all(pool)
            .await?;

        let class_map: HashMap<Uuid, String> = class_rows
            .into_iter()
            .map(|r| (r.get("id"), r.get("name")))
            .collect();

        // 2. Fetch calendar records
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                school_id,
                academic_year_id,
                date,
                type as entry_type,
                title,
                description,
                COALESCE(is_working_day, false) as is_working_day,
                applies_to_all_classes,
                class_ids,
                created_at
            FROM academic_calendar
            WHERE school_id = $1
              AND ($2::date IS NULL OR date >= $2)
              AND ($3::date IS NULL OR date <= $3)
              AND ($4::uuid IS NULL OR applies_to_all_classes = true OR (class_ids IS NOT NULL AND class_ids @> ARRAY[$4]))
            ORDER BY date ASC, created_at ASC
            "#,
        )
        .bind(school_id)
        .bind(start_date)
        .bind(end_date)
        .bind(class_id)
        .fetch_all(pool)
        .await?;

        let mut items = Vec::new();
        for r in rows {
            let raw_cids: Vec<Uuid> = r.try_get("class_ids").unwrap_or_default();
            let c_names: Vec<String> = raw_cids
                .iter()
                .filter_map(|cid| class_map.get(cid).cloned())
                .collect();

            items.push(CalendarEntryItem {
                id: r.get("id"),
                school_id: r.get("school_id"),
                academic_year_id: r.get("academic_year_id"),
                date: r.get("date"),
                entry_type: r.get("entry_type"),
                title: r.get("title"),
                description: r.try_get("description").ok(),
                is_working_day: r.get("is_working_day"),
                applies_to_all_classes: r.get("applies_to_all_classes"),
                class_ids: raw_cids,
                class_names: c_names,
                created_at: r.get("created_at"),
            });
        }

        Ok(items)
    }

    /// Creates calendar entries, expanding date range if end_date is provided
    pub async fn create_entries(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        req: CreateCalendarEntryRequest,
    ) -> Result<Vec<Uuid>, AppError> {
        let title = req.title.trim();
        if title.is_empty() {
            return Err(AppError::BadRequest(
                "Holiday/Calendar title is required".into(),
            ));
        }

        // Determine active academic year
        let year_id: Uuid = sqlx::query_scalar(
            "SELECT id FROM academic_years WHERE school_id = $1 AND status = 'ACTIVE' LIMIT 1",
        )
        .bind(school_id)
        .fetch_optional(pool)
        .await?
        .or(sqlx::query_scalar(
            "SELECT id FROM academic_years WHERE school_id = $1 ORDER BY start_date DESC LIMIT 1",
        )
        .bind(school_id)
        .fetch_optional(pool)
        .await?)
        .ok_or_else(|| AppError::BadRequest("No academic year found for school".into()))?;

        let entry_type = req
            .entry_type
            .unwrap_or_else(|| "HOLIDAY".to_string())
            .to_uppercase();

        let is_working = req
            .is_working_day
            .unwrap_or(entry_type == "SPECIAL_WORKING_DAY");
        let applies_all = req.applies_to_all_classes.unwrap_or(true);
        let class_ids = req.class_ids.unwrap_or_default();

        let start_date = req.date;
        let end_date = req.end_date.unwrap_or(start_date);

        if end_date < start_date {
            return Err(AppError::BadRequest(
                "End date cannot precede start date".into(),
            ));
        }

        // Calculate all dates in range (max 90 days protection)
        let days_diff = (end_date - start_date).num_days();
        if days_diff > 90 {
            return Err(AppError::BadRequest(
                "Date range cannot exceed 90 days in a single declaration".into(),
            ));
        }

        let mut inserted_ids = Vec::new();
        let mut cur_date = start_date;

        while cur_date <= end_date {
            let id: Uuid = sqlx::query_scalar(
                r#"
                INSERT INTO academic_calendar (
                    school_id, academic_year_id, date, type, title, description,
                    is_working_day, applies_to_all_classes, class_ids, created_by,
                    created_at, updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6,
                    $7, $8, $9, $10,
                    NOW(), NOW()
                )
                RETURNING id
                "#,
            )
            .bind(school_id)
            .bind(year_id)
            .bind(cur_date)
            .bind(&entry_type)
            .bind(title)
            .bind(req.description.as_deref())
            .bind(is_working)
            .bind(applies_all)
            .bind(&class_ids)
            .bind(user_id)
            .fetch_one(pool)
            .await?;

            inserted_ids.push(id);
            cur_date = match cur_date.succ_opt() {
                Some(d) => d,
                None => break,
            };
        }

        Ok(inserted_ids)
    }

    /// Deletes a calendar entry
    pub async fn delete_entry(
        pool: &PgPool,
        school_id: Uuid,
        entry_id: Uuid,
    ) -> Result<(), AppError> {
        let res = sqlx::query("DELETE FROM academic_calendar WHERE id = $1 AND school_id = $2")
            .bind(entry_id)
            .bind(school_id)
            .execute(pool)
            .await?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Calendar entry not found".into()));
        }

        Ok(())
    }

    /// Check if target date (and optional class or class_section) is a declared holiday or working day
    pub async fn check_date_holiday(
        pool: &PgPool,
        school_id: Uuid,
        date: NaiveDate,
        target_class_id: Option<Uuid>,
        target_class_section_id: Option<Uuid>,
    ) -> Result<CalendarHolidayCheckResult, AppError> {
        // If class_section_id is given but class_id is None, resolve class_id
        let resolved_class_id = if let Some(cid) = target_class_id {
            Some(cid)
        } else if let Some(cs_id) = target_class_section_id {
            sqlx::query_scalar(
                "SELECT class_id FROM class_sections WHERE id = $1 AND school_id = $2",
            )
            .bind(cs_id)
            .bind(school_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
        } else {
            None
        };

        // Query entries for date
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                type as entry_type,
                title,
                description,
                COALESCE(is_working_day, false) as is_working_day,
                applies_to_all_classes,
                class_ids
            FROM academic_calendar
            WHERE school_id = $1 AND date = $2
            ORDER BY is_working_day DESC, applies_to_all_classes DESC
            "#,
        )
        .bind(school_id)
        .bind(date)
        .fetch_all(pool)
        .await?;

        if rows.is_empty() {
            return Ok(CalendarHolidayCheckResult {
                date,
                is_holiday: false,
                is_working_day: true, // Default open working day unless weekend/Sunday
                title: None,
                description: None,
                entry_type: None,
                applies_to_all_classes: true,
                affected_class_ids: Vec::new(),
                affected_class_names: Vec::new(),
            });
        }

        // 1. If any entry explicitly declares this day as a SPECIAL_WORKING_DAY, it overrides normal off days
        if let Some(working_row) = rows.iter().find(|r| r.get::<bool, _>("is_working_day")) {
            return Ok(CalendarHolidayCheckResult {
                date,
                is_holiday: false,
                is_working_day: true,
                title: Some(working_row.get("title")),
                description: working_row.try_get("description").ok(),
                entry_type: Some(working_row.get("entry_type")),
                applies_to_all_classes: working_row.get("applies_to_all_classes"),
                affected_class_ids: working_row.try_get("class_ids").unwrap_or_default(),
                affected_class_names: Vec::new(),
            });
        }

        // 2. Fetch class names if needed
        let class_rows = sqlx::query("SELECT id, name FROM classes WHERE school_id = $1")
            .bind(school_id)
            .fetch_all(pool)
            .await?;
        let class_map: HashMap<Uuid, String> = class_rows
            .into_iter()
            .map(|r| (r.get("id"), r.get("name")))
            .collect();

        // 3. Inspect holiday / off entries
        for r in &rows {
            let applies_all: bool = r.get("applies_to_all_classes");
            let cids: Vec<Uuid> = r.try_get("class_ids").unwrap_or_default();
            let cnames: Vec<String> = cids
                .iter()
                .filter_map(|cid| class_map.get(cid).cloned())
                .collect();

            // School-wide holiday
            if applies_all {
                return Ok(CalendarHolidayCheckResult {
                    date,
                    is_holiday: true,
                    is_working_day: false,
                    title: Some(r.get("title")),
                    description: r.try_get("description").ok(),
                    entry_type: Some(r.get("entry_type")),
                    applies_to_all_classes: true,
                    affected_class_ids: cids,
                    affected_class_names: cnames,
                });
            }

            // Class-specific holiday
            if let Some(target_cid) = resolved_class_id {
                if cids.contains(&target_cid) {
                    return Ok(CalendarHolidayCheckResult {
                        date,
                        is_holiday: true,
                        is_working_day: false,
                        title: Some(r.get("title")),
                        description: r.try_get("description").ok(),
                        entry_type: Some(r.get("entry_type")),
                        applies_to_all_classes: false,
                        affected_class_ids: cids,
                        affected_class_names: cnames,
                    });
                }
            } else if !cids.is_empty() {
                // No specific class queried, report general holiday info
                return Ok(CalendarHolidayCheckResult {
                    date,
                    is_holiday: true,
                    is_working_day: false,
                    title: Some(r.get("title")),
                    description: r.try_get("description").ok(),
                    entry_type: Some(r.get("entry_type")),
                    applies_to_all_classes: false,
                    affected_class_ids: cids,
                    affected_class_names: cnames,
                });
            }
        }

        Ok(CalendarHolidayCheckResult {
            date,
            is_holiday: false,
            is_working_day: true,
            title: None,
            description: None,
            entry_type: None,
            applies_to_all_classes: true,
            affected_class_ids: Vec::new(),
            affected_class_names: Vec::new(),
        })
    }
}

pub struct CalendarHandler;

impl CalendarHandler {
    /// GET /api/v1/academic/calendar - list registered calendar entries
    pub async fn list_calendar(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(q): Query<ListCalendarQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let (computed_start, computed_end) = match (q.year, q.month) {
            (Some(y), Some(m)) => {
                let start = NaiveDate::from_ymd_opt(y, m, 1);
                let next_month = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
                let end = NaiveDate::from_ymd_opt(next_month.0, next_month.1, 1)
                    .map(|d| d - chrono::Duration::days(1));
                (start, end)
            }
            (Some(y), None) => (
                NaiveDate::from_ymd_opt(y, 1, 1),
                NaiveDate::from_ymd_opt(y, 12, 31),
            ),
            _ => (None, None),
        };

        let start_date = q
            .start_date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .or(computed_start);
        let end_date = q
            .end_date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .or(computed_end);

        let entries = CalendarRepository::list_entries(
            &state.db,
            user.school_id,
            start_date,
            end_date,
            q.class_id,
        )
        .await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "entries": entries,
            "count": entries.len()
        })))
    }

    /// POST /api/v1/academic/calendar - register a single or date-range holiday / event
    pub async fn create_calendar_entry(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(payload): Json<CreateCalendarEntryRequest>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        // Institutional authorization check
        let is_authorized = user.is_school_admin()
            || user.has_capability("academic:manage_calendar")
            || user.has_capability("calendar:manage");

        if !is_authorized {
            return Err(AppError::Forbidden(
                "Only school administrators can declare holidays and calendar events".into(),
            ));
        }

        let inserted_ids =
            CalendarRepository::create_entries(&state.db, user.school_id, user.id, payload).await?;

        Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({
                "success": true,
                "message": format!("Successfully registered {} calendar date(s).", inserted_ids.len()),
                "inserted_ids": inserted_ids
            })),
        ))
    }

    /// DELETE /api/v1/academic/calendar/:id - delete a calendar entry
    pub async fn delete_calendar_entry(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let is_authorized = user.is_school_admin()
            || user.has_capability("academic:manage_calendar")
            || user.has_capability("calendar:manage");

        if !is_authorized {
            return Err(AppError::Forbidden(
                "Only school administrators can delete calendar entries".into(),
            ));
        }

        CalendarRepository::delete_entry(&state.db, user.school_id, id).await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Calendar entry deleted successfully."
        })))
    }

    /// GET /api/v1/academic/calendar/check - check if date (and optional class) is a holiday
    pub async fn check_holiday(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(q): Query<CheckHolidayQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Authentication required".into()))?;

        let date = NaiveDate::parse_from_str(&q.date, "%Y-%m-%d")
            .map_err(|_| AppError::BadRequest("Invalid date format, expected YYYY-MM-DD".into()))?;

        let result = CalendarRepository::check_date_holiday(
            &state.db,
            user.school_id,
            date,
            q.class_id,
            q.class_section_id,
        )
        .await?;

        Ok(Json(serde_json::json!({
            "success": true,
            "data": result
        })))
    }
}
