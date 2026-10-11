use sqlx::PgPool;
use std::fs;
use std::io::{BufRead, BufReader};

use super::handler::TelemetryRecord;
use super::repository::TelemetryRepository;

pub async fn run_telemetry_batch_worker(pool: PgPool) {
    tracing::info!("Starting background telemetry worker (Scheduled for every 30 mins)...");

    loop {
        // Sleep for 30 minutes
        tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;

        // Time to flush!
        tracing::info!("30 minutes reached. Flushing telemetry and audit buffers to DB...");
        flush_telemetry(&pool).await;
        flush_audit_logs(&pool).await;
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
                if TelemetryRepository::insert_telemetry_record(pool, &record)
                    .await
                    .is_ok()
                {
                    success_count += 1;
                }
            }
        }
    }

    tracing::info!(
        "Successfully batch-inserted {} telemetry records",
        success_count
    );
    let _ = fs::remove_file(processing_file);
}

pub async fn flush_audit_logs(pool: &PgPool) {
    let file_path = "audit_buffer.jsonl";
    if !std::path::Path::new(file_path).exists() {
        return;
    }

    let processing_file = "audit_processing.jsonl";
    if let Err(e) = fs::rename(file_path, processing_file) {
        tracing::error!("Failed to rename audit buffer: {}", e);
        return;
    }

    let file = match fs::File::open(processing_file) {
        Ok(f) => f,
        Err(_) => return,
    };

    let reader = BufReader::new(file);
    let mut success_count = 0;

    for line_result in reader.lines() {
        if let Ok(line) = line_result {
            if let Ok(record) = serde_json::from_str::<crate::utils::audit::AuditRecord>(&line) {
                if TelemetryRepository::insert_audit_record(pool, &record)
                    .await
                    .is_ok()
                {
                    success_count += 1;
                }
            }
        }
    }

    tracing::info!("Successfully batch-inserted {} audit logs", success_count);
    let _ = fs::remove_file(processing_file);
}
