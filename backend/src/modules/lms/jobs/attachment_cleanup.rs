use crate::utils::imagekit::ImageKitClient;
use sqlx::{PgPool, Row};
use tracing::{error, info};

/// Background job to clean up temporary LMS attachments past 7 days
pub async fn run_expired_attachments_cleanup(pool: PgPool) {
    info!("Starting LMS expired attachments cleanup worker loop");
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600)); // Run hourly

    loop {
        interval.tick().await;

        let expired_files = match sqlx::query(
            r#"
            SELECT id, school_id, task_id, imagekit_file_id, file_url
            FROM lms_task_attachments
            WHERE attachment_type = 'TEMPORARY_UPLOAD'
              AND deletion_status != 'DELETED'
              AND expires_at IS NOT NULL
              AND expires_at <= NOW()
            LIMIT 50
            "#,
        )
        .fetch_all(&pool)
        .await
        {
            Ok(files) => files,
            Err(e) => {
                error!("Failed to fetch expired LMS attachments: {:?}", e);
                continue;
            }
        };

        if expired_files.is_empty() {
            continue;
        }

        info!(
            "Found {} expired LMS attachments to clean up",
            expired_files.len()
        );

        for file in expired_files {
            let file_id: uuid::Uuid = file.get("id");
            let ik_id: Option<String> = file.try_get("imagekit_file_id").ok();
            let mut deletion_succeeded = false;

            if let Some(ref id) = ik_id {
                match ImageKitClient::delete_file(id, Some(&pool)).await {
                    Ok(_) => {
                        deletion_succeeded = true;
                        info!(attachment_id = %file_id, "Successfully deleted file from ImageKit");
                    }
                    Err(e) => {
                        error!(attachment_id = %file_id, error = ?e, "Failed to delete file from ImageKit");
                    }
                }
            } else {
                // No imagekit file ID, mark as deleted directly
                deletion_succeeded = true;
            }

            let new_status = if deletion_succeeded {
                "DELETED"
            } else {
                "FAILED_DELETION"
            };

            let _ = sqlx::query(
                r#"
                UPDATE lms_task_attachments
                SET deletion_status = $1, deleted_at = NOW()
                WHERE id = $2
                "#,
            )
            .bind(new_status)
            .bind(file_id)
            .execute(&pool)
            .await;
        }
    }
}
