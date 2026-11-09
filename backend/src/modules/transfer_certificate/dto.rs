use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TemplateQuery {
    pub template_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SaveTemplatePayload {
    pub template_type: Option<String>,
    pub name: Option<String>,
    pub config: Value,
    pub is_published: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VerifyOtpPayload {
    pub otp_code: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RollbackPayload {
    pub version_id: Uuid,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UpdateSequencePayload {
    pub display_name: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub current_number: Option<i32>,
    pub padding_length: Option<i32>,
    pub format_pattern: Option<String>,
    pub academic_year: Option<String>,
    pub is_locked: Option<bool>,
    pub reset_annually: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IssueReportPayload {
    pub report_type: String,
    pub student_ids: Vec<Uuid>,
    pub academic_year: Option<String>,
    pub remarks: Option<String>,
    pub custom_variables: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ListIssuedReportsQuery {
    pub report_type: Option<String>,
    pub student_id: Option<Uuid>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CancelIssuedReportPayload {
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IssueTransferCertificatesRequest {
    pub user_ids: Vec<Uuid>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IssueTransferCertificatesResponse {
    pub success: bool,
    pub issued_count: usize,
    pub message: String,
}
