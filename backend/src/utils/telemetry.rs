use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::{SpanExporter, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{trace::SdkTracerProvider, Resource};
use std::collections::HashMap;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_telemetry() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup global error handler to catch export failures

    // 2. Parse Environment Variables explicitly
    let mut endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4318/v1/traces".to_string());

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

    // 3. Setup OpenTelemetry OTLP Exporter explicitly
    let exporter = SpanExporter::builder()
        .with_http()
        .with_endpoint(endpoint)
        .with_headers(headers)
        .build()?;

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            Resource::builder_empty()
                .with_attributes(vec![
                    KeyValue::new("service.name", "eravaya-backend"),
                    KeyValue::new("service.version", "0.1.0"),
                ])
                .build(),
        )
        .build();

    global::set_tracer_provider(provider.clone());
    let tracer = global::tracer("eravaya-backend");

    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    // 4. Setup File & Console Layers
    let file_appender = tracing_appender::rolling::daily("logs", "eravaya.log");
    let (non_blocking_appender, guard) = tracing_appender::non_blocking(file_appender);

    // Leak the guard so the background thread stays alive for the lifetime of the program
    std::mem::forget(guard);

    let console_layer = tracing_subscriber::fmt::layer().with_thread_ids(true);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking_appender)
        .with_ansi(false)
        .with_thread_ids(true);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,eravaya=debug,tower_http=info".into());

    // 5. Combine all layers
    tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .with(file_layer)
        .with(otel_layer)
        .try_init()?;

    Ok(())
}
