use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::get,
    Extension, Json, Router,
};
use eravaya::middleware::auth::TenantContext;
use tower::ServiceExt;
use uuid::Uuid;

async fn dummy_protected_handler(Extension(tenant): Extension<TenantContext>) -> impl IntoResponse {
    Json(serde_json::json!({
        "school_id": tenant.school_id,
        "user_id": tenant.user_id,
        "is_platform_admin": tenant.is_platform_admin
    }))
}

#[tokio::test]
async fn test_tenant_context_injection() {
    let school_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    let tenant_ctx = TenantContext {
        school_id,
        user_id,
        is_platform_admin: false,
    };

    let app = Router::new()
        .route("/test-tenant", get(dummy_protected_handler))
        .layer(Extension(tenant_ctx.clone()));

    let req = Request::builder()
        .uri("/test-tenant")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
