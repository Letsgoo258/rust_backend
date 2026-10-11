use std::time::Duration;
use tower_http::trace::TraceLayer;
use tracing::Span;

fn main() {
    let _trace = TraceLayer::new_for_http().on_response(
        |response: &axum::http::Response<axum::body::Body>, latency: Duration, _span: &Span| {
            if response.status().is_server_error() || response.status().is_client_error() {
                tracing::error!(
                    latency_ms = latency.as_millis(),
                    status = response.status().as_u16(),
                    otel.status_code = "ERROR",
                    "HTTP Request failed"
                );
            } else {
                tracing::info!(
                    latency_ms = latency.as_millis(),
                    status = response.status().as_u16(),
                    "HTTP Request succeeded"
                );
            }
        },
    );
}
