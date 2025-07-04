use crate::types::{ChatMessage, ChatResponse, Provider};
use anyhow::Result;
use serde_json::Value;

/// Parse OpenAI API response
pub fn parse_response(response: Value, request: Value) -> Result<ChatResponse> {
    // Extract messages from request
    let messages = parse_request_messages(&request);

    // Check if this is an error response first
    if let Some(_error) = response.get("error") {
        return parse_error_response(response, messages, &request);
    }

    // Handle different OpenAI response formats
    if response.get("choices").is_some() {
        // Chat Completions API
        parse_chat_completion_response(response, messages)
    } else if response.get("output").is_some() {
        // Responses API (/v1/responses)
        parse_responses_api_response(response, messages)
    } else {
        // Unknown format - try generic parsing
        let mut chat_response: ChatResponse = match serde_json::from_value(response.clone()) {
            Ok(resp) => resp,
            Err(_) => {
                // If parsing fails, create a basic response with the raw data
                ChatResponse {
                    id: response
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    model: response
                        .get("model")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    messages,
                    provider: Provider::OpenAI,
                    usage: None,
                    metadata: serde_json::from_value(response).unwrap_or_default(),
                }
            }
        };
        chat_response.provider = Provider::OpenAI;
        Ok(chat_response)
    }
}

fn parse_request_messages(request: &Value) -> Vec<ChatMessage> {
    let mut messages = Vec::new();

    // Try to extract messages from request
    if let Some(msgs) = request.get("messages").and_then(|m| m.as_array()) {
        for msg in msgs {
            // Parse each message more carefully to handle multipart content
            let role = msg
                .get("role")
                .and_then(|r| r.as_str())
                .unwrap_or("user")
                .to_string();

            let content = msg.get("content").cloned();
            let name = msg
                .get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());

            messages.push(ChatMessage {
                role,
                content,
                tool_calls: None,
                name,
                metadata: Default::default(),
            });
        }
    } else if let Some(input) = request.get("input").and_then(|i| i.as_str()) {
        // Responses API format
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: Some(Value::String(input.to_string())),
            tool_calls: None,
            name: None,
            metadata: Default::default(),
        });
    }

    messages
}

fn parse_chat_completion_response(
    response: Value,
    mut messages: Vec<ChatMessage>,
) -> Result<ChatResponse> {
    // Add assistant messages from choices
    if let Some(choices) = response.get("choices").and_then(|c| c.as_array()) {
        for choice in choices {
            if let Some(message) = choice.get("message")
                && let Ok(msg) = serde_json::from_value::<ChatMessage>(message.clone())
            {
                messages.push(msg);
            }
        }
    }

    // Create the response with all fields captured
    let mut chat_response: ChatResponse = serde_json::from_value(response)?;
    chat_response.provider = Provider::OpenAI;
    chat_response.messages = messages;

    Ok(chat_response)
}

fn parse_responses_api_response(
    response: Value,
    mut messages: Vec<ChatMessage>,
) -> Result<ChatResponse> {
    // Extract output messages
    if let Some(outputs) = response.get("output").and_then(|o| o.as_array()) {
        for output in outputs {
            let role = output
                .get("role")
                .and_then(|r| r.as_str())
                .unwrap_or("assistant")
                .to_string();

            let content = output
                .get("content")
                .cloned()
                .or_else(|| Some(Value::Array(vec![])));

            messages.push(ChatMessage {
                role,
                content,
                tool_calls: None,
                name: None,
                metadata: Default::default(),
            });
        }
    }

    let mut chat_response: ChatResponse = serde_json::from_value(response)?;
    chat_response.provider = Provider::OpenAI;
    chat_response.messages = messages;

    Ok(chat_response)
}

fn parse_error_response(
    response: Value,
    mut messages: Vec<ChatMessage>,
    request: &Value,
) -> Result<ChatResponse> {
    // Add error as a system message
    if let Some(error) = response.get("error") {
        let error_message = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error")
            .to_string();

        messages.push(ChatMessage {
            role: "system".to_string(),
            content: Some(Value::String(error_message)),
            tool_calls: None,
            name: None,
            metadata: Default::default(),
        });
    }

    // Extract model from request instead of hardcoding "error"
    let model = request
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(ChatResponse {
        id: "error".to_string(),
        model,
        messages,
        provider: Provider::OpenAI,
        usage: None,
        metadata: Default::default(),
    })
}
