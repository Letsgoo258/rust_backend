use sqlx::PgPool;
use std::time::Duration;

pub async fn run_login_security_worker(pool: PgPool) {
    tracing::info!("Starting background 1-Hour Login Security & Pruning Engine...");

    loop {
        // Sleep for 1 hour
        tokio::time::sleep(Duration::from_secs(3600)).await;

        tracing::info!("Executing 1-Hour Security Maintenance: Anomaly Detection & 30-Day Pruning");

        // 1. ANOMALY DETECTION ENGINE: Flag logins for the same user from different IPs within 1 hour
        let flag_res = sqlx::query!(
            r#"
            UPDATE login_history lh
            SET is_suspicious = true,
                suspicious_reason = 'Flagged by 1-Hour Security Engine: Multiple logins from different IPs within 1 hour'
            WHERE id IN (
                SELECT l1.id
                FROM login_history l1
                JOIN login_history l2 ON l1.user_id = l2.user_id 
                    AND l1.id <> l2.id
                    AND l1.ip_address <> l2.ip_address
                    AND ABS(EXTRACT(EPOCH FROM (l1.created_at - l2.created_at))) <= 3600
                WHERE l1.created_at >= NOW() - INTERVAL '24 hours'
            )
            "#
        )
        .execute(&pool)
        .await;

        if let Ok(res) = flag_res {
            if res.rows_affected() > 0 {
                tracing::warn!("Security Engine: Flagged {} suspicious far-apart login records!", res.rows_affected());
            }
        }

        // 2. DATA RETENTION ENGINE: Delete login history older than 30 days
        let prune_res = sqlx::query!(
            r#"
            DELETE FROM login_history
            WHERE created_at < NOW() - INTERVAL '30 days'
            "#
        )
        .execute(&pool)
        .await;

        if let Ok(res) = prune_res {
            tracing::info!("Retention Engine: Pruned {} login history records older than 30 days", res.rows_affected());
        }
    }
}
