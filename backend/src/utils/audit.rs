use sqlx::PgPool;
use uuid::Uuid;
use std::fs::OpenOptions;
use std::io::Write;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AuditRecord {
    pub school_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub details: Option<serde_json::Value>,
    pub ip_address: Option<String>,
}

pub async fn log_action(
    _pool: &PgPool, // Keeping pool for signature compatibility, though unused now
    school_id: Option<Uuid>,
    user_id: Option<Uuid>,
    action: &str,
    resource_type: Option<&str>,
    resource_id: Option<Uuid>,
    details: Option<serde_json::Value>,
    ip_address: Option<String>,
) -> Result<(), String> {
    let record = AuditRecord {
        school_id,
        user_id,
        action: action.to_string(),
        resource_type: resource_type.map(|s| s.to_string()),
        resource_id,
        details,
        ip_address,
    };
    
    if let Ok(json) = serde_json::to_string(&record) {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("audit_buffer.jsonl") {
            let _ = writeln!(file, "{}", json);
        }
    }
    
    Ok(())
}
