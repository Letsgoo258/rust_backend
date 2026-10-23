use crate::modules::auth::backend::{AuthSession, Backend, SessionLayer};
use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use axum_login::AuthManagerLayerBuilder;
use serde_json::json;
use sqlx::PgPool;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct TenantContext {
    pub school_id: Uuid,
    pub user_id: Uuid,
    pub is_platform_admin: bool,
}

pub async fn build_auth_layer(pool: PgPool) -> SessionLayer {
    let session_store = PostgresStore::new(pool.clone());

    // Avoid redundant advisory locks on every server boot if tower_sessions already exists
    let table_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'public' AND table_name = 'tower_sessions')"
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(false);

    if !table_exists {
        let _ = session_store.migrate().await;
    }

    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false) // Set to true in production with HTTPS
        .with_expiry(Expiry::OnInactivity(time::Duration::days(1)));

    let backend = Backend::new(pool);

    AuthManagerLayerBuilder::new(backend, session_layer).build()
}

/// Middleware to enforce strict multi-tenant school_id isolation on all authenticated requests.
pub async fn enforce_tenant_isolation_middleware(
    auth_session: AuthSession,
    mut req: Request,
    next: Next,
) -> Response {
    if let Some(ref user) = auth_session.user {
        let is_super_admin = user.roles.contains(&"SUPER_ADMIN".to_string());

        // Check if an explicit tenant/school header is specified in the request
        let requested_school_str = req
            .headers()
            .get("X-School-Id")
            .or_else(|| req.headers().get("X-Tenant-Id"))
            .and_then(|h| h.to_str().ok());

        if let Some(target_school_str) = requested_school_str {
            if let Ok(target_school_id) = target_school_str.parse::<Uuid>() {
                // Non-super-admins cannot access or switch to another tenant's school_id
                if !is_super_admin && target_school_id != user.school_id {
                    tracing::warn!(
                        user_id = %user.id,
                        user_school_id = %user.school_id,
                        attempted_school_id = %target_school_id,
                        "Cross-tenant access attempt blocked: User does not belong to requested school"
                    );

                    let client_ip = req
                        .headers()
                        .get("x-forwarded-for")
                        .and_then(|h| h.to_str().ok())
                        .map(|s| s.to_string());
                    crate::utils::audit::log_security_event(
                        Some(user.school_id),
                        Some(user.id),
                        "CROSS_TENANT_ATTEMPT_BLOCKED",
                        Some("MULTI_TENANT_ISOLATION"),
                        Some(json!({
                            "user_school_id": user.school_id,
                            "attempted_school_id": target_school_id,
                            "path": req.uri().path()
                        })),
                        client_ip,
                    )
                    .await
                    .ok();

                    return (
                        StatusCode::FORBIDDEN,
                        Json(json!({
                            "error": "Cross-tenant access forbidden: You do not have permission to access data belonging to another school.",
                            "code": "CROSS_TENANT_FORBIDDEN"
                        })),
                    )
                        .into_response();
                }
            }
        }

        // Attach TenantContext and school_id to Request extensions for downstream handlers
        let tenant_ctx = TenantContext {
            school_id: user.school_id,
            user_id: user.id,
            is_platform_admin: is_super_admin,
        };
        req.extensions_mut().insert(tenant_ctx);
        req.extensions_mut().insert(user.school_id);

        let user_school_id_str = user.school_id.to_string();
        let mut res = next.run(req).await;

        if let Ok(val) = HeaderValue::from_str(&user_school_id_str) {
            res.headers_mut().insert("X-School-Id", val);
        }

        return res;
    }

    next.run(req).await
}
