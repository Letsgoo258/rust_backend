use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiResponsePayload {
    pub reply: String,
    pub intent: Option<String>,
}

/// Connect to AI service if configured; otherwise use structured fallback.
pub async fn connect_to_ai(
    sender_jid: &str,
    query: &str,
    conversation_history: &[crate::modules::whatsapp::repository::WhatsAppInboundRow],
) -> String {
    let gemini_key = std::env::var("GEMINI_API_KEY")
        .or_else(|_| std::env::var("AI_API_KEY"))
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .ok();

    if let Some(api_key) = gemini_key {
        if !api_key.trim().is_empty() {
            match call_gemini_api(&api_key, query, conversation_history).await {
                Ok(ai_reply) => {
                    info!(sender = %sender_jid, "AI successfully generated response for WhatsApp");
                    return ai_reply;
                }
                Err(e) => {
                    warn!(
                        sender = %sender_jid,
                        error = %e,
                        "AI request failed or timed out, using structured fallback"
                    );
                }
            }
        }
    }

    // Robust Structured Fallback
    format_fallback_message(query)
}

/// Calls Gemini REST API expecting a structured JSON response.
pub async fn call_gemini_api(
    api_key: &str,
    query: &str,
    _history: &[crate::modules::whatsapp::repository::WhatsAppInboundRow],
) -> Result<String, anyhow::Error> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={}",
        api_key
    );

    let system_instruction = "You are ERAVAYA ERP's smart school WhatsApp assistant. \
Respond in concise, helpful plain text suitable for WhatsApp (you may use *bold* or _italics_). \
Always return your output as valid JSON with format: {\"reply\": \"<your response message>\", \"intent\": \"<query_category>\"}.";

    let body = serde_json::json!({
        "system_instruction": {
            "parts": [{ "text": system_instruction }]
        },
        "contents": [{
            "parts": [{ "text": query }]
        }],
        "generationConfig": {
            "response_mime_type": "application/json",
            "temperature": 0.4,
            "maxOutputTokens": 300
        }
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()?;

    let res = client.post(&url).json(&body).send().await?;
    if !res.status().is_success() {
        let status = res.status();
        let err_body = res.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("Gemini API HTTP {status}: {err_body}"));
    }

    let resp_json: serde_json::Value = res.json().await?;
    let text_candidate = resp_json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing text in Gemini response"))?;

    // Parse structured JSON returned by model
    if let Ok(parsed) = serde_json::from_str::<AiResponsePayload>(text_candidate) {
        if !parsed.reply.trim().is_empty() {
            return Ok(parsed.reply);
        }
    }

    // Direct text fallback if not matching JSON schema
    Ok(text_candidate.to_string())
}

/// Formats the graceful fallback message when AI is unavailable or no key is configured.
pub fn format_fallback_message(query: &str) -> String {
    let display_query = if query.len() > 60 {
        format!("{}...", &query[..57])
    } else {
        query.to_string()
    };

    format!(
        "🤖 *ERAVAYA ERP Assistant*\n\nThank you for reaching out! We received your query:\n_{}_\n\nOur automated system is processing your request. An administrator or staff member has also been notified.\n\nType *help* anytime to view the assistance menu.",
        display_query
    )
}
