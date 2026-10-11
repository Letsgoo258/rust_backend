use sqlx::PgPool;
use std::fs::OpenOptions;
use std::io::Write;
use uuid::Uuid;

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

fn forward_to_critical_telemetry(record: &AuditRecord) {
    // 1. Emit OpenTelemetry/Tracing critical level log with structured attributes
    tracing::error!(
        target: "eravaya::security_telemetry",
        severity = "CRITICAL",
        event = "CRITICAL_AUDIT",
        audit.action = %record.action,
        audit.school_id = ?record.school_id,
        audit.user_id = ?record.user_id,
        audit.resource_type = ?record.resource_type,
        audit.resource_id = ?record.resource_id,
        audit.ip_address = ?record.ip_address,
        audit.details = ?record.details,
        otel.status_code = "ERROR",
        "CRITICAL AUDIT EVENT: action={} school_id={:?} user_id={:?}",
        record.action,
        record.school_id,
        record.user_id
    );

    // 2. Append to telemetry buffer as CRITICAL_AUDIT event
    let telemetry_event = serde_json::json!({
        "school_id": record.school_id,
        "user_id": record.user_id,
        "session_id": format!("audit-{}", Uuid::new_v4()),
        "event_type": "CRITICAL_AUDIT",
        "url": None::<String>,
        "component": "audit_subsystem",
        "error_message": format!("CRITICAL AUDIT: {}", record.action),
        "error_stack": None::<String>,
        "duration_ms": None::<i32>,
        "metadata": {
            "action": record.action,
            "resource_type": record.resource_type,
            "resource_id": record.resource_id,
            "details": record.details,
            "ip_address": record.ip_address,
            "severity": "CRITICAL"
        }
    });

    if let Ok(json) = serde_json::to_string(&telemetry_event) {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("telemetry_buffer.jsonl")
        {
            let _ = writeln!(file, "{}", json);
        }
    }
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

    forward_to_critical_telemetry(&record);

    if let Ok(json) = serde_json::to_string(&record) {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("audit_buffer.jsonl")
        {
            let _ = writeln!(file, "{}", json);
        }
    }

    Ok(())
}

pub async fn log_security_event(
    school_id: Option<Uuid>,
    user_id: Option<Uuid>,
    action: &str,
    resource_type: Option<&str>,
    details: Option<serde_json::Value>,
    ip_address: Option<String>,
) -> Result<(), String> {
    let record = AuditRecord {
        school_id,
        user_id,
        action: action.to_string(),
        resource_type: resource_type.map(|s| s.to_string()),
        resource_id: None,
        details,
        ip_address,
    };

    forward_to_critical_telemetry(&record);

    if let Ok(json) = serde_json::to_string(&record) {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("audit_buffer.jsonl")
        {
            let _ = writeln!(file, "{}", json);
        }
    }

    Ok(())
}

pub async fn record_entity_change(
    pool: &PgPool,
    school_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
    action: &str,
    updated_by_user_id: Option<Uuid>,
    before_state: Option<serde_json::Value>,
    after_state: Option<serde_json::Value>,
    ip_address: Option<String>,
    user_agent: Option<String>,
) -> Result<Uuid, sqlx::Error> {
    use sqlx::Row;
    let row = sqlx::query(
        r#"
        INSERT INTO entity_change_history
        (school_id, entity_type, entity_id, action, updated_by_user_id, before_state, after_state, ip_address, user_agent)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id
        "#
    )
    .bind(school_id)
    .bind(entity_type)
    .bind(entity_id)
    .bind(action)
    .bind(updated_by_user_id)
    .bind(&before_state)
    .bind(&after_state)
    .bind(&ip_address)
    .bind(&user_agent)
    .fetch_one(pool)
    .await?;

    let history_id: Uuid = row.get("id");

    forward_to_critical_telemetry(&AuditRecord {
        school_id: Some(school_id),
        user_id: updated_by_user_id,
        action: action.to_string(),
        resource_type: Some(entity_type.to_string()),
        resource_id: Some(entity_id),
        details: Some(serde_json::json!({
            "before_state": before_state,
            "after_state": after_state,
            "user_agent": user_agent,
        })),
        ip_address,
    });

    Ok(history_id)
}
