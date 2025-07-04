use crate::types::{ChatMessage, ChatResponse, Provider};
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Parse SSE streaming response data
pub fn parse_streaming_response(
    sse_data: &str,
    provider: Provider,
    request: Value,
) -> Result<ChatResponse> {
    match provider {
        Provider::OpenAI => parse_openai_streaming(sse_data, request),
        Provider::Anthropic => parse_anthropic_streaming(sse_data, request),
        Provider::Google => parse_google_streaming(sse_data, request),
        Provider::Unknown(_) => {
            // Generic streaming - just create a message with the content
            Ok(ChatResponse {
                id: "streaming".to_string(),
                model: "unknown".to_string(),
                messages: vec![ChatMessage {
                    role: "assistant".to_string(),
                    content: Some(Value::String(sse_data.to_string())),
                    tool_calls: None,
                    name: None,
                    metadata: Default::default(),
                }],
                provider,
                usage: None,
                metadata: Default::default(),
            })
        }
    }
}

fn parse_openai_streaming(sse_data: &str, request: Value) -> Result<ChatResponse> {
    let mut messages = extract_request_messages(&request);
    let mut combined_content = String::new();
    let mut model = String::new();
    let mut id = String::new();
    let mut metadata = HashMap::new();
    let mut tool_calls: Vec<Value> = Vec::new();

    // Parse each SSE chunk
    for line in sse_data.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            let data = data.trim();

            // Skip empty data lines
            if data.is_empty() {
                continue;
            }

            if data == "[DONE]" {
                break;
            }

            match serde_json::from_str::<Value>(data) {
                Ok(chunk) => {
                    // Extract model and id from first chunk
                    if model.is_empty() {
                        if let Some(m) = chunk.get("model").and_then(|v| v.as_str()) {
                            model = m.to_string();
                        }
                        if let Some(i) = chunk.get("id").and_then(|v| v.as_str()) {
                            id = i.to_string();
                        }

                        // Store system_fingerprint and service_tier if present
                        if let Some(sf) = chunk.get("system_fingerprint") {
                            metadata.insert("system_fingerprint".to_string(), sf.clone());
                        }
                        if let Some(st) = chunk.get("service_tier") {
                            metadata.insert("service_tier".to_string(), st.clone());
                        }
                    }

                    // Extract content from choices
                    if let Some(choices) = chunk.get("choices").and_then(|c| c.as_array()) {
                        for choice in choices {
                            if let Some(delta) = choice.get("delta") {
                                // Handle text content
                                if let Some(content) = delta.get("content").and_then(|c| c.as_str())
                                {
                                    combined_content.push_str(content);
                                }

                                // Handle tool calls
                                if let Some(tcs) =
                                    delta.get("tool_calls").and_then(|t| t.as_array())
                                {
                                    for tc in tcs {
                                        tool_calls.push(tc.clone());
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    // Log parsing error but continue processing
                    eprintln!("Warning: Failed to parse OpenAI SSE data '{data}': {e}");
                    continue;
                }
            }
        }
    }

    // Create assistant message
    let mut assistant_msg = ChatMessage {
        role: "assistant".to_string(),
        content: if combined_content.is_empty() {
            None
        } else {
            Some(Value::String(combined_content))
        },
        tool_calls: if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        },
        name: None,
        metadata: HashMap::new(),
    };

    // Mark as streaming if content is substantial
    if assistant_msg.content.is_some() {
        assistant_msg
            .metadata
            .insert("is_streaming".to_string(), json!(true));
    }

    messages.push(assistant_msg);

    Ok(ChatResponse {
        id,
        model,
        messages,
        provider: Provider::OpenAI,
        usage: None,
        metadata,
    })
}

fn parse_anthropic_streaming(sse_data: &str, request: Value) -> Result<ChatResponse> {
    let mut messages = extract_request_messages(&request);
    let mut combined_content = String::new();
    let mut model = String::new();
    let mut id = String::new();
    let mut metadata = HashMap::new();

    // Parse SSE events
    for line in sse_data.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            let data = data.trim();

            // Skip empty data lines
            if data.is_empty() {
                continue;
            }

            // Handle special termination markers
            if data == "[DONE]" {
                break;
            }

            match serde_json::from_str::<Value>(data) {
                Ok(event) => {
                    let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

                    match event_type {
                        "message_start" => {
                            if let Some(message) = event.get("message") {
                                if let Some(m) = message.get("model").and_then(|v| v.as_str()) {
                                    model = m.to_string();
                                }
                                if let Some(i) = message.get("id").and_then(|v| v.as_str()) {
                                    id = i.to_string();
                                }
                            }
                        }
                        "content_block_delta" => {
                            if let Some(delta) = event.get("delta")
                                && let Some(text) = delta.get("text").and_then(|t| t.as_str())
                            {
                                combined_content.push_str(text);
                            }
                        }
                        "message_delta" => {
                            if let Some(delta) = event.get("delta")
                                && let Some(stop_reason) = delta.get("stop_reason")
                            {
                                metadata.insert("stop_reason".to_string(), stop_reason.clone());
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    // Log parsing error but continue processing
                    eprintln!("Warning: Failed to parse SSE data '{data}': {e}");
                    continue;
                }
            }
        }
    }

    let mut assistant_msg = ChatMessage {
        role: "assistant".to_string(),
        content: if combined_content.is_empty() {
            None
        } else {
            Some(Value::String(combined_content))
        },
        tool_calls: None,
        name: None,
        metadata: HashMap::new(),
    };

    if assistant_msg.content.is_some() {
        assistant_msg
            .metadata
            .insert("is_streaming".to_string(), json!(true));
    }

    messages.push(assistant_msg);

    Ok(ChatResponse {
        id,
        model,
        messages,
        provider: Provider::Anthropic,
        usage: None,
        metadata,
    })
}

fn parse_google_streaming(_sse_data: &str, request: Value) -> Result<ChatResponse> {
    // Google uses a different streaming format (not SSE)
    // For now, treat it as a single response
    let messages = extract_request_messages(&request);

    Ok(ChatResponse {
        id: "streaming".to_string(),
        model: "google".to_string(),
        messages,
        provider: Provider::Google,
        usage: None,
        metadata: Default::default(),
    })
}

/// Extract messages from request regardless of provider format
fn extract_request_messages(request: &Value) -> Vec<ChatMessage> {
    let mut messages = Vec::new();

    // Common message extraction patterns
    if let Some(msgs) = request.get("messages").and_then(|m| m.as_array()) {
        for msg in msgs {
            if let Ok(message) = serde_json::from_value::<ChatMessage>(msg.clone()) {
                messages.push(message);
            }
        }
    }

    // System message (Anthropic style)
    if let Some(system) = request.get("system").and_then(|s| s.as_str()) {
        messages.insert(
            0,
            ChatMessage {
                role: "system".to_string(),
                content: Some(Value::String(system.to_string())),
                tool_calls: None,
                name: None,
                metadata: Default::default(),
            },
        );
    }

    // Google contents format
    if let Some(contents) = request.get("contents").and_then(|c| c.as_array()) {
        for content in contents {
            if let Ok(message) = serde_json::from_value::<ChatMessage>(content.clone()) {
                messages.push(message);
            }
        }
    }

    messages
}
