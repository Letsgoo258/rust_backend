use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct LoginRequest {
    pub school_code: String,
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
