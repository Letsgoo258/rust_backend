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

    use std::pin::Pin;
    use std::task::{Context, Poll};

    struct EnterFuture<F> {
        handle: tokio::runtime::Handle,
        future: F,
    }

    impl<F: std::future::Future> std::future::Future for EnterFuture<F> {
        type Output = F::Output;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = unsafe { self.get_unchecked_mut() };
            let _guard = this.handle.enter();
            let pinned = unsafe { Pin::new_unchecked(&mut this.future) };
            pinned.poll(cx)
        }
    }

    #[derive(Debug)]
    struct TokioSpanExporter<E> {
        inner: E,
        handle: tokio::runtime::Handle,
    }

    impl<E: opentelemetry_sdk::trace::SpanExporter> opentelemetry_sdk::trace::SpanExporter
        for TokioSpanExporter<E>
    {
        fn export(
            &self,
            batch: Vec<opentelemetry_sdk::trace::SpanData>,
        ) -> impl std::future::Future<Output = opentelemetry_sdk::error::OTelSdkResult> + Send
        {
            EnterFuture {
                handle: self.handle.clone(),
                future: self.inner.export(batch),
            }
        }

        fn shutdown_with_timeout(
            &self,
            timeout: std::time::Duration,
        ) -> opentelemetry_sdk::error::OTelSdkResult {
            let _guard = self.handle.enter();
            self.inner.shutdown_with_timeout(timeout)
        }

        fn shutdown(&self) -> opentelemetry_sdk::error::OTelSdkResult {
            let _guard = self.handle.enter();
            self.inner.shutdown()
        }

        fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
            let _guard = self.handle.enter();
            self.inner.force_flush()
        }

        fn set_resource(&mut self, resource: &opentelemetry_sdk::Resource) {
            self.inner.set_resource(resource)
        }
    }

    let tokio_exporter = TokioSpanExporter {
        inner: exporter,
        handle: tokio::runtime::Handle::current(),
    };

    let processor = BatchSpanProcessor::builder(tokio_exporter).build();

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
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    println!("Tracer shutdown complete.");
}
