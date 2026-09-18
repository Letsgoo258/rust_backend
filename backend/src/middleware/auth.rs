use crate::modules::auth::backend::{Backend, SessionLayer};
use axum_login::AuthManagerLayerBuilder;
use sqlx::PgPool;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;

pub async fn build_auth_layer(pool: PgPool) -> SessionLayer {
    let session_store = PostgresStore::new(pool.clone());
    session_store.migrate().await.unwrap();

    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false) // Set to true in production with HTTPS
        .with_expiry(Expiry::OnInactivity(time::Duration::days(1)));

    let backend = Backend::new(pool);

    AuthManagerLayerBuilder::new(backend, session_layer).build()
}
