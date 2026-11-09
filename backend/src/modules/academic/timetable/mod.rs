pub mod generator;
pub mod handler;
pub mod notification;
pub mod repository;
pub mod service;

#[allow(unused_imports)]
pub use generator::{AutoGenerateRequest, AutoGenerateResponse, GeneratedSlot, TimetableGenerator};
pub use handler::TimetableHandler;
#[allow(unused_imports)]
pub use repository::TimetableRepository;
pub use service::TimetableService;

/// Spawns the background worker that resets and syncs the daily timetable for each school's local date.
pub fn start_daily_timetable_worker(pool: sqlx::PgPool) {
    tokio::spawn(async move {
        // Initial sync on boot for schools' current local dates
        let sync_dates: Vec<chrono::NaiveDate> = sqlx::query_scalar(
            "SELECT DISTINCT (NOW() AT TIME ZONE COALESCE(timezone, 'Asia/Kolkata'))::date FROM schools",
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_else(|_| vec![chrono::Local::now().date_naive()]);

        for sync_date in sync_dates {
            match TimetableService::sync_daily_timetable_for_date(&pool, sync_date).await {
                Ok(count) => {
                    tracing::info!(
                        date = %sync_date,
                        count = count,
                        "Initial daily timetable sync completed on startup"
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Initial daily timetable sync check warning");
                }
            }
        }

        // Periodic maintenance loop: runs every 30 minutes to ensure current_day_timetable is populated
        // and doesn't get missed if the machine sleeps or timezone boundaries change.
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1800)).await;

            let current_dates: Vec<chrono::NaiveDate> = sqlx::query_scalar(
                "SELECT DISTINCT (NOW() AT TIME ZONE COALESCE(timezone, 'Asia/Kolkata'))::date FROM schools",
            )
            .fetch_all(&pool)
            .await
            .unwrap_or_else(|_| vec![chrono::Local::now().date_naive()]);

            for sync_date in current_dates {
                let count: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM current_day_timetable WHERE date = $1",
                )
                .bind(sync_date)
                .fetch_one(&pool)
                .await
                .unwrap_or(0);

                if count == 0 {
                    tracing::info!(
                        date = %sync_date,
                        "Daily timetable empty for date, automatically synchronizing..."
                    );
                    let _ = TimetableService::sync_daily_timetable_for_date(&pool, sync_date).await;
                }
            }
        }
    });
}
