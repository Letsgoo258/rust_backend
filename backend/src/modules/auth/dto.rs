#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub success: bool,
    pub data: UserDto,
}

#[derive(Serialize, Clone)]
pub struct UserDto {
    pub id: uuid::Uuid,
    pub school_id: uuid::Uuid,
    pub username: String,
    pub display_name: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct SetupRequest {
    pub school_name: String,
    pub school_code: String,
    pub admin_username: String,
    pub admin_email: String,
    pub admin_password: String,
    pub first_name: String,
    pub last_name: String,
}

#[derive(serde::Serialize)]
pub struct SetupResponse {
    pub message: String,
    pub school_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
}

#[derive(serde::Deserialize)]
pub struct CreateSchoolRequest {
    pub name: String,
    pub code: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub admin_username: String,
    pub admin_email: String,
    pub admin_password: String,
    pub admin_first_name: String,
    pub admin_last_name: String,
}

#[derive(serde::Serialize)]
pub struct CreateSchoolResponse {
    pub message: String,
    pub school_id: uuid::Uuid,
}

#[derive(serde::Serialize)]
pub struct SchoolDto {
    pub id: uuid::Uuid,
    pub name: String,
    pub code: String,
    pub status: String,
    pub created_at: String,
}

#[derive(serde::Serialize)]
pub struct SuperAdminStatsResponse {
    pub total_users: i64,
    pub active_tenants: i64,
    pub schools: Vec<SchoolDto>,
}
