use crate::error::AppError;
use sqlx::{PgPool, Postgres, Transaction};

/// Industry-Grade Database Write Controller
///
/// Intercepts all database write actions to enforce:
/// 1. ACID Transaction Envelope guarantees (Managed Begin -> Commit / Rollback)
/// 2. Clean Database Constraint Error normalisation
/// 3. Concurrency-safe execution
pub struct DbWriteController;

pub struct TxGuard<'a> {
    tx: Option<Transaction<'a, Postgres>>,
}

impl<'a> TxGuard<'a> {
    pub async fn new(pool: &'a PgPool) -> Result<Self, AppError> {
        let tx = pool.begin().await.map_err(AppError::Database)?;
        Ok(Self { tx: Some(tx) })
    }

    pub fn conn(&mut self) -> &mut sqlx::PgConnection {
        self.tx.as_mut().expect("Transaction already finished")
    }

    #[allow(dead_code)]
    pub fn tx(&mut self) -> &mut Transaction<'a, Postgres> {
        self.tx.as_mut().expect("Transaction already finished")
    }

    pub async fn commit(mut self) -> Result<(), AppError> {
        if let Some(tx) = self.tx.take() {
            tx.commit().await.map_err(AppError::Database)?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn rollback(mut self) -> Result<(), AppError> {
        if let Some(tx) = self.tx.take() {
            tx.rollback().await.map_err(AppError::Database)?;
        }
        Ok(())
    }
}

impl<'a> std::ops::Deref for TxGuard<'a> {
    type Target = sqlx::PgConnection;
    fn deref(&self) -> &Self::Target {
        self.tx.as_ref().expect("Transaction already finished")
    }
}

impl<'a> std::ops::DerefMut for TxGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.tx.as_mut().expect("Transaction already finished")
    }
}

impl DbWriteController {
    /// Starts a managed database transaction with ACID envelope tracking.
    pub async fn begin<'a>(pool: &'a PgPool) -> Result<TxGuard<'a>, AppError> {
        TxGuard::new(pool).await
    }

    /// Helper for checking if an error is a unique constraint violation and extracting the constraint name.
    #[allow(dead_code)]
    pub fn parse_constraint_violation(err: &sqlx::Error) -> Option<String> {
        if let Some(db_err) = err.as_database_error() {
            if let Some(constraint) = db_err.constraint() {
                return Some(constraint.to_string());
            }
        }
        None
    }
}
