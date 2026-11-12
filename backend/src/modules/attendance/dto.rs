use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentAttendanceInput {
    pub student_id: Uuid,
    pub status: String, // PRESENT, ABSENT, LATE, EXCUSED, HALF_DAY
    pub remarks: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordAttendanceRequest {
    pub class_section_id: Uuid,
    pub date: String, // YYYY-MM-DD
    pub period_number: Option<i32>,
    pub attendance_type: Option<String>, // 'DAILY' or 'PERIOD_WISE'
    pub subject_id: Option<Uuid>,
    pub records: Vec<StudentAttendanceInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentAttendanceUpdateInput {
    pub student_id: Uuid,
    pub status: String,
    pub remarks: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkEditAttendanceRequest {
    pub session_id: Uuid,
    pub reason: String, // MANDATORY justification
    pub updates: Vec<StudentAttendanceUpdateInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSingleRecordRequest {
    pub status: String,
    pub remarks: Option<String>,
    pub reason: Option<String>, // Mandatory if admin or after edit window
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttendanceConfigDto {
    pub daily_cutoff_time: String,              // e.g. "10:30"
    pub freeze_period_minutes: i32,             // e.g. 120
    pub allow_teacher_edit_window_minutes: i32, // e.g. 60
    pub enable_collision_tickets: bool,
    pub enforce_timetable_check: bool,
}

impl Default for AttendanceConfigDto {
    fn default() -> Self {
        Self {
            daily_cutoff_time: "10:30".into(),
            freeze_period_minutes: 120,
            allow_teacher_edit_window_minutes: 60,
            enable_collision_tickets: true,
            enforce_timetable_check: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttendanceSessionSummary {
    pub id: Uuid,
    pub school_id: Uuid,
    pub academic_year_id: Uuid,
    pub class_section_id: Uuid,
    pub class_name: String,
    pub section_name: String,
    pub date: String,
    pub period_number: Option<i32>,
    pub attendance_type: String,
    pub subject_id: Option<Uuid>,
    pub subject_name: Option<String>,
    pub marked_by: Uuid,
    pub marked_by_name: String,
    pub marked_by_role: String,
    pub is_shared_session: bool,
    pub co_marked_count: i32,
    pub status: String,
    pub is_locked: bool,
    pub total_students: i64,
    pub present_count: i64,
    pub absent_count: i64,
    pub late_count: i64,
    pub excused_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentAttendanceDetail {
    pub record_id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub admission_number: Option<String>,
    pub roll_number: Option<String>,
    pub status: String,
    pub remarks: Option<String>,
    pub marked_at: DateTime<Utc>,
    pub corrections_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttendanceCorrectionLog {
    pub id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub old_status: String,
    pub new_status: String,
    pub reason: String,
    pub changed_by: Uuid,
    pub changed_by_name: String,
    pub changed_by_role: String,
    pub is_critical: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttendanceSessionDetailResponse {
    pub session: AttendanceSessionSummary,
    pub records: Vec<StudentAttendanceDetail>,
    pub corrections: Vec<AttendanceCorrectionLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodayAttendanceStats {
    pub date: String,
    pub total_enrolled: i64,
    pub marked_students: i64,
    pub present_students: i64,
    pub absent_students: i64,
    pub late_students: i64,
    pub attendance_percentage: f64,
    pub total_sections: i64,
    pub completed_sections: i64,
    pub pending_sections: i64,
    pub collision_alerts_count: i64,
    pub substitution_deadlocks_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollisionItem {
    pub id: Uuid,
    pub class_section_id: Uuid,
    pub class_name: String,
    pub section_name: String,
    pub date: String,
    pub period_number: Option<i32>,
    pub attempted_by_user_id: Uuid,
    pub attempted_by_name: String,
    pub attempted_by_role: String,
    pub existing_session_id: Option<Uuid>,
    pub existing_marked_by_name: Option<String>,
    pub collision_type: String,
    pub ticket_id: Option<Uuid>,
    pub ticket_number: Option<String>,
    pub status: String,
    pub resolution_notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveCollisionRequest {
    pub action: String, // 'OVERWRITE_EXISTING', 'KEEP_EXISTING', 'DISMISS'
    pub resolution_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentRosterItem {
    pub student_id: Uuid,
    pub full_name: String,
    pub admission_number: Option<String>,
    pub roll_number: Option<String>,
    pub gender: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRow {
    pub record_id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub roll_number: Option<String>,
    pub admission_number: Option<String>,
    pub class_name: String,
    pub section_name: String,
    pub subject_name: Option<String>,
    pub session_type: String,
    pub status: String,
    pub remarks: Option<String>,
    pub marked_by_name: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterQuery {
    pub date: Option<String>,
    pub class_section_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RecordWithSessionInfo {
    pub record_id: Uuid,
    pub session_id: Uuid,
    pub student_id: Uuid,
    pub old_status: String,
    pub marked_by: Uuid,
    pub is_locked: bool,
    pub created_at: DateTime<Utc>,
    pub class_section_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaiseAttendanceTicketRequest {
    pub class_section_id: Uuid,
    pub date: String,
    pub period_number: Option<i32>,
    pub reason: String,
    pub collision_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WeeklyHeatmapQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapDayHeader {
    pub date: String,
    pub day_name: String,
    pub formatted: String,
    pub is_today: bool,
    pub is_future: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassDayAttendanceSlot {
    pub date: String,
    pub day_name: String,
    pub rate: Option<i32>,
    pub status: String, // "RECORDED", "PENDING", "NOT_RECORDED", "FUTURE"
    pub present_count: i64,
    pub absent_count: i64,
    pub late_count: i64,
    pub total_records: i64,
    pub session_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassHeatmapRow {
    pub class_section_id: Uuid,
    pub class_name: String,
    pub section_name: String,
    pub class_code: Option<String>,
    pub section_code: Option<String>,
    pub class_teacher_name: Option<String>,
    pub enrolled_count: i64,
    pub days: Vec<ClassDayAttendanceSlot>,
    pub weekly_avg_rate: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyHeatmapSummary {
    pub overall_attendance_rate: Option<i32>,
    pub total_recorded_sessions: i64,
    pub best_class_section: Option<String>,
    pub lowest_class_section: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyHeatmapResponse {
    pub start_date: String,
    pub end_date: String,
    pub day_headers: Vec<HeatmapDayHeader>,
    pub rows: Vec<ClassHeatmapRow>,
    pub summary: WeeklyHeatmapSummary,
}
