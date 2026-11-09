use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use tracing::info;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::timetable::generator::{
    AutoGenerateRequest, AutoGenerateResponse, TimetableGenerator,
};
use crate::modules::academic::timetable::repository::TimetableRepository;

pub use crate::modules::academic::timetable::repository::{
    AvailableTeacherRow, CurrentDayScheduleRow, SubstitutionRow, TimetableSlotRow,
};

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
    pub elective_group_id: Option<Uuid>,
    #[serde(default)]
    pub is_elective_slot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckClashRequest {
    pub day_of_week: i32,
    pub period_number: i32,
    pub teacher_staff_id: Uuid,
    pub room_id: Option<Uuid>,
    pub exclude_slot_id: Option<Uuid>,
    pub class_section_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClashDetail {
    pub clash_type: String,
    pub message: String,
    pub class_name: String,
    pub section_name: String,
    pub subject_name: String,
    pub period_number: i32,
    pub day_of_week: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckClashResponse {
    pub has_clash: bool,
    pub clashes: Vec<ClashDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSaveSlotsPayload {
    pub class_section_id: Uuid,
    pub academic_year_id: Option<Uuid>,
    pub slots: Vec<SaveSlotInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSubstitutionInput {
    pub date: String, // YYYY-MM-DD
    pub timetable_slot_id: Uuid,
    pub substitute_teacher_id: Uuid,
    pub reason: Option<String>,
}

pub struct TimetableService;

impl TimetableService {
    /// Retrieve full weekly timetable for a given class section
    pub async fn get_section_timetable(
        pool: &PgPool,
        school_id: Uuid,
        class_section_id: Uuid,
    ) -> Result<Vec<TimetableSlotRow>, AppError> {
        TimetableRepository::get_section_timetable(pool, school_id, class_section_id).await
    }

    /// Retrieve scheduled slots for a teacher on a day (or all days)
    pub async fn get_teacher_slots(
        pool: &PgPool,
        school_id: Uuid,
        teacher_id: Uuid,
        day_of_week: Option<i32>,
    ) -> Result<Vec<TimetableSlotRow>, AppError> {
        TimetableRepository::get_teacher_slots(pool, school_id, teacher_id, day_of_week).await
    }

    /// Check for clashes across class sections for a given day and period
    pub async fn check_clashes(
        pool: &PgPool,
        school_id: Uuid,
        req: &CheckClashRequest,
    ) -> Result<CheckClashResponse, AppError> {
        let mut clashes = Vec::new();

        // 1. Teacher clash: check if teacher is assigned to another class section in the same slot
        let teacher_rows = TimetableRepository::check_teacher_clash(
            pool,
            school_id,
            req.day_of_week,
            req.period_number,
            req.teacher_staff_id,
            req.exclude_slot_id,
            req.class_section_id,
        )
        .await?;

        for r in teacher_rows {
            if !r.is_shared {
                clashes.push(ClashDetail {
                    clash_type: "TEACHER".into(),
                    message: format!(
                        "Teacher is already scheduled for {} - {} ({}) in Period {}",
                        r.class_name, r.section_name, r.subject_name, req.period_number
                    ),
                    class_name: r.class_name,
                    section_name: r.section_name,
                    subject_name: r.subject_name,
                    period_number: req.period_number,
                    day_of_week: req.day_of_week,
                });
            }
        }

        // 2. Room clash: check if room is booked by another class section in the same slot
        if let Some(room_id) = req.room_id {
            let room_rows = TimetableRepository::check_room_clash(
                pool,
                school_id,
                req.day_of_week,
                req.period_number,
                room_id,
                req.exclude_slot_id,
                req.class_section_id,
            )
            .await?;

            for r in room_rows {
                if !r.is_shared {
                    clashes.push(ClashDetail {
                        clash_type: "ROOM".into(),
                        message: format!(
                            "Room {} is already occupied by {} - {} ({}) in Period {}",
                            r.room_name,
                            r.class_name,
                            r.section_name,
                            r.subject_name,
                            req.period_number
                        ),
                        class_name: r.class_name,
                        section_name: r.section_name,
                        subject_name: r.subject_name,
                        period_number: req.period_number,
                        day_of_week: req.day_of_week,
                    });
                }
            }
        }

        let has_clash = !clashes.is_empty();
        Ok(CheckClashResponse { has_clash, clashes })
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
        let _ = TimetableRepository::lock_class_section(&mut tx, payload.class_section_id).await?;

        let mut count = 0;
        for slot in payload.slots {
            let start = NaiveTime::parse_from_str(&slot.start_time, "%H:%M:%S")
                .or_else(|_| NaiveTime::parse_from_str(&slot.start_time, "%H:%M"))
                .map_err(|e| AppError::BadRequest(format!("Invalid start_time: {e}")))?;
            let end = NaiveTime::parse_from_str(&slot.end_time, "%H:%M:%S")
                .or_else(|_| NaiveTime::parse_from_str(&slot.end_time, "%H:%M"))
                .map_err(|e| AppError::BadRequest(format!("Invalid end_time: {e}")))?;

            // 1. Clash Check: unless slot is marked is_shared, ensure teacher & room have no cross-section clashes
            if !slot.is_shared {
                let clash_res = Self::check_clashes(
                    pool,
                    school_id,
                    &CheckClashRequest {
                        day_of_week: slot.day_of_week,
                        period_number: slot.period_number,
                        teacher_staff_id: slot.teacher_staff_id,
                        room_id: slot.room_id,
                        exclude_slot_id: slot.id,
                        class_section_id: Some(payload.class_section_id),
                    },
                )
                .await?;

                if clash_res.has_clash {
                    return Err(AppError::BadRequest(format!(
                        "Schedule conflict: {}",
                        clash_res.clashes[0].message
                    )));
                }
            }

            // 2. If slot.id is provided, update existing slot directly
            if let Some(slot_id) = slot.id {
                let exists = TimetableRepository::slot_exists(&mut tx, school_id, slot_id).await?;
                if exists {
                    TimetableRepository::update_slot(
                        &mut tx,
                        school_id,
                        slot_id,
                        slot.day_of_week,
                        slot.period_number,
                        start,
                        end,
                        slot.subject_id,
                        slot.teacher_staff_id,
                        &slot.co_teacher_staff_ids,
                        slot.is_shared,
                        slot.room_id,
                        slot.elective_group_id,
                        slot.is_elective_slot,
                    )
                    .await?;

                    count += 1;
                    continue;
                }
            }

            // 3. Otherwise upsert on (class_section_id, day_of_week, period_number, subject_id)
            TimetableRepository::insert_slot_on_conflict(
                &mut tx,
                school_id,
                academic_year_id,
                payload.class_section_id,
                slot.day_of_week,
                slot.period_number,
                start,
                end,
                slot.subject_id,
                slot.teacher_staff_id,
                &slot.co_teacher_staff_ids,
                slot.is_shared,
                slot.room_id,
                slot.elective_group_id,
                slot.is_elective_slot,
            )
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
        let rows_affected = TimetableRepository::delete_slot(pool, school_id, slot_id).await?;
        if rows_affected == 0 {
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
        let existing_teacher_rows = TimetableRepository::get_existing_teacher_slots_for_generator(
            pool,
            school_id,
            req.class_section_id,
        )
        .await?;

        let mut existing_teachers: HashSet<(Uuid, u32, u32)> = HashSet::new();
        let mut teacher_day_workload: HashMap<(Uuid, u32), u32> = HashMap::new();
        for (tid, day, period) in existing_teacher_rows {
            existing_teachers.insert((tid, day as u32, period as u32));
            *teacher_day_workload.entry((tid, day as u32)).or_insert(0) += 1;
        }

        let existing_room_rows = TimetableRepository::get_existing_room_slots_for_generator(
            pool,
            school_id,
            req.class_section_id,
        )
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
        TimetableRepository::list_substitutions(pool, school_id, date).await
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
        let slot = TimetableRepository::get_slot_for_substitution(
            pool,
            school_id,
            input.timetable_slot_id,
        )
        .await?
        .ok_or_else(|| AppError::NotFound("Timetable slot not found".into()))?;

        let (slot_id, day_of_week, period_number, orig_teacher_id) = slot;

        // 2. Validate substitute teacher: make sure they do not have a teaching slot or active substitution at this period
        let has_regular_clash = TimetableRepository::check_regular_clash_for_sub(
            pool,
            school_id,
            input.substitute_teacher_id,
            day_of_week,
            period_number,
        )
        .await?;

        if has_regular_clash {
            return Err(AppError::BadRequest(
                "Substitute teacher already has a scheduled regular class in this period.".into(),
            ));
        }

        let has_sub_clash = TimetableRepository::check_sub_clash_for_sub(
            pool,
            school_id,
            input.substitute_teacher_id,
            date,
            period_number,
        )
        .await?;

        if has_sub_clash {
            return Err(AppError::BadRequest(
                "Substitute teacher is already assigned to another substitution in this period."
                    .into(),
            ));
        }

        // 3. Insert or update substitution
        let sub_id = TimetableRepository::insert_or_update_substitution(
            pool,
            school_id,
            date,
            slot_id,
            orig_teacher_id,
            input.substitute_teacher_id,
            input.reason.clone(),
            user_id,
        )
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
        let rows_affected =
            TimetableRepository::delete_substitution(pool, school_id, sub_id).await?;
        if rows_affected == 0 {
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
        TimetableRepository::get_available_substitute_teachers(
            pool,
            school_id,
            date,
            day_of_week,
            period_number,
            exclude_staff_id,
        )
        .await
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

        TimetableRepository::get_today_schedule(
            pool,
            school_id,
            date,
            class_section_id,
            teacher_staff_id,
        )
        .await
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
        let clashes = TimetableRepository::get_substitution_deadlock_clashes(pool, school_id, date)
            .await
            .unwrap_or_default();

        for (sub_staff_id, period, clash_count) in clashes {
            tracing::warn!(
                "Substitution deadlock clash: Substitute staff {} is assigned to {} classes in period {} on {}",
                sub_staff_id, clash_count, period, date
            );

            let field_identifier = format!("substitution_deadlock_{}_{}", date, period);
            let ticket_exists =
                TimetableRepository::deadlock_ticket_exists(pool, school_id, &field_identifier)
                    .await
                    .unwrap_or(false);

            if !ticket_exists {
                let user_id_opt = TimetableRepository::get_staff_user_id(pool, sub_staff_id)
                    .await
                    .unwrap_or(None);

                if let Some(user_id) = user_id_opt {
                    let ticket_number = format!(
                        "TKT-DEADLOCK-{}-{}",
                        date.format("%Y%m%d"),
                        Uuid::new_v4().to_string()[..6].to_uppercase()
                    );
                    let _ = TimetableRepository::insert_deadlock_ticket(
                        pool,
                        school_id,
                        user_id,
                        &ticket_number,
                        &field_identifier,
                        clash_count,
                        &format!(
                            "Substitution deadlock detected: Substitute teacher is double-booked across {} classes in Period {} on {}. Immediate re-assignment required.",
                            clash_count, period, date
                        ),
                    )
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
        TimetableRepository::sync_daily_timetable_for_date(pool, date).await
    }

    /// Retrieve school timetable configuration
    pub async fn get_timetable_config(
        pool: &PgPool,
        school_id: Uuid,
    ) -> Result<serde_json::Value, AppError> {
        let row = TimetableRepository::get_timetable_config(pool, school_id).await?;

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
        TimetableRepository::save_timetable_config(pool, school_id, config).await
    }
}
