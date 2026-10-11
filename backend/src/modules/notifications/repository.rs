use crate::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

pub struct NotificationPendingRow {
    pub id: Uuid,
    pub message: String,
    pub notif_type: String,
    pub priority: i32,
}

pub struct NotificationRepository;

impl NotificationRepository {
    pub async fn delete_notification(pool: &PgPool, notification_id: Uuid) -> Result<(), AppError> {
        sqlx::query("DELETE FROM notifications WHERE id = $1")
            .bind(notification_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn mark_retry(pool: &PgPool, notification_id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE notifications SET status = 'PENDING', priority = priority + 1, retry_count = retry_count + 1 WHERE id = $1"
        )
        .bind(notification_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn mark_sent(pool: &PgPool, notification_id: Uuid) -> Result<(), AppError> {
        sqlx::query("UPDATE notifications SET status = 'SENT' WHERE id = $1")
            .bind(notification_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn fetch_pending(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<NotificationPendingRow>, AppError> {
        use sqlx::Row;
        let rows = sqlx::query(
            r#"
            SELECT id, message, notification_type as notif_type, priority
            FROM notifications
            WHERE status = 'PENDING'
            ORDER BY priority ASC, created_at ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| NotificationPendingRow {
                id: r.get("id"),
                message: r.get("message"),
                notif_type: r.get("notif_type"),
                priority: r.get("priority"),
            })
            .collect();

        Ok(list)
    }
}
