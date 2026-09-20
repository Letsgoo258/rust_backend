use sqlx::PgPool;
use tokio::sync::mpsc::{Sender, Receiver};
use sysinfo::System;
use std::time::Duration;

use super::sender::{SendTask, SendResult};

pub async fn run_manager(
    pool: PgPool, 
    tx_sender: Sender<SendTask>, 
    mut rx_result: Receiver<SendResult>
) {
    tracing::info!("Notification Manager Worker started");
    let mut sys = System::new();
    
    loop {
        // Process any results from the Sender
        while let Ok(result) = rx_result.try_recv() {
            if result.success {
                // Delete or mark SENT
                let _ = sqlx::query!("DELETE FROM notifications WHERE id = $1", result.notification_id)
                    .execute(&pool).await;
            } else {
                // Handle retry
                let _ = sqlx::query!(
                    "UPDATE notifications SET status = 'PENDING', priority = priority + 1, retry_count = retry_count + 1 WHERE id = $1",
                    result.notification_id
                ).execute(&pool).await;
            }
        }
        
        // Check load
        sys.refresh_cpu_usage();
        let total_cpu: f32 = sys.cpus().iter().map(|c| c.cpu_usage()).sum();
        let avg_cpu = if !sys.cpus().is_empty() { total_cpu / sys.cpus().len() as f32 } else { 0.0 };
        let is_heavy = avg_cpu > 80.0;
        
        // Fetch pending notifications
        // Priority 1 = Critical
        let pending = sqlx::query!(
            r#"SELECT id, message, notification_type as "notif_type: String", priority 
               FROM notifications 
               WHERE status = 'PENDING' 
               ORDER BY priority ASC, created_at ASC 
               LIMIT 5"#
        ).fetch_all(&pool).await.unwrap_or_default();
        
        if pending.is_empty() {
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        }
        
        let has_critical = pending.iter().any(|n| n.priority == 1);
        
        if is_heavy && !has_critical {
            tracing::warn!("System load is high ({:.1}%) and no critical notifications. Sleeping for 15 seconds.", avg_cpu);
            tokio::time::sleep(Duration::from_secs(15)).await;
            continue;
        }
        
        for notif in pending {
            let task = SendTask {
                notification_id: notif.id,
                payload: notif.message.clone(),
                notif_type: notif.notif_type.clone(),
                priority: notif.priority,
            };
            
            // Mark as processing (by setting status to SENT temporarily, if it fails it will revert)
            // A better way is setting status to PROCESSING, but we only have PENDING/SENT/FAILED. Let's just set to SENT.
            let _ = sqlx::query!("UPDATE notifications SET status = 'SENT' WHERE id = $1", notif.id).execute(&pool).await;
            
            let _ = tx_sender.send(task).await;
        }
        
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
