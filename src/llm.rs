use std::pin::Pin;

use futures::stream::{Stream, StreamExt};

use crate::config::LlmConfig;

/// A message in the conversation history for multi-turn LLM calls.
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

/// Build Anthropic request headers.
fn anthropic_headers(config: &LlmConfig) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Ok(v) = config.api_key.parse() {
        headers.insert("x-api-key", v);
    }
    headers.insert("anthropic-version", "2023-06-01".parse().unwrap());
    headers.insert("content-type", "application/json".parse().unwrap());
    headers
}

/// Parse the LLM response JSON to extract the text content.
fn extract_response_text(config: &LlmConfig, json: &serde_json::Value) -> String {
    match config.provider.as_str() {
        "anthropic" => json["content"][0]["text"]
            .as_str()
            .unwrap_or("I couldn't generate a response.")
            .to_string(),
        _ => json["message"]["content"]
            .as_str()
            .unwrap_or("I couldn't generate a response.")
            .to_string(),
    }
}

/// Call the configured LLM provider (Anthropic or Ollama) and return the response text.
/// This is a shared function used by background jobs (anomaly root cause analysis).
pub async fn call_llm(
    config: &LlmConfig,
    system_prompt: &str,
    user_message: &str,
) -> anyhow::Result<String> {
    call_llm_with_history(config, system_prompt, &[], user_message).await
}

/// Call the LLM with conversation history for multi-turn chat.
pub async fn call_llm_with_history(
    config: &LlmConfig,
    system_prompt: &str,
    history: &[LlmMessage],
    user_message: &str,
) -> anyhow::Result<String> {
    let client = reqwest::Client::new();

    let (url, request_body, headers) = match config.provider.as_str() {
        "anthropic" => {
            let mut messages: Vec<serde_json::Value> = history
                .iter()
                .map(|m| serde_json::json!({"role": m.role, "content": m.content}))
                .collect();
            messages.push(serde_json::json!({"role": "user", "content": user_message}));

            let body = serde_json::json!({
                "model": config.model,
                "max_tokens": 2048,
                "system": system_prompt,
                "messages": messages
            });
            (config.url.clone(), body, anthropic_headers(config))
        }
        _ => {
            let mut messages =
                vec![serde_json::json!({"role": "system", "content": system_prompt})];
            for m in history {
                messages.push(serde_json::json!({"role": m.role, "content": m.content}));
            }
            messages.push(serde_json::json!({"role": "user", "content": user_message}));

            let body = serde_json::json!({
                "model": config.model,
                "messages": messages,
                "stream": false,
                "options": { "num_ctx": 8192 }
            });
            (
                config.url.clone(),
                body,
                reqwest::header::HeaderMap::new(),
            )
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

    let response_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse LLM response: {e}"))?;

    Ok(extract_response_text(config, &response_json))
}

/// Call the Anthropic streaming API and return a stream of text chunks.
/// Only supports the "anthropic" provider. Falls back to error for others.
pub async fn call_llm_stream(
    config: &LlmConfig,
    system_prompt: &str,
    history: &[LlmMessage],
    user_message: &str,
) -> anyhow::Result<Pin<Box<dyn Stream<Item = Result<String, anyhow::Error>> + Send>>> {
    if config.provider != "anthropic" {
        anyhow::bail!("Streaming only supported for Anthropic provider");
    }

    let client = reqwest::Client::new();

    let mut messages: Vec<serde_json::Value> = history
        .iter()
        .map(|m| serde_json::json!({"role": m.role, "content": m.content}))
        .collect();
    messages.push(serde_json::json!({"role": "user", "content": user_message}));

    let body = serde_json::json!({
        "model": config.model,
        "max_tokens": 2048,
        "stream": true,
        "system": system_prompt,
        "messages": messages
    });

    let resp = client
        .post(&config.url)
        .headers(anthropic_headers(config))
        .json(&body)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("LLM service error: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("LLM service returned {status}: {text}");
    }

    let byte_stream = resp.bytes_stream();
    Ok(Box::pin(parse_anthropic_sse_stream(byte_stream)))
}

/// Parse Anthropic's SSE byte stream into text chunks.
///
/// Anthropic SSE format:
///   event: content_block_delta
///   data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}
///
///   event: message_stop
///   data: {"type":"message_stop"}
fn parse_anthropic_sse_stream(
    byte_stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
) -> impl Stream<Item = Result<String, anyhow::Error>> + Send {
    futures::stream::unfold(
        (Box::pin(byte_stream), String::new()),
        |(mut stream, mut buf)| async move {
            loop {
                // Try to extract a complete SSE event from buffer (delimited by double newline)
                if let Some(pos) = buf.find("\n\n") {
                    let event_block = buf[..pos].to_string();
                    buf = buf[pos + 2..].to_string();

                    let mut event_type = String::new();
                    let mut data = String::new();
                    for line in event_block.lines() {
                        if let Some(val) = line.strip_prefix("event: ") {
                            event_type = val.to_string();
                        } else if let Some(val) = line.strip_prefix("data: ") {
                            data = val.to_string();
                        }
                    }

                    if event_type == "content_block_delta" {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) {
                            if let Some(text) = parsed["delta"]["text"].as_str() {
                                return Some((Ok(text.to_string()), (stream, buf)));
                            }
                        }
                    }

                    if event_type == "message_stop" {
                        return None;
                    }

                    // Skip other event types (content_block_start, ping, message_start, etc.)
                    continue;
                }

                // Need more data from the byte stream
                match stream.next().await {
                    Some(Ok(bytes)) => {
                        buf.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    Some(Err(e)) => {
                        return Some((
                            Err(anyhow::anyhow!("Stream error: {e}")),
                            (stream, buf),
                        ));
                    }
                    None => return None,
                }
            }
        },
    )
}
