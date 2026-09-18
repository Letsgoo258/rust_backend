use std::env;

#[tokio::test]
async fn test_grafana_otlp_connectivity() {
    let _ = dotenvy::from_filename(".env");

    let mut endpoint = env::var("OTEL_EXPORTER_OTLP_ENDPOINT").unwrap_or_default();
    let auth_headers = env::var("OTEL_EXPORTER_OTLP_HEADERS").unwrap_or_default();

    if endpoint.is_empty() {
        println!("Skipping live OTLP test: OTEL_EXPORTER_OTLP_ENDPOINT is not set in .env");
        return;
    }

    // The Rust OpenTelemetry HTTP SDK automatically appends /v1/traces to the endpoint.
    // Our raw reqwest test needs to do the same to accurately test connectivity.
    if !endpoint.ends_with("/v1/traces") {
        endpoint = format!("{}/v1/traces", endpoint.trim_end_matches('/'));
    }

    println!(
        "Testing connectivity to Grafana OTLP Endpoint: {}",
        endpoint
    );

    let client = reqwest::Client::new();

    let mut auth_val = String::new();
    if auth_headers.starts_with("Authorization=") {
        auth_val = auth_headers.replace("Authorization=", "");
    }

    let response = client
        .post(&endpoint)
        .header("Authorization", auth_val)
        .send()
        .await;

    match response {
        Ok(res) => {
            let status = res.status();
            println!("Grafana OTLP Receiver responded with status: {}", status);
            assert!(
                status != reqwest::StatusCode::UNAUTHORIZED,
                "Grafana authentication failed! Check your OTEL_EXPORTER_OTLP_HEADERS token."
            );
            assert!(
                status != reqwest::StatusCode::NOT_FOUND,
                "Grafana OTLP endpoint not found (404). Ensure the URL is correct."
            );
        }
        Err(e) => {
            panic!(
                "FATAL: Could not connect to Grafana Cloud. Network error: {:?}",
                e
            );
        }
    }
}
