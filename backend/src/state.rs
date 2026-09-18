use crate::config::AppConfig;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<AppConfig>,
}

impl AppState {
    pub fn new(db: PgPool, config: AppConfig) -> Self {
        Self {
            db,
            config: Arc::new(config),
        }
    }
}
