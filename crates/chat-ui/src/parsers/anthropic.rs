use crate::types::{ChatMessage, ChatResponse, Provider, Usage};
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Parse Anthropic API response
pub fn parse_response(response: Value, request: Value) -> Result<ChatResponse> {
    // Extract messages from request
    let mut messages = parse_request_messages(&request);

    // Parse the response content
    if let Some(content_array) = response.get("content").and_then(|c| c.as_array()) {
        let mut combined_content = Vec::new();
        let mut tool_calls = Vec::new();

        for content in content_array {
            let content_type = content.get("type").and_then(|t| t.as_str()).unwrap_or("");

            match content_type {
                "text" => {
                    if let Some(text) = content.get("text") {
                        combined_content.push(json!({
                            "type": "text",
                            "text": text
                        }));
                    }
                }
                "tool_use" => {
                    // Anthropic's tool format
                    tool_calls.push(content.clone());
                }
                _ => {
                    // Other content types (images, etc.)
                    combined_content.push(content.clone());
                }
            }
        }

        // Create assistant message
        let mut assistant_msg = ChatMessage {
            role: "assistant".to_string(),
            content: if combined_content.is_empty() {
                None
            } else if combined_content.len() == 1
                && combined_content[0].get("type").and_then(|t| t.as_str()) == Some("text")
            {
                combined_content[0].get("text").cloned()
            } else {
                Some(Value::Array(combined_content))
            },
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            name: None,
            metadata: HashMap::new(),
        };

        // Add stop_reason as metadata
        if let Some(stop_reason) = response.get("stop_reason") {
            assistant_msg
                .metadata
                .insert("finish_reason".to_string(), stop_reason.clone());
        }

        messages.push(assistant_msg);
    }

    // Parse usage with field aliasing
    let usage = response.get("usage").map(|usage_obj| Usage {
        prompt_tokens: usage_obj
            .get("input_tokens")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
        completion_tokens: usage_obj
            .get("output_tokens")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
        total_tokens: None, // Anthropic doesn't provide total_tokens
        metadata: serde_json::from_value(usage_obj.clone()).unwrap_or_default(),
    });

    // Build response with all metadata captured
    let mut metadata: HashMap<String, Value> = serde_json::from_value(response.clone())?;
    // Remove fields we've explicitly handled
    metadata.remove("content");
    metadata.remove("usage");
    metadata.remove("id");
    metadata.remove("model");

    Ok(ChatResponse {
        id: response
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        model: response
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        messages,
        provider: Provider::Anthropic,
        usage,
        metadata,
    })
}

fn parse_request_messages(request: &Value) -> Vec<ChatMessage> {
    let mut messages = Vec::new();

    // Add system message if present
    if let Some(system) = request.get("system").and_then(|s| s.as_str()) {
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: Some(Value::String(system.to_string())),
            tool_calls: None,
            name: None,
            metadata: Default::default(),
        });
    }

    // Add user/assistant messages
    if let Some(msgs) = request.get("messages").and_then(|m| m.as_array()) {
        for msg in msgs {
            if let Ok(mut message) = serde_json::from_value::<ChatMessage>(msg.clone()) {
                // Anthropic uses "content" field which can be string or array
                if let Some(content) = msg.get("content") {
                    message.content = Some(content.clone());
                }
                messages.push(message);
            }
        }
    }

    messages
}
