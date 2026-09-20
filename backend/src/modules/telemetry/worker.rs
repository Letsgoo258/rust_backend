use sqlx::PgPool;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use time::{OffsetDateTime, Duration, Time};
use super::handler::TelemetryRecord;

pub async fn run_telemetry_batch_worker(pool: PgPool) {
    tracing::info!("Starting background telemetry worker (Scheduled for 23:50 IST)...");
    
    loop {
        // Calculate time until next 23:50 IST (UTC+5:30)
        // 23:50 IST is 18:20 UTC
        let now = OffsetDateTime::now_utc();
        let target_time = Time::from_hms(18, 20, 0).unwrap();
        let current_date = now.date();
        let mut target = current_date.with_time(target_time).assume_utc();
        
        if now >= target {
            target = target + Duration::days(1);
        }
        
        let wait_duration = target - now;
        
        // Sleep until 23:50 IST
        tokio::time::sleep(std::time::Duration::from_secs(wait_duration.whole_seconds() as u64)).await;
        
        // Time to flush!
        tracing::info!("23:50 IST reached. Flushing telemetry buffer to DB...");
        flush_telemetry(&pool).await;
    }
}

pub async fn flush_telemetry(pool: &PgPool) {
    let file_path = "telemetry_buffer.jsonl";
    if !std::path::Path::new(file_path).exists() {
        return;
    }
    
    // Rename file to prevent incoming requests from writing to it while we process
    let processing_file = "telemetry_processing.jsonl";
    if let Err(e) = fs::rename(file_path, processing_file) {
        tracing::error!("Failed to rename telemetry buffer: {}", e);
        return;
    }
    
    let file = match fs::File::open(processing_file) {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("Failed to open processing file: {}", e);
            return;
        }
    };
    
    let reader = BufReader::new(file);
    let mut success_count = 0;
    
    for line_result in reader.lines() {
        if let Ok(line) = line_result {
            if let Ok(record) = serde_json::from_str::<TelemetryRecord>(&line) {
                let res = sqlx::query!(
                    r#"
                    INSERT INTO frontend_telemetry 
                    (school_id, user_id, session_id, event_type, url, component, error_message, error_stack, duration_ms, metadata)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                    "#,
                    record.school_id,
                    record.user_id,
                    record.session_id,
                    record.event_type,
                    record.url,
                    record.component,
                    record.error_message,
                    record.error_stack,
                    record.duration_ms,
                    record.metadata
                )
                .execute(pool)
                .await;
                
                if res.is_ok() {
                    success_count += 1;
                }
            }
        }
    }
    
    tracing::info!("Successfully batch-inserted {} telemetry records", success_count);
    let _ = fs::remove_file(processing_file);
}
