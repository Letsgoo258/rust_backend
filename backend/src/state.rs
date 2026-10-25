use crate::config::AppConfig;
use crate::utils::concurrency::KeyLockManager;
use crate::utils::jobs::{create_job_store, JobStore};
use crate::utils::whatsapp::WhatsAppService;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
#[allow(dead_code)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<AppConfig>,
    pub jobs: JobStore,
    pub key_locks: KeyLockManager,
    pub whatsapp: Arc<RwLock<Option<Arc<WhatsAppService>>>>,
}

impl AppState {
    pub fn new(db: PgPool, config: AppConfig) -> Self {
        Self {
            db,
            config: Arc::new(config),
            jobs: create_job_store(),
            key_locks: KeyLockManager::new(),
            whatsapp: Arc::new(RwLock::new(None)),
        }
    }
}
