use sqlx::PgPool;
use uuid::Uuid;

pub async fn log_action(
    pool: &PgPool,
    school_id: Option<Uuid>,
    user_id: Option<Uuid>,
    action: &str,
    resource_type: Option<&str>,
    resource_id: Option<Uuid>,
    details: Option<serde_json::Value>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO audit_logs (school_id, user_id, action, resource_type, resource_id, details)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        school_id,
        user_id,
        action,
        resource_type,
        resource_id,
        details
    )
    .execute(pool)
    .await?;

    Ok(())
}
