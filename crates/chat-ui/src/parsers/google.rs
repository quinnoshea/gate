use crate::types::{ChatMessage, ChatResponse, Provider, Usage};
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

/// Parse Google API response
pub fn parse_response(response: Value, request: Value) -> Result<ChatResponse> {
    // Extract messages from request
    let mut messages = parse_request_messages(&request);

    // Parse candidates (Google's equivalent of choices)
    if let Some(candidates) = response.get("candidates").and_then(|c| c.as_array()) {
        for candidate in candidates {
            if let Some(content) = candidate.get("content")
                && let Some(parts) = content.get("parts").and_then(|p| p.as_array())
            {
                let role = content
                    .get("role")
                    .and_then(|r| r.as_str())
                    .unwrap_or("model") // Google uses "model" instead of "assistant"
                    .to_string();

                // Extract function calls and regular content separately
                let mut text_parts = Vec::new();
                let mut function_calls = Vec::new();

                for part in parts {
                    if let Some(function_call) = part.get("functionCall") {
                        // Convert Google functionCall to OpenAI-style tool call
                        let tool_call =
                            convert_google_function_call(function_call, function_calls.len());
                        function_calls.push(tool_call);
                    } else if let Some(text) = part.get("text") {
                        text_parts.push(text.clone());
                    } else {
                        // For other content types, add to text parts as JSON
                        text_parts.push(part.clone());
                    }
                }

                // Build content from non-function parts
                let content_value = if text_parts.is_empty() {
                    None
                } else if text_parts.len() == 1 {
                    Some(text_parts[0].clone())
                } else {
                    Some(Value::Array(text_parts))
                };

                let mut message = ChatMessage {
                    role: if role == "model" {
                        "assistant".to_string()
                    } else {
                        role
                    },
                    content: content_value,
                    tool_calls: if function_calls.is_empty() {
                        None
                    } else {
                        Some(function_calls)
                    },
                    name: None,
                    metadata: HashMap::new(),
                };

                // Add finish reason as metadata
                if let Some(finish_reason) = candidate.get("finishReason") {
                    message
                        .metadata
                        .insert("finish_reason".to_string(), finish_reason.clone());
                }

                messages.push(message);
            }
        }
    }

    // Parse usage metadata
    let usage = response.get("usageMetadata").map(|usage_metadata| Usage {
        prompt_tokens: usage_metadata
            .get("promptTokenCount")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
        completion_tokens: usage_metadata
            .get("candidatesTokenCount")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
        total_tokens: usage_metadata
            .get("totalTokenCount")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
        metadata: serde_json::from_value(usage_metadata.clone()).unwrap_or_default(),
    });

    // Build response with all metadata
    let mut metadata: HashMap<String, Value> = serde_json::from_value(response.clone())?;
    metadata.remove("candidates");
    metadata.remove("usageMetadata");

    Ok(ChatResponse {
        id: response
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        model: response
            .get("modelVersion")
            .or_else(|| response.get("model"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        messages,
        provider: Provider::Google,
        usage,
        metadata,
    })
}

fn parse_request_messages(request: &Value) -> Vec<ChatMessage> {
    let mut messages = Vec::new();

    // Add system instruction if present
    if let Some(system_instruction) = request.get("systemInstruction")
        && let Some(parts) = system_instruction.get("parts").and_then(|p| p.as_array())
    {
        let content = if parts.len() == 1 && parts[0].get("text").is_some() {
            parts[0].get("text").cloned()
        } else {
            Some(Value::Array(parts.to_vec()))
        };

        messages.push(ChatMessage {
            role: "system".to_string(),
            content,
            tool_calls: None,
            name: None,
            metadata: Default::default(),
        });
    }

    // Add conversation contents
    if let Some(contents) = request.get("contents").and_then(|c| c.as_array()) {
        for content in contents {
            let role = content
                .get("role")
                .and_then(|r| r.as_str())
                .unwrap_or("user")
                .to_string();

            if let Some(parts) = content.get("parts").and_then(|p| p.as_array()) {
                let content_value = if parts.len() == 1 && parts[0].get("text").is_some() {
                    parts[0].get("text").cloned()
                } else {
                    Some(Value::Array(parts.to_vec()))
                };

                messages.push(ChatMessage {
                    role: if role == "model" {
                        "assistant".to_string()
                    } else {
                        role
                    },
                    content: content_value,
                    tool_calls: None,
                    name: None,
                    metadata: Default::default(),
                });
            }
        }
    }

    messages
}

/// Convert Google functionCall format to OpenAI tool call format
fn convert_google_function_call(function_call: &Value, index: usize) -> Value {
    let function_name = function_call
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("unknown");

    let empty_args = Value::Object(serde_json::Map::new());
    let args = function_call.get("args").unwrap_or(&empty_args);

    // Convert to OpenAI format
    serde_json::json!({
        "id": format!("call_{}", index),
        "type": "function",
        "function": {
            "name": function_name,
            "arguments": serde_json::to_string(args).unwrap_or_default()
        }
    })
}
