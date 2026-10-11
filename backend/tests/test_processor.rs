use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::trace::BatchSpanProcessor;

fn main() {
    let exporter = SpanExporter::builder().with_http().build().unwrap();
    let _processor = BatchSpanProcessor::builder(exporter).build();
}
