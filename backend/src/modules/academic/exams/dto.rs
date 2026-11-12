use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionConfigItem {
    pub id: String,
    pub label: String,
    pub max_marks: f64,
}

#[derive(Debug, Deserialize)]
pub struct CreateExamEventPayload {
    pub academic_year_id: Option<Uuid>,
    pub name: String,
    pub exam_type: Option<String>,
    pub affects_overall: Option<bool>,
    #[serde(default)]
    pub class_section_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExamEventListItem {
    pub id: Uuid,
    pub school_id: Uuid,
    pub academic_year_id: Uuid,
    pub name: String,
    pub exam_type: String,
    pub affects_overall: bool,
    pub status: String,
    pub classes_count: i64,
    pub earliest_date: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExamEventAssignedClassItem {
    pub class_section_id: Uuid,
    pub class_name: String,
    pub section_name: String,
    pub scheduled_papers_count: i64,
    pub total_subjects_count: i64,
    pub earliest_date: Option<String>,
    pub latest_date: Option<String>,
    pub whatsapp_published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExamEventResponse {
    pub id: Uuid,
    pub school_id: Uuid,
    pub academic_year_id: Uuid,
    pub name: String,
    pub exam_type: String,
    pub affects_overall: bool,
    pub status: String,
    pub assigned_classes: Vec<ExamEventAssignedClassItem>,
    pub schedules: Vec<ExamResponse>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionCoreSubjectItem {
    pub subject_id: Uuid,
    pub subject_name: String,
    pub subject_code: String,
    pub system_subject_type: String,
    pub subject_type: Option<String>,
    pub department: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ClassTimetableSubjectSlot {
    pub subject_id: Uuid,
    pub is_enabled: bool,
    pub exam_date: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub max_marks: Option<f64>,
    pub pass_marks: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct SaveClassExamTimetablePayload {
    pub slots: Vec<ClassTimetableSubjectSlot>,
    #[serde(default)]
    pub apply_to_all_sections: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AddClassExamSchedulePayload {
    pub class_section_id: Uuid,
    #[serde(default)]
    pub subject_id: Option<Uuid>,
    #[serde(default)]
    pub subject_ids: Vec<Uuid>,
    pub exam_date: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub max_marks: Option<f64>,
    pub pass_marks: Option<f64>,
    pub question_config: Option<Vec<QuestionConfigItem>>,
    pub notify_whatsapp: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateExamPayload {
    pub exam_event_id: Option<Uuid>,
    pub academic_year_id: Option<Uuid>,
    pub term_id: Option<Uuid>,
    pub class_section_id: Uuid,
    pub subject_id: Uuid,
    pub name: String,
    pub exam_type: Option<String>,
    pub affects_overall: Option<bool>,
    pub exam_date: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub max_marks: Option<f64>,
    pub pass_marks: Option<f64>,
    pub question_config: Option<Vec<QuestionConfigItem>>,
    pub notify_whatsapp: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExamResponse {
    pub id: Uuid,
    pub exam_event_id: Option<Uuid>,
    pub school_id: Uuid,
    pub academic_year_id: Uuid,
    pub term_id: Option<Uuid>,
    pub class_section_id: Uuid,
    pub subject_id: Uuid,
    pub class_name: Option<String>,
    pub section_name: Option<String>,
    pub subject_name: Option<String>,
    pub subject_code: Option<String>,
    pub name: String,
    pub exam_type: String,
    pub affects_overall: bool,
    pub exam_date: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub max_marks: f64,
    pub pass_marks: f64,
    pub question_config: serde_json::Value,
    pub status: String,
    pub whatsapp_notified: bool,
    pub created_by: Option<Uuid>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentMarksItem {
    pub student_id: Uuid,
    pub student_name: String,
    pub admission_number: Option<String>,
    pub roll_number: Option<String>,
    pub marks_obtained: Option<f64>,
    pub is_absent: bool,
    pub question_marks: serde_json::Value,
    pub remarks: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExamDetailResponse {
    pub exam: ExamResponse,
    pub students: Vec<StudentMarksItem>,
    pub can_edit: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateAssignmentPayload {
    pub academic_year_id: Option<Uuid>,
    pub class_section_id: Uuid,
    pub subject_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub due_date: String,
    pub max_marks: Option<f64>,
    pub affects_overall: Option<bool>,
    pub assignment_type: Option<String>,
    pub question_config: Option<Vec<QuestionConfigItem>>,
    pub notify_whatsapp: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentResponse {
    pub id: Uuid,
    pub school_id: Uuid,
    pub academic_year_id: Uuid,
    pub class_section_id: Uuid,
    pub subject_id: Uuid,
    pub teacher_user_id: Uuid,
    pub teacher_name: Option<String>,
    pub class_name: Option<String>,
    pub section_name: Option<String>,
    pub subject_name: Option<String>,
    pub subject_code: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub due_date: String,
    pub max_marks: f64,
    pub affects_overall: bool,
    pub assignment_type: String,
    pub question_config: serde_json::Value,
    pub whatsapp_notified: bool,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct AssignmentDetailResponse {
    pub assignment: AssignmentResponse,
    pub students: Vec<StudentMarksItem>,
    pub can_edit: bool,
}

#[derive(Debug, Deserialize)]
pub struct SaveStudentMarkEntry {
    pub student_id: Uuid,
    pub marks_obtained: Option<f64>,
    pub is_absent: Option<bool>,
    pub question_marks: Option<serde_json::Value>,
    pub remarks: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SaveMarksPayload {
    pub question_config: Option<Vec<QuestionConfigItem>>,
    pub marks: Vec<SaveStudentMarkEntry>,
    pub is_auto_save: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ExamFilterQuery {
    pub class_section_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
    pub academic_year_id: Option<Uuid>,
    pub date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AssignmentFilterQuery {
    pub class_section_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
    pub teacher_user_id: Option<Uuid>,
    pub academic_year_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct ClassScoreItemSummary {
    pub item_id: Uuid,
    pub item_type: String, // "EXAM" or "ASSIGNMENT"
    pub title: String,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub max_marks: f64,
    pub date: String,
    pub affects_overall: bool,
}

#[derive(Debug, Serialize)]
pub struct StudentItemScore {
    pub item_id: Uuid,
    pub marks_obtained: Option<f64>,
    pub is_absent: bool,
}

#[derive(Debug, Serialize)]
pub struct StudentScoresSummary {
    pub student_id: Uuid,
    pub student_name: String,
    pub admission_number: Option<String>,
    pub roll_number: Option<String>,
    pub scores: Vec<StudentItemScore>,
}

#[derive(Debug, Serialize)]
pub struct ClassSectionScoresResponse {
    pub class_section_id: Uuid,
    pub class_name: String,
    pub section_name: String,
    pub items: Vec<ClassScoreItemSummary>,
    pub students: Vec<StudentScoresSummary>,
}
