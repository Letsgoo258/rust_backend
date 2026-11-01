use chrono::NaiveTime;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectAllocationInput {
    pub subject_id: Uuid,
    pub subject_name: Option<String>,
    pub teacher_staff_id: Uuid,
    pub teacher_name: Option<String>,
    pub weekly_periods: u32,
    pub room_id: Option<Uuid>,
    pub is_continuous: Option<bool>, // for lab / practical continuous double periods
    pub continuous_span: Option<u32>, // default: 2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodTimingInput {
    pub period_number: u32,
    pub start_time: String, // HH:MM:SS
    pub end_time: String,   // HH:MM:SS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoGenerateRequest {
    pub class_section_id: Uuid,
    pub days: Option<Vec<u32>>, // default: [1, 2, 3, 4, 5, 6] (Mon-Sat)
    pub periods_per_day: Option<u32>, // default: 8
    pub period_timings: Option<Vec<PeriodTimingInput>>,
    pub lunch_after_period: Option<u32>, // default: 4 (no continuous lab across lunch)
    pub max_teacher_daily_workload: Option<u32>, // default: 5 periods/day
    pub allocations: Vec<SubjectAllocationInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedSlot {
    pub day_of_week: u32,
    pub period_number: u32,
    pub start_time: String,
    pub end_time: String,
    pub subject_id: Uuid,
    pub subject_name: Option<String>,
    pub teacher_staff_id: Uuid,
    pub teacher_name: Option<String>,
    pub room_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoGenerateResponse {
    pub success: bool,
    pub total_slots: usize,
    pub message: String,
    pub slots: Vec<GeneratedSlot>,
}

#[derive(Debug, Clone)]
struct SchedulableUnit {
    subject_id: Uuid,
    subject_name: Option<String>,
    teacher_staff_id: Uuid,
    teacher_name: Option<String>,
    room_id: Option<Uuid>,
    span: u32,
}

pub struct TimetableGenerator;

impl TimetableGenerator {
    /// Default timings for standard 8 periods (08:30 AM to 03:30 PM)
    pub fn default_period_timings() -> Vec<(NaiveTime, NaiveTime)> {
        vec![
            (
                NaiveTime::from_hms_opt(8, 30, 0).unwrap(),
                NaiveTime::from_hms_opt(9, 15, 0).unwrap(),
            ),
            (
                NaiveTime::from_hms_opt(9, 15, 0).unwrap(),
                NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
            ),
            (
                NaiveTime::from_hms_opt(10, 15, 0).unwrap(),
                NaiveTime::from_hms_opt(11, 0, 0).unwrap(),
            ),
            (
                NaiveTime::from_hms_opt(11, 0, 0).unwrap(),
                NaiveTime::from_hms_opt(11, 45, 0).unwrap(),
            ),
            (
                NaiveTime::from_hms_opt(12, 15, 0).unwrap(),
                NaiveTime::from_hms_opt(13, 0, 0).unwrap(),
            ),
            (
                NaiveTime::from_hms_opt(13, 0, 0).unwrap(),
                NaiveTime::from_hms_opt(13, 45, 0).unwrap(),
            ),
            (
                NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
                NaiveTime::from_hms_opt(14, 45, 0).unwrap(),
            ),
            (
                NaiveTime::from_hms_opt(14, 45, 0).unwrap(),
                NaiveTime::from_hms_opt(15, 30, 0).unwrap(),
            ),
        ]
    }

    /// Solves timetable assignment using constraint backtracking:
    /// - Guarantees 0 teacher clashes with existing commitments in other classes
    /// - Guarantees 0 section clashes
    /// - Enforces continuous consecutive periods for lab/practical subjects
    /// - Enforces maximum teacher workload per day
    /// - Enforces even daily distribution of core subjects (max 2 per day)
    pub fn solve(
        req: &AutoGenerateRequest,
        existing_teacher_commitments: &HashSet<(Uuid, u32, u32)>, // (teacher_staff_id, day, period)
        existing_room_commitments: &HashSet<(Uuid, u32, u32)>,    // (room_id, day, period)
        initial_teacher_day_workload: &HashMap<(Uuid, u32), u32>, // (teacher_staff_id, day) -> count
    ) -> Result<Vec<GeneratedSlot>, String> {
        let days = req.days.clone().unwrap_or_else(|| vec![1, 2, 3, 4, 5, 6]);
        let periods_per_day = req.periods_per_day.unwrap_or(8);
        let lunch_after = req.lunch_after_period.unwrap_or(4);
        let max_workload = req.max_teacher_daily_workload.unwrap_or(5);

        let timings: Vec<(NaiveTime, NaiveTime)> = if let Some(custom) = &req.period_timings {
            custom
                .iter()
                .filter_map(|t| {
                    let s = NaiveTime::parse_from_str(&t.start_time, "%H:%M:%S")
                        .or_else(|_| NaiveTime::parse_from_str(&t.start_time, "%H:%M"))
                        .ok()?;
                    let e = NaiveTime::parse_from_str(&t.end_time, "%H:%M:%S")
                        .or_else(|_| NaiveTime::parse_from_str(&t.end_time, "%H:%M"))
                        .ok()?;
                    Some((s, e))
                })
                .collect()
        } else {
            Self::default_period_timings()
        };

        let total_available_slots = days.len() * periods_per_day as usize;
        let total_requested_periods: u32 = req.allocations.iter().map(|a| a.weekly_periods).sum();

        if total_requested_periods as usize > total_available_slots {
            return Err(format!(
                "Requested periods ({total_requested_periods}) exceed total available periods ({total_available_slots} slots across {} days).",
                days.len()
            ));
        }

        // Decompose allocations into SchedulableUnits
        let mut units: Vec<SchedulableUnit> = Vec::new();
        for alloc in &req.allocations {
            let is_cont = alloc.is_continuous.unwrap_or(false);
            let span = if is_cont {
                alloc
                    .continuous_span
                    .unwrap_or(2)
                    .max(2)
                    .min(periods_per_day)
            } else {
                1
            };

            let mut remaining = alloc.weekly_periods;
            while remaining >= span && is_cont {
                units.push(SchedulableUnit {
                    subject_id: alloc.subject_id,
                    subject_name: alloc.subject_name.clone(),
                    teacher_staff_id: alloc.teacher_staff_id,
                    teacher_name: alloc.teacher_name.clone(),
                    room_id: alloc.room_id,
                    span,
                });
                remaining -= span;
            }

            // Remainder as single periods
            for _ in 0..remaining {
                units.push(SchedulableUnit {
                    subject_id: alloc.subject_id,
                    subject_name: alloc.subject_name.clone(),
                    teacher_staff_id: alloc.teacher_staff_id,
                    teacher_name: alloc.teacher_name.clone(),
                    room_id: alloc.room_id,
                    span: 1,
                });
            }
        }

        // Most Constrained Variable First heuristic:
        // Prioritize continuous blocks first, then higher frequency subjects
        let mut count_map: HashMap<Uuid, u32> = HashMap::new();
        for u in &units {
            *count_map.entry(u.subject_id).or_insert(0) += u.span;
        }

        units.sort_by(|a, b| {
            b.span.cmp(&a.span).then_with(|| {
                let ca = count_map.get(&a.subject_id).unwrap_or(&0);
                let cb = count_map.get(&b.subject_id).unwrap_or(&0);
                cb.cmp(ca)
            })
        });

        let mut grid: HashMap<(u32, u32), SchedulableUnit> = HashMap::new();
        let mut day_subject_count: HashMap<(u32, Uuid), u32> = HashMap::new();
        let mut teacher_day_workload = initial_teacher_day_workload.clone();

        // Slot coordinates prioritized round-robin across days
        let mut slots: Vec<(u32, u32)> = Vec::new();
        for p in 1..=periods_per_day {
            for d in &days {
                slots.push((*d, p));
            }
        }

        if !Self::backtrack(
            0,
            &units,
            &slots,
            periods_per_day,
            lunch_after,
            max_workload,
            &mut grid,
            &mut day_subject_count,
            &mut teacher_day_workload,
            existing_teacher_commitments,
            existing_room_commitments,
        ) {
            return Err(
                "Could not find a conflict-free timetable satisfying all teacher workload and continuous lab constraints. Please adjust teacher allocations or maximum workload settings.".to_string()
            );
        }

        // Convert grid to sorted GeneratedSlot list
        let mut result = Vec::new();
        for ((day, period), alloc) in grid {
            let (start_t, end_t) =
                timings
                    .get((period - 1) as usize)
                    .copied()
                    .unwrap_or_else(|| {
                        (
                            NaiveTime::from_hms_opt(8, 30, 0).unwrap(),
                            NaiveTime::from_hms_opt(9, 15, 0).unwrap(),
                        )
                    });

            result.push(GeneratedSlot {
                day_of_week: day,
                period_number: period,
                start_time: start_t.format("%H:%M:%S").to_string(),
                end_time: end_t.format("%H:%M:%S").to_string(),
                subject_id: alloc.subject_id,
                subject_name: alloc.subject_name,
                teacher_staff_id: alloc.teacher_staff_id,
                teacher_name: alloc.teacher_name,
                room_id: alloc.room_id,
            });
        }

        result.sort_by(|a, b| {
            a.day_of_week
                .cmp(&b.day_of_week)
                .then(a.period_number.cmp(&b.period_number))
        });

        Ok(result)
    }

    fn backtrack(
        unit_idx: usize,
        units: &[SchedulableUnit],
        slots: &[(u32, u32)],
        periods_per_day: u32,
        lunch_after: u32,
        max_workload: u32,
        grid: &mut HashMap<(u32, u32), SchedulableUnit>,
        day_subject_count: &mut HashMap<(u32, Uuid), u32>,
        teacher_day_workload: &mut HashMap<(Uuid, u32), u32>,
        existing_teacher_commitments: &HashSet<(Uuid, u32, u32)>,
        existing_room_commitments: &HashSet<(Uuid, u32, u32)>,
    ) -> bool {
        if unit_idx >= units.len() {
            return true;
        }

        let unit = &units[unit_idx];
        let span = unit.span;

        for &(day, period) in slots {
            // Check boundary
            if period + span - 1 > periods_per_day {
                continue;
            }

            // Do not cross lunch break
            if span > 1 && lunch_after > 0 {
                let ends_at = period + span - 1;
                if period <= lunch_after && ends_at > lunch_after {
                    continue;
                }
            }

            // Check maximum periods of same subject on this day (max 2)
            let curr_day_subject_count =
                *day_subject_count.get(&(day, unit.subject_id)).unwrap_or(&0);
            if curr_day_subject_count + span > 2 {
                continue;
            }

            // Check teacher maximum daily workload
            let curr_teacher_workload = *teacher_day_workload
                .get(&(unit.teacher_staff_id, day))
                .unwrap_or(&0);
            if curr_teacher_workload + span > max_workload {
                continue;
            }

            // Check all slots in span for clashes
            let mut valid = true;
            for offset in 0..span {
                let p = period + offset;
                if grid.contains_key(&(day, p)) {
                    valid = false;
                    break;
                }
                if existing_teacher_commitments.contains(&(unit.teacher_staff_id, day, p)) {
                    valid = false;
                    break;
                }
                if let Some(r_id) = unit.room_id {
                    if existing_room_commitments.contains(&(r_id, day, p)) {
                        valid = false;
                        break;
                    }
                }
            }

            if !valid {
                continue;
            }

            // Assign all span slots
            for offset in 0..span {
                grid.insert((day, period + offset), unit.clone());
            }
            *day_subject_count.entry((day, unit.subject_id)).or_insert(0) += span;
            *teacher_day_workload
                .entry((unit.teacher_staff_id, day))
                .or_insert(0) += span;

            if Self::backtrack(
                unit_idx + 1,
                units,
                slots,
                periods_per_day,
                lunch_after,
                max_workload,
                grid,
                day_subject_count,
                teacher_day_workload,
                existing_teacher_commitments,
                existing_room_commitments,
            ) {
                return true;
            }

            // Backtrack
            for offset in 0..span {
                grid.remove(&(day, period + offset));
            }
            if let Some(c) = day_subject_count.get_mut(&(day, unit.subject_id)) {
                if *c >= span {
                    *c -= span;
                }
            }
            if let Some(w) = teacher_day_workload.get_mut(&(unit.teacher_staff_id, day)) {
                if *w >= span {
                    *w -= span;
                }
            }
        }

        false
    }
}
