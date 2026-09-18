use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct StudentDto {
    pub id: Uuid,
    pub school_id: Uuid,
    pub user_id: Option<Uuid>,
    pub admission_number: String,
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: time::Date,
    pub gender: Option<String>,
    pub blood_group: Option<String>,
    pub status: String,
}

#[derive(Deserialize, Debug)]
pub struct CreateStudentRequest {
    pub admission_number: String,
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: time::Date,
    pub gender: Option<String>,
    pub blood_group: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct GuardianDto {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct CreateGuardianRequest {
    pub first_name: String,
    pub last_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub relationship: String,
    pub is_primary: bool,
    pub is_emergency_contact: bool,
}

#[derive(Deserialize, Debug)]
pub struct EnrollStudentRequest {
    pub academic_year_id: Uuid,
    pub class_section_id: Uuid,
    pub roll_number: Option<String>,
}
