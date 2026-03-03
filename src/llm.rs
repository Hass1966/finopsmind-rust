use crate::config::LlmConfig;

/// Call the configured LLM provider (Anthropic or Ollama) and return the response text.
/// This is a shared function used by both the chat handler and background jobs.
pub async fn call_llm(config: &LlmConfig, system_prompt: &str, user_message: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::new();

    let (url, request_body, headers) = match config.provider.as_str() {
        "anthropic" => {
            let body = serde_json::json!({
                "model": config.model,
                "max_tokens": 2048,
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
            headers.insert("content-type", "application/json".parse().unwrap());
            (config.url.clone(), body, headers)
        }
        _ => {
            // Ollama format (default)
            let body = serde_json::json!({
                "model": config.model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_message}
                ],
                "stream": false,
                "options": {
                    "num_ctx": 8192
                }
            });
            (config.url.clone(), body, reqwest::header::HeaderMap::new())
        }
    };

    let resp = client
        .post(&url)
        .headers(headers)
        .json(&request_body)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("LLM service error: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        tracing::error!("LLM API error: {} - {}", status, text);
        anyhow::bail!("LLM service returned {status}");
    }

    let response_json: serde_json::Value = resp.json().await
        .map_err(|e| anyhow::anyhow!("Failed to parse LLM response: {e}"))?;

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
