use super::handler::TelemetryRecord;
use crate::error::AppError;
use sqlx::PgPool;

pub struct TelemetryRepository;

impl TelemetryRepository {
    pub async fn insert_telemetry_record(
        pool: &PgPool,
        record: &TelemetryRecord,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO frontend_telemetry
            (school_id, user_id, session_id, event_type, url, component, error_message, error_stack, duration_ms, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#
        )
        .bind(record.school_id)
        .bind(record.user_id)
        .bind(&record.session_id)
        .bind(&record.event_type)
        .bind(&record.url)
        .bind(&record.component)
        .bind(&record.error_message)
        .bind(&record.error_stack)
        .bind(record.duration_ms)
        .bind(&record.metadata)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn insert_audit_record(
        pool: &PgPool,
        record: &crate::utils::audit::AuditRecord,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO audit_logs (school_id, user_id, action, resource_type, resource_id, details, ip_address)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
        .bind(record.school_id)
        .bind(record.user_id)
        .bind(&record.action)
        .bind(&record.resource_type)
        .bind(record.resource_id)
        .bind(&record.details)
        .bind(&record.ip_address)
        .execute(pool)
        .await?;

        Ok(())
    }
}
