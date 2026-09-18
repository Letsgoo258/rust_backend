use opentelemetry::trace::TracerProvider as _;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::{trace::SdkTracerProvider, Resource};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_telemetry() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup OpenTelemetry OTLP Exporter
    let exporter = SpanExporter::builder().with_http().build()?;

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

    // 2. Setup File & Console Layers (Existing)
    let file_appender = tracing_appender::rolling::daily("logs", "eravaya.log");
    let (non_blocking_appender, _guard) = tracing_appender::non_blocking(file_appender);

    let console_layer = tracing_subscriber::fmt::layer().with_thread_ids(true);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking_appender)
        .with_ansi(false)
        .with_thread_ids(true);

    // 3. Combine all layers
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(console_layer)
        .with(file_layer)
        .with(otel_layer)
        .try_init()?;

    Ok(())
}
