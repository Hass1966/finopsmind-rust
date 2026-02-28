use axum::{extract::State, Extension, Json};
use serde::{Deserialize, Serialize};

use crate::auth::Claims;
use crate::config::LlmConfig;
use crate::errors::AppError;
use crate::handlers::AppState;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub response: String,
    pub intent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

pub async fn chat(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(chat_req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, AppError> {
    // Build system prompt with FinOps context
    let system_prompt = format!(
        "You are a FinOps AI assistant for organization {}. \
         Help users understand their cloud costs, anomalies, budgets, and optimization recommendations. \
         Provide concise, actionable insights about cloud spending. \
         When asked about costs, try to give specific numbers and trends. \
         When asked about savings, reference specific recommendation types.",
        claims.org_id
    );

    let response = call_llm(&state.llm_config, &system_prompt, &chat_req.message).await?;

    let intent = detect_intent(&chat_req.message);

    Ok(Json(ChatResponse {
        response,
        intent,
        data: None,
    }))
}

fn detect_intent(message: &str) -> String {
    let msg = message.to_lowercase();
    if msg.contains("cost") || msg.contains("spend") || msg.contains("bill") {
        "cost_query".into()
    } else if msg.contains("anomal") || msg.contains("spike") || msg.contains("unusual") {
        "anomaly_query".into()
    } else if msg.contains("budget") {
        "budget_query".into()
    } else if msg.contains("sav") || msg.contains("optim") || msg.contains("recommend") {
        "savings_query".into()
    } else if msg.contains("forecast") || msg.contains("predict") {
        "forecast_query".into()
    } else {
        "general".into()
    }
}

async fn call_llm(config: &LlmConfig, system_prompt: &str, user_message: &str) -> Result<String, AppError> {
    let client = reqwest::Client::new();

    let (request_body, headers) = match config.provider.as_str() {
        "anthropic" => {
            let body = serde_json::json!({
                "model": config.model,
                "max_tokens": 1024,
                "system": system_prompt,
                "messages": [
                    {"role": "user", "content": user_message}
                ]
            });

            let mut headers = reqwest::header::HeaderMap::new();
            if let Ok(v) = config.api_key.parse() {
                headers.insert("x-api-key", v);
            }
            headers.insert("anthropic-version", "2023-06-01".parse().unwrap());
            (body, headers)
        }
        _ => {
            // Ollama format (default)
            let body = serde_json::json!({
                "model": config.model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_message}
                ],
                "stream": false
            });
            (body, reqwest::header::HeaderMap::new())
        }
    };

    let resp = client
        .post(&config.url)
        .headers(headers)
        .json(&request_body)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| AppError::service_unavailable(&format!("LLM service: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        tracing::error!("LLM API error: {} - {}", status, text);
        return Err(AppError::service_unavailable("LLM service returned an error"));
    }

    let response_json: serde_json::Value = resp.json().await
        .map_err(|e| AppError::internal(format!("Failed to parse LLM response: {e}")))?;

    // Extract response based on provider format
    let text = match config.provider.as_str() {
        "anthropic" => {
            response_json["content"][0]["text"]
                .as_str()
                .unwrap_or("I couldn't generate a response.")
                .to_string()
        }
        _ => {
            // Ollama format
            response_json["message"]["content"]
                .as_str()
                .unwrap_or("I couldn't generate a response.")
                .to_string()
        }
    };

    Ok(text)
}
