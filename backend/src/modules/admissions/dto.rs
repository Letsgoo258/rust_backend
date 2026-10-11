use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateAdmissionRequest {
    pub applicant_first_name: String,
    pub applicant_middle_name: Option<String>,
    pub applicant_last_name: String,
    pub date_of_birth: String, // YYYY-MM-DD
    pub applicant_gender: Option<String>,
    pub applicant_phone: Option<String>,
    pub applicant_email: Option<String>,

    pub guardian_name: String,
    pub guardian_relationship: String,
    pub guardian_phone: String,
    pub guardian_email: Option<String>,
    pub guardian_occupation: Option<String>,

    #[serde(default)]
    pub custom_fields: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdmissionApplicantDto {
    pub first_name: String,
    pub middle_name: Option<String>,
    pub last_name: String,
    pub date_of_birth: String,
    pub gender: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdmissionGuardianDto {
    pub name: String,
    pub relationship: String,
    pub phone: String,
    pub email: Option<String>,
    pub occupation: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdmissionItemDto {
    pub id: Uuid,
    pub school_id: Uuid,
    pub application_number: Option<String>,
    pub status: String,
    pub submitted_at: Option<String>,
    pub applicant: AdmissionApplicantDto,
    pub guardian: AdmissionGuardianDto,
    pub custom_fields: HashMap<String, String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAdmissionStatusRequest {
    pub status: String,
    pub reason: Option<String>,
}
