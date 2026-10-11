use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CronJobResult {
    pub task_name: String,
    pub records_affected: u64,
    pub status: String,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CronSummaryResponse {
    pub status: String,
    pub executed_at: String,
    pub duration_ms: u128,
    pub total_records_affected: u64,
    pub results: Vec<CronJobResult>,
}

#[derive(Debug, Deserialize)]
pub struct CronOptionsQuery {
    pub telemetry_retention_days: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FifteenDaysCleanupMetrics {
    pub edit_histories_purged: u64,
    pub notifications_purged: u64,
    pub otps_purged: u64,
    pub password_resets_purged: u64,
    pub telemetry_purged: u64,
    pub total_records_purged: u64,
}
