use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct CreateLmsTaskRequest {
    pub academic_year_id: Option<Uuid>,
    pub title: String,
    pub description: String,
    pub special_message: Option<String>,
    pub task_type: Option<String>,
    pub class_section_id: Uuid,
    pub subject_id: Option<Uuid>,
    pub teaching_group_id: Option<Uuid>,
    pub attendance_session_id: Option<Uuid>,
    pub assigned_at: Option<DateTime<Utc>>,
    pub deadline_at: Option<DateTime<Utc>>,
    pub add_to_calendar: Option<bool>,
    pub notify_whatsapp: Option<bool>,
    pub notify_email: Option<bool>,
    pub is_evaluation: Option<bool>,
    #[serde(alias = "evaluation")]
    pub evaluation_config: Option<CreateEvaluationConfig>,
    pub attachments: Option<Vec<CreateAttachmentInput>>,
    pub publish_now: Option<bool>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateLmsTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub special_message: Option<String>,
    pub task_type: Option<String>,
    pub class_section_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
    pub teaching_group_id: Option<Uuid>,
    pub attendance_session_id: Option<Uuid>,
    pub assigned_at: Option<DateTime<Utc>>,
    pub deadline_at: Option<DateTime<Utc>>,
    pub add_to_calendar: Option<bool>,
    pub notify_whatsapp: Option<bool>,
    pub notify_email: Option<bool>,
    pub is_evaluation: Option<bool>,
    pub evaluation_config: Option<CreateEvaluationConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateAttachmentInput {
    #[serde(alias = "storage_provider", default = "default_attachment_type")]
    pub attachment_type: String, // TEMPORARY_UPLOAD, IMAGEKIT, or GOOGLE_DRIVE
    #[serde(alias = "file_name")]
    pub original_filename: Option<String>,
    pub mime_type: Option<String>,
    #[serde(alias = "file_size_bytes")]
    pub file_size: Option<i64>,
    pub imagekit_file_id: Option<String>,
    pub file_url: String,
    pub google_drive_url: Option<String>,
}

fn default_attachment_type() -> String {
    "TEMPORARY_UPLOAD".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateEvaluationConfig {
    #[serde(alias = "title")]
    pub evaluation_title: Option<String>,
    #[serde(alias = "max_marks")]
    pub maximum_marks: f64,
    pub pass_marks: Option<f64>,
    pub evaluation_category: Option<String>,
    pub evaluation_date: Option<NaiveDate>,
    #[serde(alias = "result_inclusion")]
    pub include_in_results: Option<bool>,
    pub result_component_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveEvaluationMarksRequest {
    pub marks: Vec<StudentMarkEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StudentMarkEntry {
    pub student_id: Uuid,
    pub marks_obtained: Option<f64>,
    pub is_absent: Option<bool>,
    pub question_marks: Option<serde_json::Value>,
    pub remarks: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskFilterQuery {
    pub academic_year_id: Option<Uuid>,
    pub class_section_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
    pub task_type: Option<String>,
    pub status: Option<String>,
    pub is_evaluation: Option<bool>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LmsTaskResponse {
    pub id: Uuid,
    pub school_id: Uuid,
    pub academic_year_id: Uuid,
    pub title: String,
    pub description: String,
    pub special_message: Option<String>,
    pub task_type: String,
    pub class_section_id: Uuid,
    pub class_name: Option<String>,
    pub section_name: Option<String>,
    pub subject_id: Option<Uuid>,
    pub subject_name: Option<String>,
    pub teaching_group_id: Option<Uuid>,
    pub attendance_session_id: Option<Uuid>,
    pub attendance_session_info: Option<AttendanceSessionSnippet>,
    pub created_by: Uuid,
    pub creator_name: Option<String>,
    pub assigned_at: DateTime<Utc>,
    pub deadline_at: Option<DateTime<Utc>>,
    pub status: String,
    pub add_to_calendar: bool,
    pub calendar_event_id: Option<Uuid>,
    pub notify_whatsapp: bool,
    pub notify_email: bool,
    pub is_evaluation: bool,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub attachments: Vec<LmsAttachmentResponse>,
    pub evaluation: Option<LmsEvaluationResponse>,
    pub recipient_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttendanceSessionSnippet {
    pub id: Uuid,
    pub date: NaiveDate,
    pub period_number: Option<i32>,
    pub attendance_type: String,
    pub marked_by_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LmsAttachmentResponse {
    pub id: Uuid,
    pub attachment_type: String,
    pub original_filename: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub file_url: String,
    pub google_drive_url: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub deletion_status: String,
    pub uploaded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LmsEvaluationResponse {
    pub id: Uuid,
    pub task_id: Uuid,
    pub evaluation_title: String,
    pub maximum_marks: f64,
    pub pass_marks: f64,
    pub evaluation_category: Option<String>,
    pub evaluation_date: Option<NaiveDate>,
    pub include_in_results: bool,
    pub result_component_id: Option<Uuid>,
    pub status: String,
    pub total_students: i64,
    pub evaluated_students: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LmsEvaluationStudentMarksRow {
    pub student_id: Uuid,
    pub student_name: String,
    pub admission_number: Option<String>,
    pub roll_number: Option<String>,
    pub marks_obtained: Option<f64>,
    pub is_absent: bool,
    pub question_marks: serde_json::Value,
    pub remarks: Option<String>,
    pub entered_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LmsOverviewStats {
    pub total_tasks: i64,
    pub active_tasks: i64,
    pub upcoming_deadlines: i64,
    pub completed_tasks: i64,
    pub pending_evaluation: i64,
    pub recent_tasks: Vec<LmsTaskResponse>,
    pub class_activity: Vec<ClassActivitySummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClassActivitySummary {
    pub class_section_id: Uuid,
    pub class_name: String,
    pub section_name: String,
    pub task_count: i64,
    pub active_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyTeacherTaskReportItem {
    pub date: NaiveDate,
    pub teacher_id: Uuid,
    pub teacher_name: String,
    pub class_section_id: Uuid,
    pub class_name: String,
    pub section_name: String,
    pub subject_id: Option<Uuid>,
    pub subject_name: Option<String>,
    pub attendance_session_id: Option<Uuid>,
    pub period_number: Option<i32>,
    pub task_assigned: bool,
    pub task_id: Option<Uuid>,
    pub task_title: Option<String>,
    pub task_type: Option<String>,
    pub deadline_at: Option<DateTime<Utc>>,
    pub evaluation_required: bool,
    pub marks_entered: bool,
    pub marks_completed: bool,
}
