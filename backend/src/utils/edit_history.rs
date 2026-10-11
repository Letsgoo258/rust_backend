use crate::error::AppError;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

/// Edit history record containing before and after states for 15-day compliance
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditHistoryRecord {
    pub id: Uuid,
    pub school_id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub action: String, // "CREATE", "UPDATE", "DELETE"
    pub updated_by_user_id: Option<Uuid>,
    pub updated_by_name: Option<String>,
    pub before_state: Option<serde_json::Value>,
    pub after_state: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: String,
}

#[allow(dead_code)]
pub struct EditHistoryService;

#[allow(dead_code)]
impl EditHistoryService {
    /// Records an edit history entry capturing the row ID, old values, and new values
    pub async fn record_edit(
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
    ) -> Result<Uuid, AppError> {
        let row = sqlx::query(
            r#"
            INSERT INTO entity_change_history
            (school_id, entity_type, entity_id, action, updated_by_user_id, before_state, after_state, ip_address, user_agent, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
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

        // Forward to structured telemetry and audit
        let _ = crate::utils::audit::log_action(
            pool,
            Some(school_id),
            updated_by_user_id,
            action,
            Some(entity_type),
            Some(entity_id),
            Some(serde_json::json!({
                "history_id": history_id,
                "before_state": before_state,
                "after_state": after_state,
            })),
            ip_address,
        )
        .await;

        Ok(history_id)
    }

    /// Prunes edit history records older than retention_days (default 15 days for compliance)
    pub async fn prune_older_than(pool: &PgPool, retention_days: i32) -> Result<u64, sqlx::Error> {
        let days = retention_days.max(1);
        let result = sqlx::query(
            r#"
            DELETE FROM entity_change_history
            WHERE created_at < NOW() - ($1 || ' days')::INTERVAL
            "#,
        )
        .bind(days.to_string())
        .execute(pool)
        .await;

        match result {
            Ok(res) => Ok(res.rows_affected()),
            Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("42P01") => {
                Ok(0)
            }
            Err(e) => Err(e),
        }
    }

    /// Retrieves 15-day edit history for a specific row/entity
    pub async fn get_entity_history(
        pool: &PgPool,
        school_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        limit: i64,
    ) -> Result<Vec<EditHistoryRecord>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                h.id,
                h.school_id,
                h.entity_type,
                h.entity_id,
                h.action,
                h.updated_by_user_id,
                u.display_name as updated_by_name,
                h.before_state,
                h.after_state,
                h.ip_address,
                h.user_agent,
                h.created_at
            FROM entity_change_history h
            LEFT JOIN users u ON h.updated_by_user_id = u.id
            WHERE h.school_id = $1
              AND h.entity_type = $2
              AND h.entity_id = $3
              AND h.created_at >= NOW() - INTERVAL '15 days'
            ORDER BY h.created_at DESC
            LIMIT $4
            "#,
        )
        .bind(school_id)
        .bind(entity_type)
        .bind(entity_id)
        .bind(limit.max(1).min(100))
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| {
                let dt: OffsetDateTime = r.get("created_at");
                EditHistoryRecord {
                    id: r.get("id"),
                    school_id: r.get("school_id"),
                    entity_type: r.get("entity_type"),
                    entity_id: r.get("entity_id"),
                    action: r.get("action"),
                    updated_by_user_id: r.get("updated_by_user_id"),
                    updated_by_name: r.get("updated_by_name"),
                    before_state: r.get("before_state"),
                    after_state: r.get("after_state"),
                    ip_address: r.get("ip_address"),
                    user_agent: r.get("user_agent"),
                    created_at: dt.to_string(),
                }
            })
            .collect();

        Ok(list)
    }

    /// Retrieves recent school-wide edit history (for compliance reporting)
    pub async fn get_school_recent_history(
        pool: &PgPool,
        school_id: Uuid,
        limit: i64,
    ) -> Result<Vec<EditHistoryRecord>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                h.id,
                h.school_id,
                h.entity_type,
                h.entity_id,
                h.action,
                h.updated_by_user_id,
                u.display_name as updated_by_name,
                h.before_state,
                h.after_state,
                h.ip_address,
                h.user_agent,
                h.created_at
            FROM entity_change_history h
            LEFT JOIN users u ON h.updated_by_user_id = u.id
            WHERE h.school_id = $1
              AND h.created_at >= NOW() - INTERVAL '15 days'
            ORDER BY h.created_at DESC
            LIMIT $2
            "#,
        )
        .bind(school_id)
        .bind(limit.max(1).min(200))
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| {
                let dt: OffsetDateTime = r.get("created_at");
                EditHistoryRecord {
                    id: r.get("id"),
                    school_id: r.get("school_id"),
                    entity_type: r.get("entity_type"),
                    entity_id: r.get("entity_id"),
                    action: r.get("action"),
                    updated_by_user_id: r.get("updated_by_user_id"),
                    updated_by_name: r.get("updated_by_name"),
                    before_state: r.get("before_state"),
                    after_state: r.get("after_state"),
                    ip_address: r.get("ip_address"),
                    user_agent: r.get("user_agent"),
                    created_at: dt.to_string(),
                }
            })
            .collect();

        Ok(list)
    }
}
