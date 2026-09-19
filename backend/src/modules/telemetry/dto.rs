use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct TelemetryEventRequest {
    pub session_id: String,
    pub event_type: String,
    pub url: Option<String>,
    pub component: Option<String>,
    pub error_message: Option<String>,
    pub error_stack: Option<String>,
    pub duration_ms: Option<i32>,
    pub metadata: Option<serde_json::Value>,
}
