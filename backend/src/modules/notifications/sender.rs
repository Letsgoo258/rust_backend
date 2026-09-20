use tokio::sync::mpsc::{Receiver, Sender};
use uuid::Uuid;

#[derive(Debug)]
pub struct SendTask {
    pub notification_id: Uuid,
    pub payload: String,
    pub notif_type: String,
    pub priority: i32,
}

#[derive(Debug)]
pub struct SendResult {
    pub notification_id: Uuid,
    pub success: bool,
    pub retryable: bool,
}

pub async fn run_sender(mut rx: Receiver<SendTask>, tx_result: Sender<SendResult>) {
    tracing::info!("Notification Sender Worker started");
    while let Some(task) = rx.recv().await {
        // Mock sending action
        tracing::info!("Sender executing notification: {} (Priority {})", task.notification_id, task.priority);
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        
        let success = true; // Pretend it succeeded
        
        let _ = tx_result.send(SendResult {
            notification_id: task.notification_id,
            success,
            retryable: !success,
        }).await;
    }
}
