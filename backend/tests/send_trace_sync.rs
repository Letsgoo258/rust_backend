use opentelemetry::{trace::TracerProvider, KeyValue};
use opentelemetry_otlp::{SpanExporter, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{
    trace::{BatchSpanProcessor, SdkTracerProvider},
    Resource,
};
use std::collections::HashMap;

#[tokio::test]
async fn send_trace_sync() {
    let _ = dotenvy::from_filename(".env");

    let endpoint_var = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();
    if endpoint_var.is_none() || endpoint_var.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
        println!("Skipping send_trace_sync: OTEL_EXPORTER_OTLP_ENDPOINT not configured.");
        return;
    }

    let mut endpoint = endpoint_var.unwrap();
    if !endpoint.ends_with("/v1/traces") {
        endpoint.push_str("/v1/traces");
    }

    let mut headers = HashMap::new();
    if let Ok(h) = std::env::var("OTEL_EXPORTER_OTLP_HEADERS") {
        for part in h.split(',') {
            let mut kv = part.splitn(2, '=');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                headers.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }

    let exporter = SpanExporter::builder()
        .with_http()
        .with_endpoint(endpoint)
        .with_headers(headers)
        .with_timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap();

    let processor = BatchSpanProcessor::builder(exporter).build();

    let provider = SdkTracerProvider::builder()
        .with_span_processor(processor)
        .with_resource(
            Resource::builder_empty()
                .with_attributes(vec![KeyValue::new("service.name", "eravaya-backend")])
                .build(),
        )
        .build();

    let tracer = provider.tracer("startup-test");
    use opentelemetry::trace::Tracer;
    tracer.in_span("grafana-test-span", |_cx| {
        // dummy span
    });

    let _ = provider.shutdown();
    println!("Tracer shutdown complete.");
}
