use crate::error::AppError;
use crate::modules::academic::timetable::generator::{
    AutoGenerateRequest, AutoGenerateResponse, TimetableGenerator,
};
use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::{HashMap, HashSet};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimetableSlotRow {
    pub id: Uuid,
    pub school_id: Uuid,
    pub academic_year_id: Uuid,
    pub class_section_id: Uuid,
    pub class_name: Option<String>,
    pub section_name: Option<String>,
    pub day_of_week: i32,
    pub period_number: i32,
    pub start_time: String,
    pub end_time: String,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub subject_code: String,
    pub subject_color: Option<String>,
    pub teacher_staff_id: Uuid,
    pub teacher_name: String,
    pub co_teacher_staff_ids: Vec<Uuid>,
    pub is_shared: bool,
    pub room_id: Option<Uuid>,
    pub room_name: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSlotInput {
    pub id: Option<Uuid>,
    pub day_of_week: i32,
    pub period_number: i32,
    pub start_time: String,
    pub end_time: String,
    pub subject_id: Uuid,
    pub teacher_staff_id: Uuid,
    #[serde(default)]
    pub co_teacher_staff_ids: Vec<Uuid>,
    #[serde(default)]
    pub is_shared: bool,
    pub room_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSaveSlotsPayload {
    pub class_section_id: Uuid,
    pub academic_year_id: Option<Uuid>,
    pub slots: Vec<SaveSlotInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubstitutionRow {
    pub id: Uuid,
    pub school_id: Uuid,
    pub date: String,
    pub timetable_slot_id: Uuid,
    pub period_number: i32,
    pub day_of_week: i32,
    pub start_time: String,
    pub end_time: String,
    pub class_name: String,
    pub section_name: String,
    pub subject_name: String,
    pub original_teacher_id: Uuid,
    pub original_teacher_name: String,
    pub substitute_teacher_id: Uuid,
    pub substitute_teacher_name: String,
    pub status: String,
    pub reason: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSubstitutionInput {
    pub date: String, // YYYY-MM-DD
    pub timetable_slot_id: Uuid,
    pub substitute_teacher_id: Uuid,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableTeacherRow {
    pub staff_id: Uuid,
    pub teacher_name: String,
    pub name: String,
    pub employee_id: String,
    pub department: Option<String>,
    pub designation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentDayScheduleRow {
    pub id: Uuid,
    pub date: String,
    pub timetable_slot_id: Uuid,
    pub class_section_id: Uuid,
    pub class_name: String,
    pub section_name: String,
    pub day_of_week: i32,
    pub period_number: i32,
    pub start_time: String,
    pub end_time: String,
    pub subject_name: String,
    pub subject_code: String,
    pub subject_color: Option<String>,
    pub effective_teacher_id: Uuid,
    pub effective_teacher_name: String,
    pub original_teacher_id: Uuid,
    pub original_teacher_name: String,
    pub co_teacher_ids: Vec<Uuid>,
    pub is_shared_subject: bool,
    pub is_substituted: bool,
    pub substitution_id: Option<Uuid>,
    pub room_name: Option<String>,
    pub status: String,
}

pub struct TimetableService;

impl TimetableService {
    /// Retrieve full weekly timetable for a given class section
    pub async fn get_section_timetable(
        pool: &PgPool,
        school_id: Uuid,
        class_section_id: Uuid,
    ) -> Result<Vec<TimetableSlotRow>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                ts.id, ts.school_id, ts.academic_year_id, ts.class_section_id,
                c.name as class_name, s.name as section_name,
                ts.day_of_week, ts.period_number,
                ts.start_time::text as start_time, ts.end_time::text as end_time,
                ts.subject_id, subj.name as subject_name, subj.code as subject_code, subj.color as subject_color,
                ts.teacher_staff_id,
                COALESCE(NULLIF(u.display_name, ''), u.username, 'Staff') as teacher_name,
                COALESCE(ts.co_teacher_staff_ids, '{}') as co_teacher_staff_ids,
                COALESCE(ts.is_shared, false) as is_shared,
                ts.room_id, r.name as room_name,
                ts.status
            FROM timetable_slots ts
            JOIN class_sections cs ON ts.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            JOIN subjects subj ON ts.subject_id = subj.id
            JOIN staff st ON ts.teacher_staff_id = st.id
            JOIN users u ON st.user_id = u.id
            LEFT JOIN rooms r ON ts.room_id = r.id
            WHERE ts.school_id = $1 AND ts.class_section_id = $2 AND ts.status = 'ACTIVE'
            ORDER BY ts.day_of_week ASC, ts.period_number ASC
            "#,
        )
        .bind(school_id)
        .bind(class_section_id)
        .fetch_all(pool)
        .await?;

        let slots = rows
            .into_iter()
            .map(|r| TimetableSlotRow {
                id: r.get("id"),
                school_id: r.get("school_id"),
                academic_year_id: r.get("academic_year_id"),
                class_section_id: r.get("class_section_id"),
                class_name: r.get("class_name"),
                section_name: r.get("section_name"),
                day_of_week: r.get("day_of_week"),
                period_number: r.get("period_number"),
                start_time: r.get("start_time"),
                end_time: r.get("end_time"),
                subject_id: r.get("subject_id"),
                subject_name: r.get("subject_name"),
                subject_code: r.get("subject_code"),
                subject_color: r.get("subject_color"),
                teacher_staff_id: r.get("teacher_staff_id"),
                teacher_name: r.get("teacher_name"),
                co_teacher_staff_ids: r.try_get("co_teacher_staff_ids").unwrap_or_default(),
                is_shared: r.try_get("is_shared").unwrap_or(false),
                room_id: r.get("room_id"),
                room_name: r.get("room_name"),
                status: r.get("status"),
            })
            .collect();

        Ok(slots)
    }

    /// Retrieve scheduled slots for a teacher on a day (or all days)
    pub async fn get_teacher_slots(
        pool: &PgPool,
        school_id: Uuid,
        teacher_id: Uuid,
        day_of_week: Option<i32>,
    ) -> Result<Vec<TimetableSlotRow>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                ts.id, ts.school_id, ts.academic_year_id, ts.class_section_id,
                c.name as class_name, s.name as section_name,
                ts.day_of_week, ts.period_number,
                ts.start_time::text as start_time, ts.end_time::text as end_time,
                ts.subject_id, subj.name as subject_name, subj.code as subject_code, subj.color as subject_color,
                ts.teacher_staff_id,
                COALESCE(NULLIF(u.display_name, ''), u.username, 'Staff') as teacher_name,
                COALESCE(ts.co_teacher_staff_ids, '{}') as co_teacher_staff_ids,
                COALESCE(ts.is_shared, false) as is_shared,
                ts.room_id, r.name as room_name,
                ts.status
            FROM timetable_slots ts
            JOIN class_sections cs ON ts.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            JOIN subjects subj ON ts.subject_id = subj.id
            JOIN staff st ON ts.teacher_staff_id = st.id
            JOIN users u ON st.user_id = u.id
            LEFT JOIN rooms r ON ts.room_id = r.id
            WHERE ts.school_id = $1 AND (ts.teacher_staff_id = $2 OR ts.co_teacher_staff_ids @> ARRAY[$2::uuid])
              AND ($3::int IS NULL OR ts.day_of_week = $3)
              AND ts.status = 'ACTIVE'
            ORDER BY ts.day_of_week ASC, ts.period_number ASC
            "#,
        )
        .bind(school_id)
        .bind(teacher_id)
        .bind(day_of_week)
        .fetch_all(pool)
        .await?;

        let slots = rows
            .into_iter()
            .map(|r| TimetableSlotRow {
                id: r.get("id"),
                school_id: r.get("school_id"),
                academic_year_id: r.get("academic_year_id"),
                class_section_id: r.get("class_section_id"),
                class_name: r.get("class_name"),
                section_name: r.get("section_name"),
                day_of_week: r.get("day_of_week"),
                period_number: r.get("period_number"),
                start_time: r.get("start_time"),
                end_time: r.get("end_time"),
                subject_id: r.get("subject_id"),
                subject_name: r.get("subject_name"),
                subject_code: r.get("subject_code"),
                subject_color: r.get("subject_color"),
                teacher_staff_id: r.get("teacher_staff_id"),
                teacher_name: r.get("teacher_name"),
                co_teacher_staff_ids: r.try_get("co_teacher_staff_ids").unwrap_or_default(),
                is_shared: r.try_get("is_shared").unwrap_or(false),
                room_id: r.get("room_id"),
                room_name: r.get("room_name"),
                status: r.get("status"),
            })
            .collect();

        Ok(slots)
    }

    /// Batch upsert timetable slots deterministically with deadlock elimination
    pub async fn batch_upsert_slots(
        pool: &PgPool,
        school_id: Uuid,
        payload: BatchSaveSlotsPayload,
    ) -> Result<usize, AppError> {
        let academic_year_id = match payload.academic_year_id {
            Some(id) => id,
            None => {
                crate::modules::academic::repository::AcademicRepository::get_active_academic_year_id(
                    pool, school_id,
                )
                .await?
                .ok_or_else(|| AppError::BadRequest("No active academic year found for school".into()))?
            }
        };

        let mut tx = pool.begin().await?;

        // Deterministic lock: acquire row lock on class_sections to serialize updates per section
        let _ = sqlx::query("SELECT id FROM class_sections WHERE id = $1 FOR UPDATE")
            .bind(payload.class_section_id)
            .fetch_optional(&mut *tx)
            .await?;

        let mut count = 0;
        for slot in payload.slots {
            let start = NaiveTime::parse_from_str(&slot.start_time, "%H:%M:%S")
                .or_else(|_| NaiveTime::parse_from_str(&slot.start_time, "%H:%M"))
                .map_err(|e| AppError::BadRequest(format!("Invalid start_time: {e}")))?;
            let end = NaiveTime::parse_from_str(&slot.end_time, "%H:%M:%S")
                .or_else(|_| NaiveTime::parse_from_str(&slot.end_time, "%H:%M"))
                .map_err(|e| AppError::BadRequest(format!("Invalid end_time: {e}")))?;

            // Upsert with atomic on conflict
            sqlx::query(
                r#"
                INSERT INTO timetable_slots (
                    school_id, academic_year_id, class_section_id,
                    day_of_week, period_number, start_time, end_time,
                    subject_id, teacher_staff_id, co_teacher_staff_ids, is_shared, room_id, status
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 'ACTIVE')
                ON CONFLICT (class_section_id, day_of_week, period_number) DO UPDATE SET
                    start_time = EXCLUDED.start_time,
                    end_time = EXCLUDED.end_time,
                    subject_id = EXCLUDED.subject_id,
                    teacher_staff_id = EXCLUDED.teacher_staff_id,
                    co_teacher_staff_ids = EXCLUDED.co_teacher_staff_ids,
                    is_shared = EXCLUDED.is_shared,
                    room_id = EXCLUDED.room_id,
                    status = 'ACTIVE',
                    updated_at = NOW()
                "#,
            )
            .bind(school_id)
            .bind(academic_year_id)
            .bind(payload.class_section_id)
            .bind(slot.day_of_week)
            .bind(slot.period_number)
            .bind(start)
            .bind(end)
            .bind(slot.subject_id)
            .bind(slot.teacher_staff_id)
            .bind(&slot.co_teacher_staff_ids)
            .bind(slot.is_shared)
            .bind(slot.room_id)
            .execute(&mut *tx)
            .await?;

            count += 1;
        }

        tx.commit().await?;

        // Re-sync today's projection for this school
        let today = chrono::Utc::now().date_naive();
        let _ = Self::sync_daily_timetable_for_date(pool, today).await;

        Ok(count)
    }

    /// Delete a timetable slot
    pub async fn delete_slot(
        pool: &PgPool,
        school_id: Uuid,
        slot_id: Uuid,
    ) -> Result<(), AppError> {
        let res = sqlx::query("DELETE FROM timetable_slots WHERE id = $1 AND school_id = $2")
            .bind(slot_id)
            .bind(school_id)
            .execute(pool)
            .await?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Timetable slot not found".into()));
        }

        Ok(())
    }

    /// Auto-generate clash-free preview
    pub async fn auto_generate_preview(
        pool: &PgPool,
        school_id: Uuid,
        req: AutoGenerateRequest,
    ) -> Result<AutoGenerateResponse, AppError> {
        // 1. Fetch all other existing commitments in the school for all other sections
        let existing_teacher_rows = sqlx::query_as::<_, (Uuid, i32, i32)>(
            r#"
            SELECT teacher_staff_id, day_of_week, period_number
            FROM timetable_slots
            WHERE school_id = $1 AND class_section_id != $2 AND status = 'ACTIVE'
            "#,
        )
        .bind(school_id)
        .bind(req.class_section_id)
        .fetch_all(pool)
        .await?;

        let mut existing_teachers: HashSet<(Uuid, u32, u32)> = HashSet::new();
        let mut teacher_day_workload: HashMap<(Uuid, u32), u32> = HashMap::new();
        for (tid, day, period) in existing_teacher_rows {
            existing_teachers.insert((tid, day as u32, period as u32));
            *teacher_day_workload.entry((tid, day as u32)).or_insert(0) += 1;
        }

        let existing_room_rows = sqlx::query_as::<_, (Uuid, i32, i32)>(
            r#"
            SELECT room_id, day_of_week, period_number
            FROM timetable_slots
            WHERE school_id = $1 AND class_section_id != $2 AND room_id IS NOT NULL AND status = 'ACTIVE'
            "#,
        )
        .bind(school_id)
        .bind(req.class_section_id)
        .fetch_all(pool)
        .await?;

        let mut existing_rooms: HashSet<(Uuid, u32, u32)> = HashSet::new();
        for (rid, day, period) in existing_room_rows {
            existing_rooms.insert((rid, day as u32, period as u32));
        }

        // 2. Run solver with workload and continuous constraints
        match TimetableGenerator::solve(
            &req,
            &existing_teachers,
            &existing_rooms,
            &teacher_day_workload,
        ) {
            Ok(slots) => Ok(AutoGenerateResponse {
                success: true,
                total_slots: slots.len(),
                message: format!("Successfully generated {} clash-free slots!", slots.len()),
                slots,
            }),
            Err(e) => Err(AppError::BadRequest(e)),
        }
    }

    /// List substitutions for a given date
    pub async fn list_substitutions(
        pool: &PgPool,
        school_id: Uuid,
        date: NaiveDate,
    ) -> Result<Vec<SubstitutionRow>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                sub.id, sub.school_id, sub.date::text as date,
                sub.timetable_slot_id, ts.period_number, ts.day_of_week,
                ts.start_time::text as start_time, ts.end_time::text as end_time,
                c.name as class_name, s.name as section_name,
                subj.name as subject_name,
                sub.original_teacher_id,
                COALESCE(NULLIF(u_orig.display_name, ''), u_orig.username, 'Teacher') as original_teacher_name,
                sub.substitute_teacher_id,
                COALESCE(NULLIF(u_sub.display_name, ''), u_sub.username, 'Substitute') as substitute_teacher_name,
                sub.status, sub.reason, sub.created_at::text as created_at
            FROM substitutions sub
            JOIN timetable_slots ts ON sub.timetable_slot_id = ts.id
            JOIN class_sections cs ON ts.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            JOIN subjects subj ON ts.subject_id = subj.id
            JOIN staff st_orig ON sub.original_teacher_id = st_orig.id
            JOIN users u_orig ON st_orig.user_id = u_orig.id
            JOIN staff st_sub ON sub.substitute_teacher_id = st_sub.id
            JOIN users u_sub ON st_sub.user_id = u_sub.id
            WHERE sub.school_id = $1 AND sub.date = $2
            ORDER BY ts.period_number ASC
            "#,
        )
        .bind(school_id)
        .bind(date)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| SubstitutionRow {
                id: r.get("id"),
                school_id: r.get("school_id"),
                date: r.get("date"),
                timetable_slot_id: r.get("timetable_slot_id"),
                period_number: r.get("period_number"),
                day_of_week: r.get("day_of_week"),
                start_time: r.get("start_time"),
                end_time: r.get("end_time"),
                class_name: r.get("class_name"),
                section_name: r.get("section_name"),
                subject_name: r.get("subject_name"),
                original_teacher_id: r.get("original_teacher_id"),
                original_teacher_name: r.get("original_teacher_name"),
                substitute_teacher_id: r.get("substitute_teacher_id"),
                substitute_teacher_name: r.get("substitute_teacher_name"),
                status: r.get("status"),
                reason: r.get("reason"),
                created_at: r.get("created_at"),
            })
            .collect();

        Ok(list)
    }

    /// Create or record a teacher substitution
    pub async fn create_substitution(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        input: CreateSubstitutionInput,
    ) -> Result<Uuid, AppError> {
        let date = NaiveDate::parse_from_str(&input.date, "%Y-%m-%d")
            .map_err(|e| AppError::BadRequest(format!("Invalid date format: {e}")))?;

        // 1. Fetch timetable slot details
        let slot = sqlx::query_as::<_, (Uuid, i32, i32, Uuid)>(
            "SELECT id, day_of_week, period_number, teacher_staff_id FROM timetable_slots WHERE id = $1 AND school_id = $2"
        )
        .bind(input.timetable_slot_id)
        .bind(school_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Timetable slot not found".into()))?;

        let (slot_id, day_of_week, period_number, orig_teacher_id) = slot;

        // 2. Validate substitute teacher: make sure they do not have a teaching slot or active substitution at this period
        let has_regular_clash: bool = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM timetable_slots
                WHERE school_id = $1 AND teacher_staff_id = $2
                  AND day_of_week = $3 AND period_number = $4 AND status = 'ACTIVE'
            )
            "#,
        )
        .bind(school_id)
        .bind(input.substitute_teacher_id)
        .bind(day_of_week)
        .bind(period_number)
        .fetch_one(pool)
        .await?;

        if has_regular_clash {
            return Err(AppError::BadRequest(
                "Substitute teacher already has a scheduled regular class in this period.".into(),
            ));
        }

        let has_sub_clash: bool = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM substitutions sub
                JOIN timetable_slots ts ON sub.timetable_slot_id = ts.id
                WHERE sub.school_id = $1 AND sub.substitute_teacher_id = $2
                  AND sub.date = $3 AND ts.period_number = $4 AND sub.status = 'ACTIVE'
            )
            "#,
        )
        .bind(school_id)
        .bind(input.substitute_teacher_id)
        .bind(date)
        .bind(period_number)
        .fetch_one(pool)
        .await?;

        if has_sub_clash {
            return Err(AppError::BadRequest(
                "Substitute teacher is already assigned to another substitution in this period."
                    .into(),
            ));
        }

        // 3. Insert or update substitution
        let sub_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO substitutions (
                school_id, date, timetable_slot_id,
                original_teacher_id, substitute_teacher_id, status, reason, created_by
            )
            VALUES ($1, $2, $3, $4, $5, 'ACTIVE', $6, $7)
            ON CONFLICT (timetable_slot_id, date) DO UPDATE SET
                substitute_teacher_id = EXCLUDED.substitute_teacher_id,
                status = 'ACTIVE',
                reason = EXCLUDED.reason,
                created_by = EXCLUDED.created_by,
                updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(school_id)
        .bind(date)
        .bind(slot_id)
        .bind(orig_teacher_id)
        .bind(input.substitute_teacher_id)
        .bind(input.reason.clone())
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        info!(
            school_id = %school_id,
            slot_id = %slot_id,
            date = %date,
            sub_id = %sub_id,
            "Substitution successfully created and synced to active schedule"
        );

        // Enqueue automated Email & WhatsApp notifications
        crate::modules::academic::timetable::notification::queue_substitution_notifications(
            pool,
            school_id,
            sub_id,
            slot_id,
            input.substitute_teacher_id,
            date,
            input.reason.as_deref(),
        )
        .await;

        Ok(sub_id)
    }

    /// Cancel a substitution
    pub async fn cancel_substitution(
        pool: &PgPool,
        school_id: Uuid,
        sub_id: Uuid,
    ) -> Result<(), AppError> {
        let res = sqlx::query("DELETE FROM substitutions WHERE id = $1 AND school_id = $2")
            .bind(sub_id)
            .bind(school_id)
            .execute(pool)
            .await?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Substitution not found".into()));
        }

        Ok(())
    }

    /// Find all teaching staff with zero clashes for a specific slot on a date
    pub async fn get_available_substitute_teachers(
        pool: &PgPool,
        school_id: Uuid,
        date: NaiveDate,
        day_of_week: i32,
        period_number: i32,
        exclude_staff_id: Option<Uuid>,
    ) -> Result<Vec<AvailableTeacherRow>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                st.id as staff_id,
                COALESCE(NULLIF(u.display_name, ''), u.username, 'Teacher') as teacher_name,
                st.employee_id,
                st.department,
                st.designation
            FROM staff st
            JOIN users u ON st.user_id = u.id
            WHERE st.school_id = $1 AND st.status = 'ACTIVE'
              AND ($5::uuid IS NULL OR st.id != $5)
              -- Not teaching regular class in this period
              AND NOT EXISTS (
                  SELECT 1 FROM timetable_slots ts
                  WHERE ts.school_id = $1 AND ts.teacher_staff_id = st.id
                    AND ts.day_of_week = $2 AND ts.period_number = $3 AND ts.status = 'ACTIVE'
              )
              -- Not substituting another class in this period on this date
              AND NOT EXISTS (
                  SELECT 1 FROM substitutions sub
                  JOIN timetable_slots ts ON sub.timetable_slot_id = ts.id
                  WHERE sub.school_id = $1 AND sub.substitute_teacher_id = st.id
                    AND sub.date = $4 AND ts.period_number = $3 AND sub.status = 'ACTIVE'
              )
            ORDER BY teacher_name ASC
            "#,
        )
        .bind(school_id)
        .bind(day_of_week)
        .bind(period_number)
        .bind(date)
        .bind(exclude_staff_id)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| {
                let teacher_name: String = r.get("teacher_name");
                AvailableTeacherRow {
                    staff_id: r.get("staff_id"),
                    teacher_name: teacher_name.clone(),
                    name: teacher_name,
                    employee_id: r.get("employee_id"),
                    department: r.get("department"),
                    designation: r.get("designation"),
                }
            })
            .collect();

        Ok(list)
    }

    /// Retrieve today's effective schedule (with substitutions applied)
    pub async fn get_today_schedule(
        pool: &PgPool,
        school_id: Uuid,
        date: NaiveDate,
        class_section_id: Option<Uuid>,
        teacher_staff_id: Option<Uuid>,
    ) -> Result<Vec<CurrentDayScheduleRow>, AppError> {
        // Ensure today's schedule is materialized
        let _ = Self::sync_daily_timetable_for_date(pool, date).await;

        let rows = sqlx::query(
            r#"
            SELECT
                cdt.id, cdt.date::text as date, cdt.timetable_slot_id,
                cdt.class_section_id, c.name as class_name, s.name as section_name,
                cdt.day_of_week, cdt.period_number,
                cdt.start_time::text as start_time, cdt.end_time::text as end_time,
                subj.name as subject_name, subj.code as subject_code, subj.color as subject_color,
                cdt.effective_teacher_id,
                COALESCE(NULLIF(u_eff.display_name, ''), u_eff.username, 'Teacher') as effective_teacher_name,
                cdt.original_teacher_id,
                COALESCE(NULLIF(u_orig.display_name, ''), u_orig.username, 'Teacher') as original_teacher_name,
                COALESCE(cdt.co_teacher_ids, '{}') as co_teacher_ids,
                COALESCE(cdt.is_shared_subject, false) as is_shared_subject,
                cdt.is_substituted, cdt.substitution_id,
                r.name as room_name, cdt.status
            FROM current_day_timetable cdt
            JOIN class_sections cs ON cdt.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            JOIN subjects subj ON cdt.subject_id = subj.id
            JOIN staff st_eff ON cdt.effective_teacher_id = st_eff.id
            JOIN users u_eff ON st_eff.user_id = u_eff.id
            JOIN staff st_orig ON cdt.original_teacher_id = st_orig.id
            JOIN users u_orig ON st_orig.user_id = u_orig.id
            LEFT JOIN rooms r ON cdt.room_id = r.id
            WHERE cdt.school_id = $1 AND cdt.date = $2
              AND ($3::uuid IS NULL OR cdt.class_section_id = $3)
              AND ($4::uuid IS NULL OR cdt.effective_teacher_id = $4 OR cdt.co_teacher_ids @> ARRAY[$4::uuid])
            ORDER BY cdt.period_number ASC
            "#,
        )
        .bind(school_id)
        .bind(date)
        .bind(class_section_id)
        .bind(teacher_staff_id)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| CurrentDayScheduleRow {
                id: r.get("id"),
                date: r.get("date"),
                timetable_slot_id: r.get("timetable_slot_id"),
                class_section_id: r.get("class_section_id"),
                class_name: r.get("class_name"),
                section_name: r.get("section_name"),
                day_of_week: r.get("day_of_week"),
                period_number: r.get("period_number"),
                start_time: r.get("start_time"),
                end_time: r.get("end_time"),
                subject_name: r.get("subject_name"),
                subject_code: r.get("subject_code"),
                subject_color: r.get("subject_color"),
                effective_teacher_id: r.get("effective_teacher_id"),
                effective_teacher_name: r.get("effective_teacher_name"),
                original_teacher_id: r.get("original_teacher_id"),
                original_teacher_name: r.get("original_teacher_name"),
                co_teacher_ids: r.try_get("co_teacher_ids").unwrap_or_default(),
                is_shared_subject: r.try_get("is_shared_subject").unwrap_or(false),
                is_substituted: r.get("is_substituted"),
                substitution_id: r.get("substitution_id"),
                room_name: r.get("room_name"),
                status: r.get("status"),
            })
            .collect();

        Ok(list)
    }

    /// Reconciles today's schedule and inspects for substitution deadlocks & clashes
    pub async fn reconcile_and_verify_today_schedule(
        pool: &PgPool,
        school_id: Uuid,
        date: NaiveDate,
    ) -> Result<(), AppError> {
        // 1. Ensure daily materialized table is up to date with any master edits or morning changes
        let _ = Self::sync_daily_timetable_for_date(pool, date).await;

        // 2. Deadlock Detection: Check for substitute teachers double-booked across distinct class sections in the same period
        let clashes = sqlx::query_as::<_, (Uuid, i32, i64)>(
            r#"
            SELECT sub.substitute_teacher_id, ts.period_number, COUNT(*) as clash_count
            FROM substitutions sub
            JOIN timetable_slots ts ON sub.timetable_slot_id = ts.id
            WHERE sub.school_id = $1 AND sub.date = $2 AND sub.status = 'ACTIVE'
            GROUP BY sub.substitute_teacher_id, ts.period_number
            HAVING COUNT(*) > 1
            "#,
        )
        .bind(school_id)
        .bind(date)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for (sub_staff_id, period, clash_count) in clashes {
            tracing::warn!(
                "Substitution deadlock clash: Substitute staff {} is assigned to {} classes in period {} on {}",
                sub_staff_id, clash_count, period, date
            );

            let field_identifier = format!("substitution_deadlock_{}_{}", date, period);
            let ticket_exists: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM data_change_tickets
                    WHERE school_id = $1 AND ticket_type = 'SUBSTITUTION_DEADLOCK'
                      AND status = 'PENDING' AND field_name = $2
                )
                "#,
            )
            .bind(school_id)
            .bind(&field_identifier)
            .fetch_one(pool)
            .await
            .unwrap_or(false);

            if !ticket_exists {
                let user_id_opt: Option<Uuid> =
                    sqlx::query_scalar("SELECT user_id FROM staff WHERE id = $1")
                        .bind(sub_staff_id)
                        .fetch_optional(pool)
                        .await
                        .unwrap_or(None);

                if let Some(user_id) = user_id_opt {
                    let ticket_number = format!(
                        "TKT-DEADLOCK-{}-{}",
                        date.format("%Y%m%d"),
                        &Uuid::new_v4().to_string()[..6].to_uppercase()
                    );
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO data_change_tickets (
                            school_id, user_id, ticket_number, ticket_type, status,
                            field_name, new_value, reason, priority, created_at, updated_at
                        ) VALUES ($1, $2, $3, 'SUBSTITUTION_DEADLOCK', 'PENDING', $4, $5, $6, 'HIGH', NOW(), NOW())
                        "#,
                    )
                    .bind(school_id)
                    .bind(user_id)
                    .bind(&ticket_number)
                    .bind(&field_identifier)
                    .bind(format!("clash_count_{}", clash_count))
                    .bind(format!(
                        "Substitution deadlock detected: Substitute teacher is double-booked across {} classes in Period {} on {}. Immediate re-assignment required.",
                        clash_count, period, date
                    ))
                    .execute(pool)
                    .await;
                }
            }
        }

        Ok(())
    }

    /// Invokes PostgreSQL stored procedure to sync daily schedule at 01:30 AM or on demand
    pub async fn sync_daily_timetable_for_date(
        pool: &PgPool,
        date: NaiveDate,
    ) -> Result<i32, AppError> {
        let count: i32 = sqlx::query_scalar("SELECT sync_daily_timetable_for_date($1)")
            .bind(date)
            .fetch_one(pool)
            .await?;

        Ok(count)
    }

    /// Retrieve school timetable configuration
    pub async fn get_timetable_config(
        pool: &PgPool,
        school_id: Uuid,
    ) -> Result<serde_json::Value, AppError> {
        let row: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT value FROM school_settings WHERE school_id = $1 AND key = 'timetable_config'",
        )
        .bind(school_id)
        .fetch_optional(pool)
        .await?;

        if let Some(val) = row {
            Ok(val)
        } else {
            Ok(serde_json::json!({
                "periods_per_day": 8,
                "days_per_week": 6,
                "period_duration_minutes": 45,
                "lunch_after_period": 4,
                "lunch_duration_minutes": 30,
                "start_time": "08:30:00",
                "max_teacher_daily_workload": 5,
                "period_timings": [
                    { "period_number": 1, "start_time": "08:30:00", "end_time": "09:15:00", "is_break": false, "label": "08:30 - 09:15" },
                    { "period_number": 2, "start_time": "09:15:00", "end_time": "10:00:00", "is_break": false, "label": "09:15 - 10:00" },
                    { "period_number": 3, "start_time": "10:15:00", "end_time": "11:00:00", "is_break": false, "label": "10:15 - 11:00" },
                    { "period_number": 4, "start_time": "11:00:00", "end_time": "11:45:00", "is_break": false, "label": "11:00 - 11:45" },
                    { "period_number": 5, "start_time": "12:15:00", "end_time": "13:00:00", "is_break": false, "label": "12:15 - 13:00" },
                    { "period_number": 6, "start_time": "13:00:00", "end_time": "13:45:00", "is_break": false, "label": "13:00 - 13:45" },
                    { "period_number": 7, "start_time": "14:00:00", "end_time": "14:45:00", "is_break": false, "label": "14:00 - 14:45" },
                    { "period_number": 8, "start_time": "14:45:00", "end_time": "15:30:00", "is_break": false, "label": "14:45 - 15:30" }
                ]
            }))
        }
    }

    /// Save school timetable configuration
    pub async fn save_timetable_config(
        pool: &PgPool,
        school_id: Uuid,
        config: serde_json::Value,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO school_settings (school_id, key, value, updated_at)
            VALUES ($1, 'timetable_config', $2, NOW())
            ON CONFLICT (school_id, key) DO UPDATE SET
                value = EXCLUDED.value,
                updated_at = NOW()
            "#,
        )
        .bind(school_id)
        .bind(config)
        .execute(pool)
        .await?;

        Ok(())
    }
}
