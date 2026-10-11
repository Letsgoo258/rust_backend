use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    PENDING,
    PROCESSING,
    COMPLETED,
    FAILED,
    INTERRUPTED,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkUploadJob {
    pub id: Uuid,
    pub school_id: Uuid,
    pub user_id: Uuid,
    pub job_type: String,
    pub status: JobStatus,
    pub total_rows: usize,
    pub processed_rows: usize,
    pub success_count: usize,
    pub failed_count: usize,
    pub failed_csv: Option<String>,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub type JobStore = Arc<DashMap<Uuid, BulkUploadJob>>;

pub fn create_job_store() -> JobStore {
    Arc::new(DashMap::new())
}

/// Helper to persist job state into PostgreSQL background_jobs table (non-cluttered, auto-purged)
pub async fn save_job_to_db(
    pool: &PgPool,
    job: &BulkUploadJob,
    payload_json: Option<&serde_json::Value>,
) {
    let status_str = match job.status {
        JobStatus::PENDING => "PENDING",
        JobStatus::PROCESSING => "PROCESSING",
        JobStatus::COMPLETED => "COMPLETED",
        JobStatus::FAILED => "FAILED",
        JobStatus::INTERRUPTED => "INTERRUPTED",
    };

    let _ = sqlx::query(
        r#"
        INSERT INTO background_jobs (
            id, school_id, user_id, job_type, status,
            total_rows, processed_rows, success_count, failed_count,
            failed_csv, error, payload_json, created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW(), NOW())
        ON CONFLICT (id) DO UPDATE SET
            status = EXCLUDED.status,
            processed_rows = EXCLUDED.processed_rows,
            success_count = EXCLUDED.success_count,
            failed_count = EXCLUDED.failed_count,
            failed_csv = EXCLUDED.failed_csv,
            error = EXCLUDED.error,
            updated_at = NOW()
        "#,
    )
    .bind(job.id)
    .bind(job.school_id)
    .bind(job.user_id)
    .bind(&job.job_type)
    .bind(status_str)
    .bind(job.total_rows as i32)
    .bind(job.processed_rows as i32)
    .bind(job.success_count as i32)
    .bind(job.failed_count as i32)
    .bind(&job.failed_csv)
    .bind(&job.error)
    .bind(payload_json)
    .execute(pool)
    .await;
}

/// Helper to fetch job from DB if server restarted and memory cache missed
pub async fn get_job_from_db(pool: &PgPool, job_id: Uuid) -> Option<BulkUploadJob> {
    use sqlx::Row;
    let row = sqlx::query(
        r#"
        SELECT id, school_id, user_id, job_type, status,
               total_rows, processed_rows, success_count, failed_count,
               failed_csv, error, EXTRACT(EPOCH FROM created_at)::BIGINT as created_at,
               EXTRACT(EPOCH FROM updated_at)::BIGINT as updated_at
        FROM background_jobs
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None)?;

    let status_str: String = row.get("status");
    let status = match status_str.as_str() {
        "PENDING" => JobStatus::PENDING,
        "PROCESSING" => JobStatus::PROCESSING,
        "COMPLETED" => JobStatus::COMPLETED,
        "FAILED" => JobStatus::FAILED,
        _ => JobStatus::INTERRUPTED,
    };

    Some(BulkUploadJob {
        id: row.get("id"),
        school_id: row.get("school_id"),
        user_id: row.get("user_id"),
        job_type: row.get("job_type"),
        status,
        total_rows: row.get::<i32, _>("total_rows") as usize,
        processed_rows: row.get::<i32, _>("processed_rows") as usize,
        success_count: row.get::<i32, _>("success_count") as usize,
        failed_count: row.get::<i32, _>("failed_count") as usize,
        failed_csv: row.get("failed_csv"),
        error: row.get("error"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

/// Sweeps DB on server startup to handle interrupted jobs & purge old records (>3 days) to keep DB clean
pub async fn recover_and_clean_jobs(pool: &PgPool, memory_store: JobStore) {
    use sqlx::Row;
    // 1. Auto-purge completed/interrupted/failed jobs older than 3 days (Zero clutter)
    let _ = sqlx::query("DELETE FROM background_jobs WHERE updated_at < NOW() - INTERVAL '3 days'")
        .execute(pool)
        .await;

    // 2. Mark any jobs that were stuck in PROCESSING when server unexpectedly restarted as INTERRUPTED
    let _ = sqlx::query(
        "UPDATE background_jobs SET status = 'INTERRUPTED', error = 'Server process restarted during background execution' WHERE status = 'PROCESSING'"
    )
    .execute(pool)
    .await;

    // 3. Populate memory store with recent active/recent jobs
    if let Ok(recent_jobs) = sqlx::query(
        r#"
        SELECT id, school_id, user_id, job_type, status,
               total_rows, processed_rows, success_count, failed_count,
               failed_csv, error, EXTRACT(EPOCH FROM created_at)::BIGINT as created_at,
               EXTRACT(EPOCH FROM updated_at)::BIGINT as updated_at
        FROM background_jobs
        WHERE updated_at >= NOW() - INTERVAL '1 day'
        "#,
    )
    .fetch_all(pool)
    .await
    {
        for row in recent_jobs {
            let status_str: String = row.get("status");
            let status = match status_str.as_str() {
                "PENDING" => JobStatus::PENDING,
                "PROCESSING" => JobStatus::PROCESSING,
                "COMPLETED" => JobStatus::COMPLETED,
                "FAILED" => JobStatus::FAILED,
                _ => JobStatus::INTERRUPTED,
            };

            let id: Uuid = row.get("id");
            memory_store.insert(
                id,
                BulkUploadJob {
                    id,
                    school_id: row.get("school_id"),
                    user_id: row.get("user_id"),
                    job_type: row.get("job_type"),
                    status,
                    total_rows: row.get::<i32, _>("total_rows") as usize,
                    processed_rows: row.get::<i32, _>("processed_rows") as usize,
                    success_count: row.get::<i32, _>("success_count") as usize,
                    failed_count: row.get::<i32, _>("failed_count") as usize,
                    failed_csv: row.get("failed_csv"),
                    error: row.get("error"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                },
            );
        }
    }
}
