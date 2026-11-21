use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskType {
    Homework,
    Assignment,
    ClassroomTask,
    Other,
}

impl TaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::Homework => "HOMEWORK",
            TaskType::Assignment => "ASSIGNMENT",
            TaskType::ClassroomTask => "CLASSROOM_TASK",
            TaskType::Other => "OTHER",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_uppercase().as_str() {
            "HOMEWORK" => TaskType::Homework,
            "ASSIGNMENT" => TaskType::Assignment,
            "CLASSROOM_TASK" | "CLASSWORK" | "CLASSROOM" => TaskType::ClassroomTask,
            _ => TaskType::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskStatus {
    Draft,
    Published,
    Closed,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Draft => "DRAFT",
            TaskStatus::Published => "PUBLISHED",
            TaskStatus::Closed => "CLOSED",
            TaskStatus::Cancelled => "CANCELLED",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_uppercase().as_str() {
            "PUBLISHED" => TaskStatus::Published,
            "CLOSED" => TaskStatus::Closed,
            "CANCELLED" => TaskStatus::Cancelled,
            _ => TaskStatus::Draft,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttachmentType {
    TemporaryUpload,
    GoogleDrive,
}

impl AttachmentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AttachmentType::TemporaryUpload => "TEMPORARY_UPLOAD",
            AttachmentType::GoogleDrive => "GOOGLE_DRIVE",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_uppercase().as_str() {
            "GOOGLE_DRIVE" | "GOOGLEDRIVE" | "DRIVE" => AttachmentType::GoogleDrive,
            _ => AttachmentType::TemporaryUpload,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LmsTask {
    pub id: Uuid,
    pub school_id: Uuid,
    pub academic_year_id: Uuid,
    pub title: String,
    pub description: String,
    pub special_message: Option<String>,
    pub task_type: String,
    pub class_section_id: Uuid,
    pub subject_id: Option<Uuid>,
    pub teaching_group_id: Option<Uuid>,
    pub attendance_session_id: Option<Uuid>,
    pub created_by: Uuid,
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
}
