use sqlx::PgPool;
use std::time::Duration;
use sysinfo::System;
use tokio::sync::mpsc::{Receiver, Sender};

use super::repository::NotificationRepository;
use super::sender::{SendResult, SendTask};

#[allow(dead_code)]
pub async fn run_manager(
    pool: PgPool,
    tx_sender: Sender<SendTask>,
    mut rx_result: Receiver<SendResult>,
) {
    tracing::info!("Notification Manager Worker started");
    let mut sys = System::new();

    loop {
        // Process any results from the Sender
        while let Ok(result) = rx_result.try_recv() {
            if result.success {
                let _ = NotificationRepository::delete_notification(&pool, result.notification_id)
                    .await;
            } else {
                let _ = NotificationRepository::mark_retry(&pool, result.notification_id).await;
            }
        }

        // Check load
        sys.refresh_cpu_usage();
        let total_cpu: f32 = sys.cpus().iter().map(|c| c.cpu_usage()).sum();
        let avg_cpu = if !sys.cpus().is_empty() {
            total_cpu / sys.cpus().len() as f32
        } else {
            0.0
        };
        let is_heavy = avg_cpu > 80.0;

        // Fetch pending notifications via NotificationRepository
        let pending = NotificationRepository::fetch_pending(&pool, 5)
            .await
            .unwrap_or_default();

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

            let _ = NotificationRepository::mark_sent(&pool, notif.id).await;
            let _ = tx_sender.send(task).await;
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
