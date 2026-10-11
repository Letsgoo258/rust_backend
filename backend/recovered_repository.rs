use sqlx::PgPool;

pub struct CronRepository;

impl CronRepository {
    /// Purge OTPs that expired more than 1 day ago
    pub async fn cleanup_expired_otps(pool: &PgPool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM admin_action_otps
            WHERE expires_at < NOW() - INTERVAL '1 day'
            "#
        )
        .execute(pool)
        .await;

        match result {
            Ok(res) => Ok(res.rows_affected()),
            Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("42P01") => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// Purge password resets that expired more than 7 days ago
    pub async fn cleanup_expired_password_resets(pool: &PgPool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM password_resets
            WHERE expires_at < NOW() - INTERVAL '7 days'
            "#
        )
        .execute(pool)
        .await;

        match result {
            Ok(res) => Ok(res.rows_affected()),
            Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("42P01") => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// Purge user sessions that are past their expiration timestamp
    pub async fn cleanup_expired_sessions(pool: &PgPool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM user_sessions
            WHERE expires_at < NOW()
            "#
        )
        .execute(pool)
        .await;

        match result {
            Ok(res) => Ok(res.rows_affected()),
            Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("42P01") => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// Purge telemetry entries older than retention_days (default 30)
    pub async fn cleanup_old_telemetry(pool: &PgPool, retention_days: i32) -> Result<u64, sqlx::Error> {
        let days = retention_days.max(1);
        let result = sqlx::query(
            r#"
            DELETE FROM frontend_telemetry
            WHERE timestamp < NOW() - ($1 || ' days')::INTERVAL
            "#
        )
        .bind(days.to_string())
        .execute(pool)
        .await;

        match result {
            Ok(res) => Ok(res.rows_affected()),
            Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("42P01") => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// Purge capability grants expired more than 30 days ago
    pub async fn cleanup_expired_capability_grants(pool: &PgPool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM user_capability_grants
            WHERE expires_at < NOW() - INTERVAL '30 days'
            "#
        )
        .execute(pool)
        .await;

        match result {
            Ok(res) => Ok(res.rows_affected()),
            Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("42P01") => Ok(0),
            Err(e) => Err(e),
        }
    }
}
