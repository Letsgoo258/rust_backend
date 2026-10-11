#[test]
fn test_tls() {
    dotenvy::dotenv().ok();
    let mut headers = reqwest::header::HeaderMap::new();
    if let Ok(h) = std::env::var("OTEL_EXPORTER_OTLP_HEADERS") {
        for part in h.split(',') {
            let mut kv = part.splitn(2, '=');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                headers.insert(
                    reqwest::header::HeaderName::from_bytes(k.trim().as_bytes()).unwrap(),
                    reqwest::header::HeaderValue::from_str(v.trim()).unwrap(),
                );
            }
        }
    } else {
        println!("Skipping test_tls: OTEL_EXPORTER_OTLP_HEADERS not configured.");
        return;
    }

    let client = reqwest::blocking::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();
    let res = client
        .post("https://otlp-gateway-prod-ap-south-1.grafana.net/otlp/v1/traces")
        .send();
    println!("Result: {:?}", res);
}
