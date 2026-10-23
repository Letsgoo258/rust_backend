use async_trait::async_trait;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::{SpanExporter, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{trace::SdkTracerProvider, Resource};
use sqlx::PgPool;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use uuid::Uuid;

/// Unified Telemetry Payload for all adapters and data sources
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TelemetryPayload {
    pub school_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub session_id: String,
    pub event_type: String,
    pub severity: String, // "INFO", "WARN", "ERROR", "CRITICAL"
    pub url: Option<String>,
    pub component: Option<String>,
    pub error_message: Option<String>,
    pub error_stack: Option<String>,
    pub duration_ms: Option<i32>,
    pub metadata: Option<serde_json::Value>,
    pub timestamp: String,
}

/// Extensible Telemetry Adapter Trait to push data to various destinations
#[async_trait]
pub trait TelemetryAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    async fn send(&self, payload: &TelemetryPayload) -> Result<(), String>;
}

// ---------------------------------------------------------------------------
// Concrete Adapters
// ---------------------------------------------------------------------------

/// 1. File Adapter: Appends JSON lines to a local file buffer
pub struct FileTelemetryAdapter {
    pub file_path: String,
}

impl FileTelemetryAdapter {
    pub fn new(file_path: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
        }
    }
}

#[async_trait]
impl TelemetryAdapter for FileTelemetryAdapter {
    fn name(&self) -> &'static str {
        "file_jsonl"
    }

    async fn send(&self, payload: &TelemetryPayload) -> Result<(), String> {
        let json = serde_json::to_string(payload).map_err(|e| e.to_string())?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .map_err(|e| e.to_string())?;
        writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// 2. PostgreSQL Adapter: Directly persists telemetry into the database
#[allow(dead_code)]
pub struct PostgresTelemetryAdapter {
    pool: PgPool,
}

#[allow(dead_code)]
impl PostgresTelemetryAdapter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TelemetryAdapter for PostgresTelemetryAdapter {
    fn name(&self) -> &'static str {
        "postgres_db"
    }

    async fn send(&self, payload: &TelemetryPayload) -> Result<(), String> {
        let _ = sqlx::query(
            r#"
            INSERT INTO frontend_telemetry
            (school_id, user_id, session_id, event_type, url, component, error_message, error_stack, duration_ms, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#
        )
        .bind(payload.school_id)
        .bind(payload.user_id)
        .bind(&payload.session_id)
        .bind(&payload.event_type)
        .bind(&payload.url)
        .bind(&payload.component)
        .bind(&payload.error_message)
        .bind(&payload.error_stack)
        .bind(payload.duration_ms)
        .bind(&payload.metadata)
        .execute(&self.pool)
        .await;

        Ok(())
    }
}

/// 3. HTTP Webhook Adapter: Dispatches JSON payloads to remote endpoints (Slack, Datadog, Sentry, external ingestors)
pub struct HttpWebhookTelemetryAdapter {
    endpoint_url: String,
    auth_header: Option<String>,
    client: reqwest::Client,
}

impl HttpWebhookTelemetryAdapter {
    pub fn new(endpoint_url: impl Into<String>, auth_header: Option<String>) -> Self {
        Self {
            endpoint_url: endpoint_url.into(),
            auth_header,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait]
impl TelemetryAdapter for HttpWebhookTelemetryAdapter {
    fn name(&self) -> &'static str {
        "http_webhook"
    }

    async fn send(&self, payload: &TelemetryPayload) -> Result<(), String> {
        let mut req = self.client.post(&self.endpoint_url).json(payload);
        if let Some(ref auth) = self.auth_header {
            req = req.header("Authorization", auth);
        }
        let res = req.send().await.map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            return Err(format!("Webhook returned HTTP {}", res.status()));
        }
        Ok(())
    }
}

/// 4. Tracing / OpenTelemetry Adapter: Emits structured logs and span attributes
pub struct TracingTelemetryAdapter;

#[async_trait]
impl TelemetryAdapter for TracingTelemetryAdapter {
    fn name(&self) -> &'static str {
        "tracing_otel"
    }

    async fn send(&self, payload: &TelemetryPayload) -> Result<(), String> {
        match payload.severity.to_uppercase().as_str() {
            "CRITICAL" => {
                tracing::error!(
                    target: "eravaya::security_telemetry",
                    severity = "CRITICAL",
                    event = %payload.event_type,
                    school_id = ?payload.school_id,
                    user_id = ?payload.user_id,
                    component = ?payload.component,
                    url = ?payload.url,
                    error = ?payload.error_message,
                    metadata = ?payload.metadata,
                    "CRITICAL TELEMETRY: [{}] {:?} on {:?}",
                    payload.event_type,
                    payload.error_message,
                    payload.component
                );
            }
            "ERROR" => {
                tracing::error!(
                    target: "eravaya::telemetry",
                    event = %payload.event_type,
                    component = ?payload.component,
                    error = ?payload.error_message,
                    "Telemetry error: [{}]",
                    payload.event_type
                );
            }
            "WARN" => {
                tracing::warn!(
                    target: "eravaya::telemetry",
                    event = %payload.event_type,
                    component = ?payload.component,
                    "Telemetry warning: [{}]",
                    payload.event_type
                );
            }
            _ => {
                tracing::info!(
                    target: "eravaya::telemetry",
                    event = %payload.event_type,
                    component = ?payload.component,
                    "Telemetry event: [{}]",
                    payload.event_type
                );
            }
        }
        Ok(())
    }
}

/// 5. Notification Alert Adapter: Enqueues priority alerts for critical events into PostgreSQL notifications table
pub struct NotificationAlertAdapter {
    pool: PgPool,
}

impl NotificationAlertAdapter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TelemetryAdapter for NotificationAlertAdapter {
    fn name(&self) -> &'static str {
        "notification_alerts"
    }

    async fn send(&self, payload: &TelemetryPayload) -> Result<(), String> {
        if payload.severity.to_uppercase() == "CRITICAL" || payload.event_type.contains("SECURITY")
        {
            if let Some(uid) = payload.user_id {
                let notif_msg = format!(
                    "CRITICAL SECURITY ALERT: [{}] on component '{}': {}",
                    payload.event_type,
                    payload.component.as_deref().unwrap_or("unknown"),
                    payload
                        .error_message
                        .as_deref()
                        .unwrap_or("Unauthorized action detected")
                );

                let _ = sqlx::query(
                    r#"
                    INSERT INTO notifications (user_id, message, notification_type, status, priority)
                    VALUES ($1, $2, 'EMAIL'::notification_type, 'PENDING'::notification_status, 1)
                    "#
                )
                .bind(uid)
                .bind(&notif_msg)
                .execute(&self.pool)
                .await;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Telemetry Dispatcher (Multiplexer)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct TelemetryDispatcher {
    adapters: Arc<Vec<Arc<dyn TelemetryAdapter>>>,
}

#[allow(dead_code)]
impl TelemetryDispatcher {
    pub fn new() -> Self {
        Self {
            adapters: Arc::new(Vec::new()),
        }
    }

    /// Automatically constructs dispatcher with active adapters based on environment & DB pool
    pub fn with_default_adapters(pool: Option<PgPool>) -> Self {
        let mut list: Vec<Arc<dyn TelemetryAdapter>> = Vec::new();

        // 1. Always attach Tracing / OTel adapter
        list.push(Arc::new(TracingTelemetryAdapter));

        // 2. Attach File adapter (configured via TELEMETRY_FILE_PATH or default buffer)
        let file_path = std::env::var("TELEMETRY_FILE_PATH")
            .unwrap_or_else(|_| "telemetry_buffer.jsonl".into());
        list.push(Arc::new(FileTelemetryAdapter::new(file_path)));

        // 3. Attach Database & Notification Alert adapters if pool is supplied
        if let Some(p) = pool {
            list.push(Arc::new(NotificationAlertAdapter::new(p)));
        }

        // 4. Attach Webhook adapter if TELEMETRY_WEBHOOK_URL is set
        if let Ok(webhook_url) = std::env::var("TELEMETRY_WEBHOOK_URL") {
            if !webhook_url.is_empty() {
                let auth = std::env::var("TELEMETRY_WEBHOOK_AUTH").ok();
                list.push(Arc::new(HttpWebhookTelemetryAdapter::new(
                    webhook_url,
                    auth,
                )));
            }
        }

        Self {
            adapters: Arc::new(list),
        }
    }

    pub fn add_adapter(&mut self, adapter: Arc<dyn TelemetryAdapter>) {
        let mut new_list = (*self.adapters).clone();
        new_list.push(adapter);
        self.adapters = Arc::new(new_list);
    }

    /// Dispatches telemetry payload to all registered adapters asynchronously without blocking
    pub async fn dispatch(&self, payload: TelemetryPayload) {
        let adapters = Arc::clone(&self.adapters);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                for adapter in adapters.iter() {
                    if let Err(e) = adapter.send(&payload).await {
                        tracing::warn!("Telemetry adapter '{}' error: {}", adapter.name(), e);
                    }
                }
            });
        } else {
            for adapter in adapters.iter() {
                if let Err(e) = adapter.send(&payload).await {
                    tracing::warn!("Telemetry adapter '{}' error: {}", adapter.name(), e);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// OpenTelemetry & Tracing Subsystem Initializer
// ---------------------------------------------------------------------------

pub fn init_telemetry() -> Result<Option<SdkTracerProvider>, Box<dyn std::error::Error>> {
    let has_otel_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok();

    let (otel_layer, provider_opt) = if has_otel_endpoint {
        let mut endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").unwrap();
        if !endpoint.ends_with("/v1/traces") {
            endpoint = format!("{}/v1/traces", endpoint.trim_end_matches('/'));
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

        if let Ok(exporter) = SpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .with_headers(headers)
            .build()
        {
            let batch_processor =
                opentelemetry_sdk::trace::BatchSpanProcessor::builder(exporter).build();
            let provider = SdkTracerProvider::builder()
                .with_span_processor(batch_processor)
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
            (
                Some(tracing_opentelemetry::layer().with_tracer(tracer)),
                Some(provider),
            )
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let console_layer = tracing_subscriber::fmt::layer().with_thread_ids(true);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,eravaya=debug,tower_http=info,opentelemetry=off,opentelemetry_sdk=off,opentelemetry_otlp=off,reqwest=off,hyper=off,h2=off".into());

    tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .with(otel_layer)
        .try_init()?;

    Ok(provider_opt)
}

/// Dispatches a critical security alert across all configured Telemetry Adapters, Audit Logs, and Notification alerts
pub async fn dispatch_security_alert(
    pool: &sqlx::PgPool,
    school_id: uuid::Uuid,
    user_id: uuid::Uuid,
    action: &str,
    component: &str,
    error_message: &str,
    user_roles: &[String],
    ip_address: Option<String>,
) {
    let payload = TelemetryPayload {
        school_id: Some(school_id),
        user_id: Some(user_id),
        session_id: format!("security-alert-{}", Uuid::new_v4()),
        event_type: "CRITICAL_SECURITY_ALERT".into(),
        severity: "CRITICAL".into(),
        url: None,
        component: Some(component.to_string()),
        error_message: Some(error_message.to_string()),
        error_stack: None,
        duration_ms: None,
        metadata: Some(serde_json::json!({
            "severity": "CRITICAL",
            "alert_category": "UNAUTHORIZED_PRIVILEGE_ATTEMPT",
            "action": action,
            "user_roles": user_roles,
            "ip_address": ip_address,
            "timestamp": time::OffsetDateTime::now_utc().to_string()
        })),
        timestamp: time::OffsetDateTime::now_utc().to_string(),
    };

    let dispatcher = TelemetryDispatcher::with_default_adapters(Some(pool.clone()));
    dispatcher.dispatch(payload).await;

    // Also record in Audit subsystem
    let _ = crate::utils::audit::log_security_event(
        Some(school_id),
        Some(user_id),
        action,
        Some(component),
        Some(serde_json::json!({
            "severity": "CRITICAL",
            "user_roles": user_roles,
            "error_message": error_message,
        })),
        ip_address,
    )
    .await;
}
