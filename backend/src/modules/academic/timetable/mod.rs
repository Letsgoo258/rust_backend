pub mod generator;
pub mod handler;
pub mod notification;
pub mod service;

#[allow(unused_imports)]
pub use generator::{AutoGenerateRequest, AutoGenerateResponse, GeneratedSlot, TimetableGenerator};
pub use handler::TimetableHandler;
pub use service::TimetableService;

/// Spawns the background worker that resets and syncs the daily timetable at 01:30 AM every morning.
pub fn start_daily_timetable_worker(pool: sqlx::PgPool) {
    tokio::spawn(async move {
        // Initial sync on boot for today's schedule
        let today = chrono::Utc::now().date_naive();
        match TimetableService::sync_daily_timetable_for_date(&pool, today).await {
            Ok(count) => {
                tracing::info!(
                    date = %today,
                    count = count,
                    "Initial daily timetable sync completed on startup"
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "Initial daily timetable sync check warning");
            }
        }

        // Periodic loop calculating duration until next 01:30 AM
        loop {
            let now = chrono::Local::now();
            let target_time = chrono::NaiveTime::from_hms_opt(1, 30, 0).unwrap();
            let target_date = if now.time() >= target_time {
                now.date_naive() + chrono::Duration::days(1)
            } else {
                now.date_naive()
            };

            let target_dt = target_date.and_time(target_time);
            let wait_duration = match target_dt.and_local_timezone(chrono::Local).single() {
                Some(dt) => (dt - now)
                    .to_std()
                    .unwrap_or(std::time::Duration::from_secs(3600)),
                None => std::time::Duration::from_secs(3600),
            };

            tokio::time::sleep(wait_duration).await;

            let sync_date = chrono::Utc::now().date_naive();
            tracing::info!(
                date = %sync_date,
                "Executing scheduled morning 01:30 AM daily timetable reset & synchronization..."
            );
            match TimetableService::sync_daily_timetable_for_date(&pool, sync_date).await {
                Ok(c) => tracing::info!(
                    date = %sync_date,
                    count = c,
                    "Morning 01:30 AM timetable reset completed successfully"
                ),
                Err(e) => tracing::error!(
                    date = %sync_date,
                    error = %e,
                    "Morning 01:30 AM timetable reset encountered error"
                ),
            }
        }
    });
}
