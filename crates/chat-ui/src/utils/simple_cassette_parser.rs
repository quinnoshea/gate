use crate::types::{ChatMessage, ChatResponse, Provider};
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct CassetteFile {
    interactions: Vec<Interaction>,
}

#[derive(Debug, Deserialize)]
struct Interaction {
    request: Request,
    response: Response,
}

#[derive(Debug, Deserialize)]
struct Request {
    body: String,
    uri: String,
}

#[derive(Debug, Deserialize)]
struct Response {
    body: ResponseBody,
}

#[derive(Debug, Deserialize)]
struct ResponseBody {
    string: String,
}

/// Parse a cassette file into a ChatResponse
pub fn parse_cassette(json_content: &str) -> Result<ChatResponse> {
    let cassette: CassetteFile =
        serde_json::from_str(json_content).context("Failed to parse cassette JSON")?;

    if cassette.interactions.is_empty() {
        anyhow::bail!("No interactions found in cassette");
    }

    let interaction = &cassette.interactions[0];
    let provider = Provider::from_uri(&interaction.request.uri);

    // Parse request to get the messages
    let request_body: Value =
        serde_json::from_str(&interaction.request.body).context("Failed to parse request body")?;

    let mut messages = extract_request_messages(&request_body, &provider)?;

    // Parse response and determine if it's streaming
    let is_streaming = interaction.response.body.string.starts_with("data:")
        || interaction.response.body.string.starts_with("event:");

    // Parse response and add assistant message
    if is_streaming {
        parse_streaming_response(&interaction.response.body.string, &mut messages)?;
    } else {
        parse_json_response(&interaction.response.body.string, &mut messages, &provider)?;
    }

    // Extract model and id from response if available
    let (id, model) = if !is_streaming {
        if let Ok(resp) = serde_json::from_str::<Value>(&interaction.response.body.string) {
            (
                resp.get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                resp.get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
            )
        } else {
            ("unknown".to_string(), "unknown".to_string())
        }
    } else {
        // For streaming, extract from first data chunk
        extract_streaming_metadata(&interaction.response.body.string)
            .unwrap_or(("unknown".to_string(), "unknown".to_string()))
    };

    Ok(ChatResponse {
        id,
        model,
        messages,
        provider,
        usage: None,
        metadata: Default::default(),
    })
}

/// Extract messages from request body
fn extract_request_messages(
    request_body: &Value,
    _provider: &Provider,
) -> Result<Vec<ChatMessage>> {
    let mut messages = Vec::new();

    // Handle different request formats
    if let Some(msgs_array) = request_body.get("messages").and_then(|v| v.as_array()) {
        // Standard messages array format (OpenAI, Anthropic)
        for msg in msgs_array {
            let role = msg
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("user")
                .to_string();

            let content = msg.get("content").cloned();

            let mut metadata = HashMap::new();

            // Store other fields in metadata
            if let Some(name) = msg.get("name") {
                metadata.insert("name".to_string(), name.clone());
            }
            if let Some(tool_calls) = msg.get("tool_calls") {
                metadata.insert("tool_calls".to_string(), tool_calls.clone());
            }

            messages.push(ChatMessage {
                role,
                content,
                tool_calls: None,
                name: None,
                metadata,
            });
        }
    } else if let Some(input) = request_body.get("input").and_then(|v| v.as_str()) {
        // OpenAI Responses API format
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: Some(Value::String(input.to_string())),
            tool_calls: None,
            name: None,
            metadata: Default::default(),
        });
    } else if let Some(contents) = request_body.get("contents").and_then(|v| v.as_array()) {
        // Google format
        for content in contents {
            let role = content
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("user")
                .to_string();

            if let Some(parts) = content.get("parts").and_then(|v| v.as_array()) {
                for part in parts {
                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                        messages.push(ChatMessage {
                            role: role.clone(),
                            content: Some(Value::String(text.to_string())),
                            tool_calls: None,
                            name: None,
                            metadata: Default::default(),
                        });
                    }
                }
            }
        }
    }

    Ok(messages)
}

/// Parse streaming response (SSE format)
fn parse_streaming_response(response_str: &str, messages: &mut Vec<ChatMessage>) -> Result<()> {
    let mut combined_content = String::new();
    let mut role = "assistant".to_string();

    for line in response_str.lines() {
        if let Some(data_str) = line.strip_prefix("data: ") {
            if data_str == "[DONE]" {
                break;
            }

            if let Ok(chunk) = serde_json::from_str::<Value>(data_str) {
                // Extract role if present
                if let Some(choices) = chunk.get("choices").and_then(|v| v.as_array())
                    && let Some(choice) = choices.first()
                    && let Some(delta) = choice.get("delta")
                {
                    if let Some(r) = delta.get("role").and_then(|v| v.as_str()) {
                        role = r.to_string();
                    }
                    if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                        combined_content.push_str(content);
                    }
                }

                // Handle Anthropic streaming format
                if chunk.get("type").and_then(|v| v.as_str()) == Some("content_block_delta")
                    && let Some(text) = chunk
                        .get("delta")
                        .and_then(|d| d.get("text"))
                        .and_then(|t| t.as_str())
                {
                    combined_content.push_str(text);
                }
            }
        }
    }

    if !combined_content.is_empty() {
        let mut metadata = HashMap::new();
        metadata.insert("is_streaming".to_string(), json!(true));

        messages.push(ChatMessage {
            role,
            content: Some(Value::String(combined_content)),
            tool_calls: None,
            name: None,
            metadata,
        });
    }

    Ok(())
}

/// Parse JSON response
fn parse_json_response(
    response_str: &str,
    messages: &mut Vec<ChatMessage>,
    provider: &Provider,
) -> Result<()> {
    let response: Value =
        serde_json::from_str(response_str).context("Failed to parse response JSON")?;

    match provider {
        Provider::OpenAI => {
            // Handle both Chat Completions and Responses API
            if let Some(choices) = response.get("choices").and_then(|v| v.as_array()) {
                // Chat Completions API
                for choice in choices {
                    if let Some(message) = choice.get("message") {
                        parse_message_object(message, messages)?;
                    }
                }
            } else if let Some(outputs) = response.get("output").and_then(|v| v.as_array()) {
                // Responses API
                for output in outputs {
                    parse_message_object(output, messages)?;
                }
            }
        }
        Provider::Anthropic => {
            // Anthropic messages API
            if let Some(content_array) = response.get("content").and_then(|v| v.as_array()) {
                let mut text_parts = Vec::new();
                let mut metadata = HashMap::new();

                for content in content_array {
                    if let Some(content_type) = content.get("type").and_then(|v| v.as_str()) {
                        match content_type {
                            "text" => {
                                if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
                                    text_parts.push(text);
                                }
                            }
                            "tool_use" => {
                                // Store tool use in metadata
                                metadata.insert("tool_use".to_string(), content.clone());
                            }
                            _ => {}
                        }
                    }
                }

                if !text_parts.is_empty() || !metadata.is_empty() {
                    messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: Some(Value::String(text_parts.join(""))),
                        tool_calls: None,
                        name: None,
                        metadata,
                    });
                }
            }
        }
        Provider::Google => {
            // Google Gemini API
            if let Some(candidates) = response.get("candidates").and_then(|v| v.as_array()) {
                for candidate in candidates {
                    if let Some(content) = candidate.get("content")
                        && let Some(parts) = content.get("parts").and_then(|v| v.as_array())
                    {
                        for part in parts {
                            if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                                messages.push(ChatMessage {
                                    role: "assistant".to_string(),
                                    content: Some(Value::String(text.to_string())),
                                    tool_calls: None,
                                    name: None,
                                    metadata: Default::default(),
                                });
                            }
                        }
                    }
                }
            }
        }
        _ => {
            // Unknown provider - try to extract a reasonable response
            if let Some(text) = response.get("text").and_then(|v| v.as_str()) {
                messages.push(ChatMessage::assistant(text));
            }
        }
    }

    Ok(())
}

/// Parse a message object into a ChatMessage
fn parse_message_object(message: &Value, messages: &mut Vec<ChatMessage>) -> Result<()> {
    let role = message
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("assistant")
        .to_string();

    let content = message.get("content").cloned();

    let mut metadata = HashMap::new();

    // Store additional fields in metadata
    if let Some(refusal) = message.get("refusal") {
        metadata.insert("refusal".to_string(), refusal.clone());
    }
    if let Some(finish_reason) = message.get("finish_reason") {
        metadata.insert("finish_reason".to_string(), finish_reason.clone());
    }
    if let Some(tool_calls) = message.get("tool_calls") {
        metadata.insert("tool_calls".to_string(), tool_calls.clone());
    }

    messages.push(ChatMessage {
        role,
        content,
        tool_calls: None,
        name: None,
        metadata,
    });

    Ok(())
}

/// Extract metadata from streaming response
fn extract_streaming_metadata(response_str: &str) -> Option<(String, String)> {
    for line in response_str.lines() {
        if let Some(data_str) = line.strip_prefix("data: ")
            && data_str != "[DONE]"
            && let Ok(chunk) = serde_json::from_str::<Value>(data_str)
        {
            let id = chunk
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let model = chunk
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            return Some((id, model));
        }
    }
    None
}
