use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::error::AppError;

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
    pub elective_group_id: Option<Uuid>,
    pub is_elective_slot: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherClashRow {
    pub class_name: String,
    pub section_name: String,
    pub subject_name: String,
    pub period_number: i32,
    pub day_of_week: i32,
    pub is_shared: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomClashRow {
    pub class_name: String,
    pub section_name: String,
    pub subject_name: String,
    pub room_name: String,
    pub period_number: i32,
    pub day_of_week: i32,
    pub is_shared: bool,
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

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SubstitutionNotificationContext {
    pub period_number: i32,
    pub start_time: String,
    pub end_time: String,
    pub class_name: String,
    pub section_name: String,
    pub subject_name: String,
    pub subject_code: String,
    pub orig_staff_id: Uuid,
    pub orig_user_id: Uuid,
    pub orig_teacher_name: String,
    pub orig_email: Option<String>,
    pub orig_phone: Option<String>,
    pub sub_staff_id: Uuid,
    pub sub_user_id: Uuid,
    pub sub_teacher_name: String,
    pub sub_email: Option<String>,
    pub sub_phone: Option<String>,
    pub school_name: String,
}

pub struct TimetableRepository;

impl TimetableRepository {
    // =========================================================================
    // QUERY INTENT:
    // 1. What: Fetches the weekly schedule slots for a specific class section.
    // 2. Joins & Foreign Keys:
    //    - timetable_slots.class_section_id -> class_sections.id
    //    - class_sections.class_id -> classes.id & class_sections.section_id -> sections.id
    //    - timetable_slots.subject_id -> subjects.id
    //    - timetable_slots.teacher_staff_id -> staff.id (CRITICAL: References staff entity,
    //      then staff.user_id -> users.id to fetch user display name)
    //    - timetable_slots.room_id -> rooms.id (LEFT JOIN optional physical room)
    // 3. Why & Correctness:
    //    - Scoped strictly by school_id ($1) and class_section_id ($2) with status = 'ACTIVE'
    //    - Returns slots ordered chronologically by day_of_week and period_number
    // =========================================================================
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
                ts.elective_group_id,
                COALESCE(ts.is_elective_slot, false) as is_elective_slot,
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
                elective_group_id: r.try_get("elective_group_id").ok().flatten(),
                is_elective_slot: r.try_get("is_elective_slot").unwrap_or(false),
                status: r.get("status"),
            })
            .collect();

        Ok(slots)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Fetches scheduled teaching slots for a teacher (primary or co-teacher).
    // 2. Joins & Foreign Keys:
    //    - Supports both staff.id and users.id interchangeably for teacher_staff_or_user_id ($2)
    //      via (ts.teacher_staff_id = $2 OR ts.teacher_staff_id IN (SELECT id FROM staff WHERE user_id = $2 AND school_id = $1))
    //    - ts.teacher_staff_id -> staff.id -> users.id for faculty name
    // 3. Why & Correctness:
    //    - Eliminates identity confusion between users.id (auth login) and staff.id (record ID)
    //    - Allows optional filtering by day_of_week ($3)
    // =========================================================================
    pub async fn get_teacher_slots(
        pool: &PgPool,
        school_id: Uuid,
        teacher_staff_or_user_id: Uuid,
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
                ts.elective_group_id,
                COALESCE(ts.is_elective_slot, false) as is_elective_slot,
                ts.status
            FROM timetable_slots ts
            JOIN class_sections cs ON ts.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            JOIN subjects subj ON ts.subject_id = subj.id
            JOIN staff st ON ts.teacher_staff_id = st.id
            JOIN users u ON st.user_id = u.id
            LEFT JOIN rooms r ON ts.room_id = r.id
            WHERE ts.school_id = $1
              AND (
                  ts.teacher_staff_id = $2
                  OR ts.teacher_staff_id IN (SELECT id FROM staff WHERE user_id = $2 AND school_id = $1)
                  OR ts.co_teacher_staff_ids @> ARRAY[$2::uuid]
                  OR ts.co_teacher_staff_ids @> ARRAY[(SELECT id FROM staff WHERE user_id = $2 AND school_id = $1)]
              )
              AND ($3::int IS NULL OR ts.day_of_week = $3)
              AND ts.status = 'ACTIVE'
            ORDER BY ts.day_of_week ASC, ts.period_number ASC
            "#,
        )
        .bind(school_id)
        .bind(teacher_staff_or_user_id)
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
                elective_group_id: r.try_get("elective_group_id").ok().flatten(),
                is_elective_slot: r.try_get("is_elective_slot").unwrap_or(false),
                status: r.get("status"),
            })
            .collect();

        Ok(slots)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Detects whether a teacher is already booked in another class section during the same period.
    // 2. Joins & Foreign Keys:
    //    - timetable_slots.class_section_id -> class_sections.id -> classes / sections
    //    - timetable_slots.subject_id -> subjects.id
    // 3. Why & Correctness:
    //    - Checks primary teacher_staff_id and co_teacher_staff_ids array
    //    - Excludes currently edited slot ($5) or section ($6) if specified
    //    - Multi-tenant isolated on school_id ($1)
    // =========================================================================
    pub async fn check_teacher_clash(
        pool: &PgPool,
        school_id: Uuid,
        day_of_week: i32,
        period_number: i32,
        teacher_staff_id: Uuid,
        exclude_slot_id: Option<Uuid>,
        class_section_id: Option<Uuid>,
    ) -> Result<Vec<TeacherClashRow>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                c.name as class_name, s.name as section_name, subj.name as subject_name,
                ts.period_number, ts.day_of_week, COALESCE(ts.is_shared, false) as is_shared
            FROM timetable_slots ts
            JOIN class_sections cs ON ts.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            JOIN subjects subj ON ts.subject_id = subj.id
            WHERE ts.school_id = $1
              AND ts.day_of_week = $2
              AND ts.period_number = $3
              AND (ts.teacher_staff_id = $4 OR ts.co_teacher_staff_ids @> ARRAY[$4::uuid])
              AND ($5::uuid IS NULL OR ts.id != $5)
              AND ($6::uuid IS NULL OR ts.class_section_id != $6)
              AND ts.status = 'ACTIVE'
            "#,
        )
        .bind(school_id)
        .bind(day_of_week)
        .bind(period_number)
        .bind(teacher_staff_id)
        .bind(exclude_slot_id)
        .bind(class_section_id)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| TeacherClashRow {
                class_name: r.get("class_name"),
                section_name: r.get("section_name"),
                subject_name: r.get("subject_name"),
                period_number: r.get("period_number"),
                day_of_week: r.get("day_of_week"),
                is_shared: r.get("is_shared"),
            })
            .collect();

        Ok(list)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Detects whether a physical classroom/lab is already booked in another class section.
    // 2. Joins & Foreign Keys:
    //    - timetable_slots.room_id -> rooms.id
    //    - timetable_slots.class_section_id -> class_sections.id -> classes / sections
    // 3. Why & Correctness:
    //    - Prevents double-booking labs/rooms in the same period
    //    - Allows shared room usage if marked is_shared = true
    // =========================================================================
    pub async fn check_room_clash(
        pool: &PgPool,
        school_id: Uuid,
        day_of_week: i32,
        period_number: i32,
        room_id: Uuid,
        exclude_slot_id: Option<Uuid>,
        class_section_id: Option<Uuid>,
    ) -> Result<Vec<RoomClashRow>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                c.name as class_name, s.name as section_name, subj.name as subject_name,
                r.name as room_name, ts.period_number, ts.day_of_week,
                COALESCE(ts.is_shared, false) as is_shared
            FROM timetable_slots ts
            JOIN class_sections cs ON ts.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            JOIN subjects subj ON ts.subject_id = subj.id
            JOIN rooms r ON ts.room_id = r.id
            WHERE ts.school_id = $1
              AND ts.day_of_week = $2
              AND ts.period_number = $3
              AND ts.room_id = $4
              AND ($5::uuid IS NULL OR ts.id != $5)
              AND ($6::uuid IS NULL OR ts.class_section_id != $6)
              AND ts.status = 'ACTIVE'
            "#,
        )
        .bind(school_id)
        .bind(day_of_week)
        .bind(period_number)
        .bind(room_id)
        .bind(exclude_slot_id)
        .bind(class_section_id)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| RoomClashRow {
                class_name: r.get("class_name"),
                section_name: r.get("section_name"),
                subject_name: r.get("subject_name"),
                room_name: r.get("room_name"),
                period_number: r.get("period_number"),
                day_of_week: r.get("day_of_week"),
                is_shared: r.get("is_shared"),
            })
            .collect();

        Ok(list)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Obtains an exclusive row-level lock (FOR UPDATE) on class_sections during timetable batch upserts.
    // 2. Joins & Foreign Keys: class_sections.id
    // 3. Why & Correctness:
    //    - Eliminates concurrent transaction deadlocks when multiple administrators save schedules simultaneously
    // =========================================================================
    pub async fn lock_class_section(
        conn: &mut Transaction<'_, Postgres>,
        class_section_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        let locked_id =
            sqlx::query_scalar("SELECT id FROM class_sections WHERE id = $1 FOR UPDATE")
                .bind(class_section_id)
                .fetch_optional(&mut **conn)
                .await?;

        Ok(locked_id)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Checks if a timetable slot exists in a school before updating.
    // 2. Joins & Foreign Keys: timetable_slots.id, timetable_slots.school_id
    // 3. Why & Correctness: Enforces multi-tenant ownership before mutating existing slot.
    // =========================================================================
    pub async fn slot_exists(
        conn: &mut Transaction<'_, Postgres>,
        school_id: Uuid,
        slot_id: Uuid,
    ) -> Result<bool, AppError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM timetable_slots WHERE id = $1 AND school_id = $2)",
        )
        .bind(slot_id)
        .bind(school_id)
        .fetch_one(&mut **conn)
        .await?;

        Ok(exists)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Updates an existing timetable slot's period timings, subject, faculty, and room.
    // 2. Joins & Foreign Keys:
    //    - teacher_staff_id references staff.id
    //    - subject_id references subjects.id
    //    - room_id references rooms.id
    // 3. Why & Correctness:
    //    - Updates slot safely within active transaction and sets updated_at = NOW()
    // =========================================================================
    pub async fn update_slot(
        conn: &mut Transaction<'_, Postgres>,
        school_id: Uuid,
        slot_id: Uuid,
        day_of_week: i32,
        period_number: i32,
        start_time: NaiveTime,
        end_time: NaiveTime,
        subject_id: Uuid,
        teacher_staff_id: Uuid,
        co_teacher_staff_ids: &[Uuid],
        is_shared: bool,
        room_id: Option<Uuid>,
        elective_group_id: Option<Uuid>,
        is_elective_slot: bool,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE timetable_slots
            SET
                day_of_week = $1,
                period_number = $2,
                start_time = $3,
                end_time = $4,
                subject_id = $5,
                teacher_staff_id = $6,
                co_teacher_staff_ids = $7,
                is_shared = $8,
                room_id = $9,
                elective_group_id = $10,
                is_elective_slot = $11,
                status = 'ACTIVE',
                updated_at = NOW()
            WHERE id = $12 AND school_id = $13
            "#,
        )
        .bind(day_of_week)
        .bind(period_number)
        .bind(start_time)
        .bind(end_time)
        .bind(subject_id)
        .bind(teacher_staff_id)
        .bind(co_teacher_staff_ids)
        .bind(is_shared)
        .bind(room_id)
        .bind(elective_group_id)
        .bind(is_elective_slot)
        .bind(slot_id)
        .bind(school_id)
        .execute(&mut **conn)
        .await?;

        Ok(())
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Inserts a new timetable slot or updates conflicting slot on unique constraint.
    // 2. Joins & Foreign Keys:
    //    - class_section_id -> class_sections.id
    //    - academic_year_id -> academic_years.id
    //    - teacher_staff_id -> staff.id
    //    - subject_id -> subjects.id
    // 3. Why & Correctness:
    //    - Idempotent upsert matching ON CONFLICT (class_section_id, day_of_week, period_number, subject_id)
    // =========================================================================
    pub async fn insert_slot_on_conflict(
        conn: &mut Transaction<'_, Postgres>,
        school_id: Uuid,
        academic_year_id: Uuid,
        class_section_id: Uuid,
        day_of_week: i32,
        period_number: i32,
        start_time: NaiveTime,
        end_time: NaiveTime,
        subject_id: Uuid,
        teacher_staff_id: Uuid,
        co_teacher_staff_ids: &[Uuid],
        is_shared: bool,
        room_id: Option<Uuid>,
        elective_group_id: Option<Uuid>,
        is_elective_slot: bool,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO timetable_slots (
                school_id, academic_year_id, class_section_id,
                day_of_week, period_number, start_time, end_time,
                subject_id, teacher_staff_id, co_teacher_staff_ids, is_shared, room_id,
                elective_group_id, is_elective_slot, status
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, 'ACTIVE')
            ON CONFLICT (class_section_id, day_of_week, period_number, subject_id) DO UPDATE SET
                start_time = EXCLUDED.start_time,
                end_time = EXCLUDED.end_time,
                teacher_staff_id = EXCLUDED.teacher_staff_id,
                co_teacher_staff_ids = EXCLUDED.co_teacher_staff_ids,
                is_shared = EXCLUDED.is_shared,
                room_id = EXCLUDED.room_id,
                elective_group_id = EXCLUDED.elective_group_id,
                is_elective_slot = EXCLUDED.is_elective_slot,
                status = 'ACTIVE',
                updated_at = NOW()
            "#,
        )
        .bind(school_id)
        .bind(academic_year_id)
        .bind(class_section_id)
        .bind(day_of_week)
        .bind(period_number)
        .bind(start_time)
        .bind(end_time)
        .bind(subject_id)
        .bind(teacher_staff_id)
        .bind(co_teacher_staff_ids)
        .bind(is_shared)
        .bind(room_id)
        .bind(elective_group_id)
        .bind(is_elective_slot)
        .execute(&mut **conn)
        .await?;

        Ok(())
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Deletes a timetable slot permanently.
    // 2. Joins & Foreign Keys: timetable_slots.id, timetable_slots.school_id
    // 3. Why & Correctness: Multi-tenant isolated deletion.
    // =========================================================================
    pub async fn delete_slot(
        pool: &PgPool,
        school_id: Uuid,
        slot_id: Uuid,
    ) -> Result<u64, AppError> {
        let res = sqlx::query("DELETE FROM timetable_slots WHERE id = $1 AND school_id = $2")
            .bind(slot_id)
            .bind(school_id)
            .execute(pool)
            .await?;

        Ok(res.rows_affected())
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Fetches all active teacher commitments across all other sections in the school.
    // 2. Joins & Foreign Keys: timetable_slots.teacher_staff_id (staff.id)
    // 3. Why & Correctness: Feeds into timetable generator constraint solver to avoid faculty clashes.
    // =========================================================================
    pub async fn get_existing_teacher_slots_for_generator(
        pool: &PgPool,
        school_id: Uuid,
        exclude_class_section_id: Uuid,
    ) -> Result<Vec<(Uuid, i32, i32)>, AppError> {
        let rows = sqlx::query_as::<_, (Uuid, i32, i32)>(
            r#"
            SELECT teacher_staff_id, day_of_week, period_number
            FROM timetable_slots
            WHERE school_id = $1 AND class_section_id != $2 AND status = 'ACTIVE'
            "#,
        )
        .bind(school_id)
        .bind(exclude_class_section_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Fetches all active room bookings across all other sections in the school.
    // 2. Joins & Foreign Keys: timetable_slots.room_id (rooms.id)
    // 3. Why & Correctness: Feeds into timetable generator constraint solver to avoid room clashes.
    // =========================================================================
    pub async fn get_existing_room_slots_for_generator(
        pool: &PgPool,
        school_id: Uuid,
        exclude_class_section_id: Uuid,
    ) -> Result<Vec<(Uuid, i32, i32)>, AppError> {
        let rows = sqlx::query_as::<_, (Uuid, i32, i32)>(
            r#"
            SELECT room_id, day_of_week, period_number
            FROM timetable_slots
            WHERE school_id = $1 AND class_section_id != $2 AND room_id IS NOT NULL AND status = 'ACTIVE'
            "#,
        )
        .bind(school_id)
        .bind(exclude_class_section_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Lists teacher substitutions for a given date.
    // 2. Joins & Foreign Keys:
    //    - substitutions.timetable_slot_id -> timetable_slots.id
    //    - timetable_slots.class_section_id -> class_sections.id -> classes / sections
    //    - timetable_slots.subject_id -> subjects.id
    //    - substitutions.original_teacher_id -> staff.id -> users.id (Original faculty name)
    //    - substitutions.substitute_teacher_id -> staff.id -> users.id (Substitute faculty name)
    // 3. Why & Correctness:
    //    - Accurately tracks absent teachers replaced by substitutes on a specific calendar day.
    // =========================================================================
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

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Fetches slot metadata (id, day_of_week, period_number, original teacher staff_id) for substitution creation.
    // 2. Joins & Foreign Keys: timetable_slots.id, timetable_slots.school_id
    // 3. Why & Correctness: Ensures slot belongs to the multi-tenant school before substituting.
    // =========================================================================
    pub async fn get_slot_for_substitution(
        pool: &PgPool,
        school_id: Uuid,
        slot_id: Uuid,
    ) -> Result<Option<(Uuid, i32, i32, Uuid)>, AppError> {
        let slot = sqlx::query_as::<_, (Uuid, i32, i32, Uuid)>(
            "SELECT id, day_of_week, period_number, teacher_staff_id FROM timetable_slots WHERE id = $1 AND school_id = $2"
        )
        .bind(slot_id)
        .bind(school_id)
        .fetch_optional(pool)
        .await?;

        Ok(slot)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Verifies if a proposed substitute teacher already has an active regular class at that period.
    // 2. Joins & Foreign Keys: timetable_slots.teacher_staff_id (staff.id)
    // 3. Why & Correctness: Prevents assigning a teacher who is already lecturing their own class.
    // =========================================================================
    pub async fn check_regular_clash_for_sub(
        pool: &PgPool,
        school_id: Uuid,
        teacher_staff_id: Uuid,
        day_of_week: i32,
        period_number: i32,
    ) -> Result<bool, AppError> {
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
        .bind(teacher_staff_id)
        .bind(day_of_week)
        .bind(period_number)
        .fetch_one(pool)
        .await?;

        Ok(has_regular_clash)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Verifies if a proposed substitute teacher is already covering another class at the same date & period.
    // 2. Joins & Foreign Keys: substitutions.substitute_teacher_id (staff.id) -> timetable_slots.period_number
    // 3. Why & Correctness: Prevents double-booking substitute teachers across classrooms.
    // =========================================================================
    pub async fn check_sub_clash_for_sub(
        pool: &PgPool,
        school_id: Uuid,
        teacher_staff_id: Uuid,
        date: NaiveDate,
        period_number: i32,
    ) -> Result<bool, AppError> {
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
        .bind(teacher_staff_id)
        .bind(date)
        .bind(period_number)
        .fetch_one(pool)
        .await?;

        Ok(has_sub_clash)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Inserts or updates a substitution for a timetable slot and date.
    // 2. Joins & Foreign Keys:
    //    - original_teacher_id -> staff.id
    //    - substitute_teacher_id -> staff.id
    //    - timetable_slot_id -> timetable_slots.id
    //    - created_by -> users.id (The user who scheduled the substitution)
    // 3. Why & Correctness:
    //    - Idempotent upsert on ON CONFLICT (timetable_slot_id, date)
    // =========================================================================
    pub async fn insert_or_update_substitution(
        pool: &PgPool,
        school_id: Uuid,
        date: NaiveDate,
        slot_id: Uuid,
        orig_teacher_id: Uuid,
        sub_teacher_id: Uuid,
        reason: Option<String>,
        user_id: Uuid,
    ) -> Result<Uuid, AppError> {
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
        .bind(sub_teacher_id)
        .bind(reason)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(sub_id)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Deletes an active substitution.
    // 2. Joins & Foreign Keys: substitutions.id, substitutions.school_id
    // 3. Why & Correctness: Multi-tenant isolated deletion.
    // =========================================================================
    pub async fn delete_substitution(
        pool: &PgPool,
        school_id: Uuid,
        sub_id: Uuid,
    ) -> Result<u64, AppError> {
        let res = sqlx::query("DELETE FROM substitutions WHERE id = $1 AND school_id = $2")
            .bind(sub_id)
            .bind(school_id)
            .execute(pool)
            .await?;

        Ok(res.rows_affected())
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Finds all teaching staff available with ZERO clashes for a specific slot on a date.
    // 2. Joins & Foreign Keys:
    //    - staff.user_id -> users.id (To obtain faculty name)
    //    - NOT EXISTS in timetable_slots where teacher_staff_id = staff.id (Regular clash check)
    //    - NOT EXISTS in substitutions where substitute_teacher_id = staff.id (Sub clash check)
    // 3. Why & Correctness:
    //    - Ensures suggestions for substitute teachers are strictly verified against both schedules.
    // =========================================================================
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
                COALESCE(st.employee_id, '') as employee_id,
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

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Fetches today's materialized schedule with effective teacher replacements applied.
    // 2. Joins & Foreign Keys:
    //    - current_day_timetable.class_section_id -> class_sections.id -> classes / sections
    //    - current_day_timetable.subject_id -> subjects.id
    //    - current_day_timetable.effective_teacher_id -> staff.id -> users.id (Effective teacher)
    //    - current_day_timetable.original_teacher_id -> staff.id -> users.id (Original teacher)
    //    - current_day_timetable.room_id -> rooms.id
    // 3. Why & Correctness:
    //    - Reads from the materialized current_day_timetable table for instantaneous O(1) daily schedule queries.
    // =========================================================================
    pub async fn get_today_schedule(
        pool: &PgPool,
        school_id: Uuid,
        date: NaiveDate,
        class_section_id: Option<Uuid>,
        teacher_staff_id: Option<Uuid>,
    ) -> Result<Vec<CurrentDayScheduleRow>, AppError> {
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

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Detects deadlock clashes where a substitute teacher is double-booked across distinct class sections in the same period.
    // 2. Joins & Foreign Keys: substitutions.timetable_slot_id -> timetable_slots.id
    // 3. Why & Correctness: Enforces deadlock resolution and triggers alert tickets.
    // =========================================================================
    pub async fn get_substitution_deadlock_clashes(
        pool: &PgPool,
        school_id: Uuid,
        date: NaiveDate,
    ) -> Result<Vec<(Uuid, i32, i64)>, AppError> {
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
        .await?;

        Ok(clashes)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Checks if a pending data change ticket already exists for a deadlock clash.
    // 2. Joins & Foreign Keys: data_change_tickets.school_id, data_change_tickets.field_name
    // 3. Why & Correctness: Prevents duplicate administrative deadlock tickets.
    // =========================================================================
    pub async fn deadlock_ticket_exists(
        pool: &PgPool,
        school_id: Uuid,
        field_identifier: &str,
    ) -> Result<bool, AppError> {
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM data_change_tickets
                WHERE school_id = $1 AND ticket_type = 'SUBSTITUTION_DEADLOCK'
                  AND status = 'PENDING' AND field_name = $2
            )
            "#,
        )
        .bind(school_id)
        .bind(field_identifier)
        .fetch_one(pool)
        .await?;

        Ok(exists)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Retrieves the auth users.id associated with a staff.id.
    // 2. Joins & Foreign Keys: staff.user_id -> users.id
    // 3. Why & Correctness: Resolves employee identity to auth user for ticketing and notification delivery.
    // =========================================================================
    pub async fn get_staff_user_id(
        pool: &PgPool,
        staff_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        let user_id: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM staff WHERE id = $1")
            .bind(staff_id)
            .fetch_optional(pool)
            .await?;

        Ok(user_id)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Inserts a high-priority deadlock alert ticket for administrative intervention.
    // 2. Joins & Foreign Keys: data_change_tickets.school_id, data_change_tickets.user_id (users.id)
    // 3. Why & Correctness: Surfaces schedule collisions in the admin inbox immediately.
    // =========================================================================
    pub async fn insert_deadlock_ticket(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        ticket_number: &str,
        field_identifier: &str,
        clash_count: i64,
        reason: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO data_change_tickets (
                school_id, user_id, ticket_number, ticket_type, status,
                field_name, new_value, reason, priority, created_at, updated_at
            ) VALUES ($1, $2, $3, 'SUBSTITUTION_DEADLOCK', 'PENDING', $4, $5, $6, 'HIGH', NOW(), NOW())
            "#,
        )
        .bind(school_id)
        .bind(user_id)
        .bind(ticket_number)
        .bind(field_identifier)
        .bind(format!("clash_count_{}", clash_count))
        .bind(reason)
        .execute(pool)
        .await?;

        Ok(())
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Invokes PostgreSQL stored procedure to materialize the daily schedule for a date.
    // 2. Joins & Foreign Keys: Internal stored function sync_daily_timetable_for_date.
    // 3. Why & Correctness: Ensures substitutions and weekly slots are baked into current_day_timetable.
    // =========================================================================
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

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Fetches school timetable configuration JSON from school_settings.
    // 2. Joins & Foreign Keys: school_settings.school_id
    // 3. Why & Correctness: Per-tenant timetable periods and break configuration.
    // =========================================================================
    pub async fn get_timetable_config(
        pool: &PgPool,
        school_id: Uuid,
    ) -> Result<Option<serde_json::Value>, AppError> {
        let row: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT value FROM school_settings WHERE school_id = $1 AND key = 'timetable_config'",
        )
        .bind(school_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Saves school timetable configuration JSON into school_settings.
    // 2. Joins & Foreign Keys: school_settings (school_id, key) unique constraint
    // 3. Why & Correctness: Idempotent upsert of school-wide timetable settings.
    // =========================================================================
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

    // =========================================================================
    // QUERY INTENT:
    // 1. What: Fetches complete notification context for a newly assigned substitution.
    // 2. Joins & Foreign Keys:
    //    - timetable_slots.class_section_id -> class_sections.id -> classes / sections
    //    - timetable_slots.subject_id -> subjects.id
    //    - timetable_slots.school_id -> schools.id
    //    - timetable_slots.teacher_staff_id -> staff st_orig ON st_orig.id -> users u_orig (Original faculty)
    //    - st_sub ON st_sub.id = $2 -> users u_sub (Substitute faculty)
    // 3. Why & Correctness:
    //    - Accurately gathers emails, phones, and class metadata to dispatch automated Email & WhatsApp alerts.
    // =========================================================================
    pub async fn get_substitution_notification_context(
        pool: &PgPool,
        school_id: Uuid,
        slot_id: Uuid,
        substitute_staff_id: Uuid,
    ) -> Result<Option<SubstitutionNotificationContext>, AppError> {
        let row_opt = sqlx::query(
            r#"
            SELECT
                ts.period_number,
                ts.start_time::text as start_time,
                ts.end_time::text as end_time,
                c.name as class_name,
                s.name as section_name,
                subj.name as subject_name,
                subj.code as subject_code,
                -- Original faculty
                st_orig.id as orig_staff_id,
                u_orig.id as orig_user_id,
                COALESCE(NULLIF(u_orig.display_name, ''), u_orig.username, 'Teacher') as orig_teacher_name,
                u_orig.email as orig_email,
                u_orig.phone as orig_phone,
                -- Substitute faculty
                st_sub.id as sub_staff_id,
                u_sub.id as sub_user_id,
                COALESCE(NULLIF(u_sub.display_name, ''), u_sub.username, 'Teacher') as sub_teacher_name,
                u_sub.email as sub_email,
                u_sub.phone as sub_phone,
                -- School
                sch.name as school_name
            FROM timetable_slots ts
            JOIN class_sections cs ON ts.class_section_id = cs.id
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            JOIN subjects subj ON ts.subject_id = subj.id
            JOIN schools sch ON ts.school_id = sch.id
            JOIN staff st_orig ON ts.teacher_staff_id = st_orig.id
            JOIN users u_orig ON st_orig.user_id = u_orig.id
            JOIN staff st_sub ON st_sub.id = $2
            JOIN users u_sub ON st_sub.user_id = u_sub.id
            WHERE ts.id = $1 AND ts.school_id = $3
            "#,
        )
        .bind(slot_id)
        .bind(substitute_staff_id)
        .bind(school_id)
        .fetch_optional(pool)
        .await?;

        let ctx = row_opt.map(|row| SubstitutionNotificationContext {
            period_number: row.get("period_number"),
            start_time: row.get("start_time"),
            end_time: row.get("end_time"),
            class_name: row.get("class_name"),
            section_name: row.get("section_name"),
            subject_name: row.get("subject_name"),
            subject_code: row.get("subject_code"),
            orig_staff_id: row.get("orig_staff_id"),
            orig_user_id: row.get("orig_user_id"),
            orig_teacher_name: row.get("orig_teacher_name"),
            orig_email: row.get("orig_email"),
            orig_phone: row.get("orig_phone"),
            sub_staff_id: row.get("sub_staff_id"),
            sub_user_id: row.get("sub_user_id"),
            sub_teacher_name: row.get("sub_teacher_name"),
            sub_email: row.get("sub_email"),
            sub_phone: row.get("sub_phone"),
            school_name: row.get("school_name"),
        });

        Ok(ctx)
    }
}
