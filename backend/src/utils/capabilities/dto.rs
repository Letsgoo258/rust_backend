use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CapabilityItem {
    pub key: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleCapabilitiesDto {
    pub role_code: String,
    pub role_name: String,
    pub is_system_role: bool,
    pub capabilities: Vec<CapabilityItem>,
}

#[derive(Debug, Clone)]
pub struct RoleCapabilityOverride {
    pub role_code: String,
    pub capability_key: String,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSearchItem {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCapabilityGrantDto {
    pub id: Uuid,
    pub school_id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub user_username: String,
    pub user_roles: Vec<String>,
    pub capability_key: String,
    pub capability_name: String,
    pub capability_category: String,
    pub granted_by_name: Option<String>,
    pub reason: Option<String>,
    pub expires_at: Option<String>,
    pub is_active: bool,
    pub duration_label: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RequestGrantOtpPayload {
    pub user_id: Uuid,
    pub capability_key: String,
    pub duration: String,
    pub custom_expires_at: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmGrantPayload {
    pub user_id: Uuid,
    pub capability_key: String,
    pub duration: String,
    pub custom_expires_at: Option<String>,
    pub reason: Option<String>,
    pub otp: String,
}
