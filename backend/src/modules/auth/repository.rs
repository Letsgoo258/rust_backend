use crate::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

#[allow(dead_code)]
pub struct AuthRepository;

#[allow(dead_code)]
impl AuthRepository {
    pub async fn get_school_count(pool: &PgPool) -> Result<i64, AppError> {
        let count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM schools")
            .fetch_one(pool)
            .await?
            .unwrap_or(0);
        Ok(count)
    }

    pub async fn get_total_users(pool: &PgPool) -> Result<i64, AppError> {
        let count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM users")
            .fetch_one(pool)
            .await?
            .unwrap_or(0);
        Ok(count)
    }

    pub async fn get_active_tenants_count(pool: &PgPool) -> Result<i64, AppError> {
        let count: i64 =
            sqlx::query_scalar!("SELECT COUNT(*) FROM schools WHERE status = 'ACTIVE'")
                .fetch_one(pool)
                .await?
                .unwrap_or(0);
        Ok(count)
    }

    pub async fn school_exists(pool: &PgPool, school_id: Uuid) -> Result<bool, AppError> {
        let exists = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM schools WHERE id = $1)",
            school_id
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(false);
        Ok(exists)
    }

    pub async fn delete_school(pool: &PgPool, school_id: Uuid) -> Result<u64, AppError> {
        let res = sqlx::query!("DELETE FROM schools WHERE id = $1", school_id)
            .execute(pool)
            .await?;
        Ok(res.rows_affected())
    }

    pub async fn store_admin_otp(
        pool: &PgPool,
        user_id: Uuid,
        action_type: &str,
        target_id: Option<Uuid>,
        otp_code: &str,
    ) -> Result<(), AppError> {
        sqlx::query!(
            "DELETE FROM admin_action_otps WHERE user_id = $1 AND action_type = $2",
            user_id,
            action_type
        )
        .execute(pool)
        .await?;

        sqlx::query!(
            "INSERT INTO admin_action_otps (user_id, action_type, target_id, otp_code, expires_at) VALUES ($1, $2, $3, $4, NOW() + INTERVAL '10 minutes')",
            user_id,
            action_type,
            target_id,
            otp_code
        )
        .execute(pool)
        .await?;

        Ok(())
    }
}
