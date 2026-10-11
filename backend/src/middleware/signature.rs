use crate::{modules::auth::backend::AuthSession, utils::url_signer::UrlSigner};
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, Debug, Default)]
pub struct UrlSignatureQuery {
    pub _sig: Option<String>,
    pub _ts: Option<u64>,
}

/// Middleware to enforce or validate rotating URL signatures on sensitive / dashboard operations
pub async fn verify_url_signature_middleware(
    auth_session: AuthSession,
    req: Request,
    next: Next,
) -> Response {
    let user = match auth_session.user {
        Some(u) => u,
        None => {
            // Not authenticated yet - pass through to auth handler / layer
            return next.run(req).await;
        }
    };

    // Extract signature from Header or Query
    let header_sig = req
        .headers()
        .get("X-Url-Signature")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let header_ts = req
        .headers()
        .get("X-Signature-Ts")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    let query_params: Option<UrlSignatureQuery> = req
        .uri()
        .query()
        .and_then(|q| serde_urlencoded::from_str(q).ok());

    let sig = header_sig.or_else(|| query_params.as_ref().and_then(|q| q._sig.clone()));
    let ts = header_ts.or_else(|| query_params.as_ref().and_then(|q| q._ts));

    // If signature parameters are provided, strictly verify them
    if let (Some(signature), Some(epoch)) = (sig, ts) {
        if let Err(e) = UrlSigner::verify(user.id, user.school_id, &signature, epoch) {
            tracing::warn!(
                user_id = %user.id,
                school_id = %user.school_id,
                error = %e,
                "Access blocked: URL signature validation failed"
            );

            // Record security audit event
            let client_ip = req
                .headers()
                .get("x-forwarded-for")
                .and_then(|h| h.to_str().ok())
                .map(|s| s.to_string());
            crate::utils::audit::log_security_event(
                Some(user.school_id),
                Some(user.id),
                "TAMPERED_URL_SIGNATURE",
                Some("URL_SECURITY"),
                Some(json!({
                    "error": e.to_string(),
                    "signature": signature,
                    "epoch": epoch,
                    "path": req.uri().path()
                })),
                client_ip,
            )
            .await
            .ok();

            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": "Invalid, expired, or tampered URL signature. Access denied.",
                    "code": "TAMPERED_URL_SIGNATURE"
                })),
            )
                .into_response();
        }
    }

    next.run(req).await
}
