use crate::types::{ChatMessage, ChatResponse, MessageRole, Provider, ToolCall, Usage};
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

// Helper macro for console logging that works in both WASM and non-WASM environments
macro_rules! console_warn {
    ($($arg:tt)*) => {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::warn_1(&format!($($arg)*).into());
        #[cfg(not(target_arch = "wasm32"))]
        eprintln!("WARN: {}", format!($($arg)*));
    };
}

macro_rules! console_error {
    ($($arg:tt)*) => {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::error_1(&format!($($arg)*).into());
        #[cfg(not(target_arch = "wasm32"))]
        eprintln!("ERROR: {}", format!($($arg)*));
    };
}

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

// OpenAI response structures
#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    id: String,
    model: String,
    choices: Vec<OpenAIChoice>,
    usage: Option<OpenAIUsage>,
    created: Option<i64>,
}

// OpenAI error response structure
#[derive(Debug, Deserialize)]
struct OpenAIErrorResponse {
    error: OpenAIError,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIError {
    message: String,
    #[serde(rename = "type")]
    error_type: String,
    param: Option<String>,
    code: Option<String>,
}

// OpenAI Responses API structure (/v1/responses)
#[derive(Debug, Deserialize)]
struct OpenAIResponsesResponse {
    id: String,
    model: String,
    output: Vec<OpenAIOutput>,
    usage: Option<OpenAIResponsesUsage>,
    created_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIOutput {
    id: String,
    #[serde(rename = "type")]
    output_type: String,
    content: Vec<OpenAIOutputContent>,
    role: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIOutputContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
    image_url: Option<serde_json::Value>, // For image content
}

#[derive(Debug, Deserialize)]
struct OpenAIResponsesUsage {
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    total_tokens: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<serde_json::Value>, // Can be string or array
    tool_calls: Option<Vec<OpenAIToolCall>>,
    refusal: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunction,
}

#[derive(Debug, Deserialize)]
struct OpenAIFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: Option<i32>,
    completion_tokens: Option<i32>,
    total_tokens: Option<i32>,
}

// Anthropic response structures
#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    id: String,
    model: String,
    content: Vec<AnthropicContent>,
    usage: Option<AnthropicUsage>,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
    // Tool use fields
    id: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
}

pub fn parse_cassette(json_content: &str) -> Result<ChatResponse> {
    let cassette: CassetteFile = serde_json::from_str(json_content)
        .map_err(|e| {
            console_error!("Failed to parse cassette JSON: {}", e);
            e
        })
        .context("Failed to parse cassette JSON")?;
    
    if cassette.interactions.is_empty() {
        anyhow::bail!("No interactions found in cassette");
    }
    
    let interaction = &cassette.interactions[0];
    let provider = detect_provider(&interaction.request.uri);
    
    // Parse request to get the messages
    let request_body: Value = serde_json::from_str(&interaction.request.body)
        .context("Failed to parse request body")?;
    
    let mut messages = parse_request_messages(&request_body, &provider)?;
    
    // Parse response based on provider
    let chat_response = match &provider {
        Provider::OpenAI => {
            parse_openai_response(&interaction.response.body.string, &mut messages)
                .map_err(|e| {
                    console_error!("Failed to parse OpenAI response: {}", e);
                    e
                })
        }
        Provider::Anthropic => {
            parse_anthropic_response(&interaction.response.body.string, &mut messages)
                .map_err(|e| {
                    console_error!("Failed to parse Anthropic response: {}", e);
                    e
                })
        }
        Provider::Google => {
            parse_google_response(&interaction.response.body.string, &mut messages)
                .map_err(|e| {
                    console_error!("Failed to parse Google response: {}", e);
                    e
                })
        }
        Provider::Unknown(uri) => {
            console_warn!("Unknown provider for URI: {}. Attempting generic parsing.", uri);
            // Create a basic response with the raw messages
            Ok(ChatResponse {
                id: "unknown".to_string(),
                model: "unknown".to_string(),
                messages: messages.clone(),
                usage: None,
                created: None,
                provider: provider.clone(),
            })
        }
    }?;

    // Don't automatically mark messages as streaming - only use actual streaming data from cassettes
    // This was causing all assistant messages to appear as "streaming" and disabling the input
    // apply_smart_streaming(&mut chat_response.messages);
    
    Ok(chat_response)
}


fn detect_provider(uri: &str) -> Provider {
    if uri.contains("openai.com") {
        Provider::OpenAI
    } else if uri.contains("anthropic.com") {
        Provider::Anthropic
    } else if uri.contains("google") || uri.contains("generativelanguage") {
        Provider::Google
    } else {
        Provider::Unknown(uri.to_string())
    }
}

fn parse_request_messages(request_body: &Value, provider: &Provider) -> Result<Vec<ChatMessage>> {
    let mut messages = Vec::new();
    
    match provider {
        Provider::OpenAI => {
            // Handle both Chat Completions API (messages) and Responses API (input)
            if let Some(msgs) = request_body["messages"].as_array() {
                // Chat Completions API format
                for msg in msgs {
                    let role = msg["role"].as_str().unwrap_or("user");
                    
                    // Handle different content formats
                    let message = if let Some(content_str) = msg["content"].as_str() {
                        // Simple string content
                        ChatMessage::new(parse_role(role), content_str.to_string())
                    } else if let Some(content_array) = msg["content"].as_array() {
                        // Array of content parts (multimodal)
                        let mut parts = Vec::new();
                        for part in content_array {
                            if let Some(part_type) = part["type"].as_str() {
                                match part_type {
                                    "text" => {
                                        if let Some(text) = part["text"].as_str() {
                                            parts.push(crate::types::MessagePart::Text {
                                                part_type: "text".to_string(),
                                                text: text.to_string(),
                                            });
                                        }
                                    }
                                    "image_url" => {
                                        if let Some(image_obj) = part["image_url"].as_object() {
                                            if let Some(url) = image_obj.get("url").and_then(|v| v.as_str()) {
                                                parts.push(crate::types::MessagePart::Image {
                                                    part_type: "image_url".to_string(),
                                                    image_url: crate::types::ImageContent {
                                                        url: url.to_string(),
                                                        detail: image_obj.get("detail").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                                    },
                                                });
                                            }
                                        }
                                    }
                                    unknown_type => {
                                        console_warn!("Unknown OpenAI content part type in request: {}", unknown_type);
                                    }
                                }
                            }
                        }
                        let msg = ChatMessage {
                            role: parse_role(role),
                            content: Some(crate::types::MessageContent::Parts(parts)),
                            name: None,
                            tool_calls: None,
                            tool_call_id: None,
                            refusal: None,
                            is_streaming: false,
                            finish_reason: None,
                            metadata: Default::default(),
                        };
                        msg
                    } else {
                        // Empty content
                        ChatMessage::new(parse_role(role), String::new())
                    };
                    
                    messages.push(message);
                }
            } else if let Some(input) = request_body["input"].as_str() {
                // Responses API format
                messages.push(ChatMessage::new(
                    MessageRole::User,
                    input.to_string(),
                ));
            }
        }
        Provider::Anthropic => {
            if let Some(msgs) = request_body["messages"].as_array() {
                for msg in msgs {
                    let role = msg["role"].as_str().unwrap_or("user");
                    
                    // Handle different content formats for Anthropic
                    let message = if let Some(content_str) = msg["content"].as_str() {
                        // Simple string content
                        ChatMessage::new(parse_role(role), content_str.to_string())
                    } else if let Some(content_array) = msg["content"].as_array() {
                        // Array of content parts (multimodal)
                        let mut parts = Vec::new();
                        for part in content_array {
                            if let Some(part_type) = part["type"].as_str() {
                                match part_type {
                                    "text" => {
                                        if let Some(text) = part["text"].as_str() {
                                            parts.push(crate::types::MessagePart::Text {
                                                part_type: "text".to_string(),
                                                text: text.to_string(),
                                            });
                                        }
                                    }
                                    "image" => {
                                        // Anthropic uses a different format for images
                                        if let Some(source) = part["source"].as_object() {
                                            if let Some(data) = source["data"].as_str() {
                                                let media_type = source["media_type"].as_str().unwrap_or("image/jpeg");
                                                let url = format!("data:{};base64,{}", media_type, data);
                                                parts.push(crate::types::MessagePart::Image {
                                                    part_type: "image".to_string(),
                                                    image_url: crate::types::ImageContent {
                                                        url,
                                                        detail: None,
                                                    },
                                                });
                                            }
                                        }
                                    }
                                    unknown_type => {
                                        console_warn!("Unknown Anthropic content part type in request: {}", unknown_type);
                                    }
                                }
                            }
                        }
                        let msg = ChatMessage {
                            role: parse_role(role),
                            content: Some(crate::types::MessageContent::Parts(parts)),
                            name: None,
                            tool_calls: None,
                            tool_call_id: None,
                            refusal: None,
                            is_streaming: false,
                            finish_reason: None,
                            metadata: Default::default(),
                        };
                        msg
                    } else {
                        // Empty content
                        ChatMessage::new(parse_role(role), String::new())
                    };
                    
                    messages.push(message);
                }
            }
        }
        Provider::Google => {
            if let Some(contents) = request_body["contents"].as_array() {
                for content in contents {
                    let role = content["role"].as_str().unwrap_or("user");
                    if let Some(parts) = content["parts"].as_array() {
                        for part in parts {
                            if let Some(text) = part["text"].as_str() {
                                messages.push(ChatMessage::new(
                                    parse_role(role),
                                    text.to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        }
        _ => {
            console_warn!("Unsupported provider in request parsing: {:?}", provider);
        }
    }
    
    Ok(messages)
}

fn parse_role(role: &str) -> MessageRole {
    match role.to_lowercase().as_str() {
        "system" => MessageRole::System,
        "user" => MessageRole::User,
        "assistant" | "model" => MessageRole::Assistant,
        "tool" | "function" => MessageRole::Tool,
        _ => MessageRole::User,
    }
}

fn parse_openai_response(response_str: &str, messages: &mut Vec<ChatMessage>) -> Result<ChatResponse> {
    // Handle streaming responses
    if response_str.starts_with("data:") {
        return parse_openai_streaming_response(response_str, messages);
    }
    
    // Check for error responses first
    if let Ok(error_response) = serde_json::from_str::<OpenAIErrorResponse>(response_str) {
        let error_message = format!("OpenAI Error ({}): {}", 
            error_response.error.error_type, 
            error_response.error.message
        );
        
        // Add error as a system message
        messages.push(ChatMessage::new(
            crate::types::MessageRole::System,
            error_message.clone(),
        ));
        
        return Ok(ChatResponse {
            id: "error".to_string(),
            model: "error".to_string(),
            messages: messages.clone(),
            usage: None,
            created: None,
            provider: crate::types::Provider::OpenAI,
        });
    }
    
    // Try parsing as Responses API first (has "output" field)
    if let Ok(responses_response) = serde_json::from_str::<OpenAIResponsesResponse>(response_str) {
        return parse_openai_responses_api(responses_response, messages);
    }
    
    // Fall back to Chat Completions API (has "choices" field)
    let response: OpenAIResponse = serde_json::from_str(response_str)
        .context("Failed to parse OpenAI response")?;
    
    for choice in response.choices {
        // Parse content which can be either a string or array of parts
        let message_content = if let Some(content_value) = &choice.message.content {
            match content_value {
                serde_json::Value::String(s) => Some(crate::types::MessageContent::Text(s.clone())),
                serde_json::Value::Array(parts) => {
                    let mut parsed_parts = Vec::new();
                    for part in parts {
                        if let Some(part_type) = part["type"].as_str() {
                            match part_type {
                                "text" => {
                                    if let Some(text) = part["text"].as_str() {
                                        parsed_parts.push(crate::types::MessagePart::Text {
                                            part_type: "text".to_string(),
                                            text: text.to_string(),
                                        });
                                    }
                                }
                                "image_url" => {
                                    if let Some(image_obj) = part["image_url"].as_object() {
                                        if let Some(url) = image_obj.get("url").and_then(|v| v.as_str()) {
                                            parsed_parts.push(crate::types::MessagePart::Image {
                                                part_type: "image_url".to_string(),
                                                image_url: crate::types::ImageContent {
                                                    url: url.to_string(),
                                                    detail: image_obj.get("detail").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                                },
                                            });
                                        }
                                    }
                                }
                                unknown_type => {
                                    console_warn!("Unknown OpenAI content part type in response: {}", unknown_type);
                                }
                            }
                        }
                    }
                    Some(crate::types::MessageContent::Parts(parsed_parts))
                }
                _ => None
            }
        } else {
            None
        };
        
        let mut message = ChatMessage {
            role: parse_role(&choice.message.role),
            content: message_content,
            name: None,
            tool_calls: None,
            tool_call_id: None,
            refusal: choice.message.refusal.clone(),
            is_streaming: false,
            finish_reason: choice.finish_reason.clone(),
            metadata: Default::default(),
        };
        
        if let Some(tool_calls) = choice.message.tool_calls {
            message.tool_calls = Some(tool_calls.into_iter().map(|tc| ToolCall {
                id: tc.id,
                tool_type: tc.tool_type,
                function: crate::types::FunctionCall {
                    name: tc.function.name,
                    arguments: tc.function.arguments,
                },
            }).collect());
        }
        
        message.refusal = choice.message.refusal;
        message.finish_reason = choice.finish_reason;
        
        // is_streaming is set to false by default
        
        messages.push(message);
    }
    
    Ok(ChatResponse {
        id: response.id,
        model: response.model,
        messages: messages.clone(),
        usage: response.usage.map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        }),
        created: response.created,
        provider: Provider::OpenAI,
    })
}

fn parse_openai_responses_api(response: OpenAIResponsesResponse, messages: &mut Vec<ChatMessage>) -> Result<ChatResponse> {
    for output in response.output {
        // Parse content parts (could include text and images)
        let mut parts = Vec::new();
        
        for content_part in &output.content {
            match content_part.content_type.as_str() {
                "text" | "input_text" | "output_text" => {
                    if let Some(text) = &content_part.text {
                        parts.push(crate::types::MessagePart::Text {
                            part_type: "text".to_string(),
                            text: text.clone(),
                        });
                    }
                }
                "image_url" => {
                    if let Some(image_value) = &content_part.image_url {
                        if let Some(image_obj) = image_value.as_object() {
                            if let Some(url) = image_obj.get("url").and_then(|v| v.as_str()) {
                                parts.push(crate::types::MessagePart::Image {
                                    part_type: "image_url".to_string(),
                                    image_url: crate::types::ImageContent {
                                        url: url.to_string(),
                                        detail: image_obj.get("detail").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                    },
                                });
                            }
                        }
                    }
                }
                unknown_type => {
                    console_warn!("Unknown OpenAI Responses API content type: {}", unknown_type);
                }
            }
        }
        
        let message_content = if parts.is_empty() {
            None
        } else if parts.len() == 1 && matches!(&parts[0], crate::types::MessagePart::Text { .. }) {
            // Single text part - use simple text content
            if let crate::types::MessagePart::Text { text, .. } = &parts[0] {
                Some(crate::types::MessageContent::Text(text.clone()))
            } else {
                None
            }
        } else {
            // Multiple parts or contains images
            Some(crate::types::MessageContent::Parts(parts))
        };
        
        let message = ChatMessage {
            role: parse_role(&output.role),
            content: message_content,
            name: None,
            tool_calls: None,
            tool_call_id: None,
            refusal: None,
            is_streaming: false,
            finish_reason: None,
            metadata: Default::default(),
        };
        
        messages.push(message);
    }
    
    Ok(ChatResponse {
        id: response.id,
        model: response.model,
        messages: messages.clone(),
        usage: response.usage.map(|u| Usage {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
            total_tokens: u.total_tokens,
        }),
        created: response.created_at,
        provider: Provider::OpenAI,
    })
}

fn parse_openai_streaming_response(response_str: &str, messages: &mut Vec<ChatMessage>) -> Result<ChatResponse> {
    let mut combined_content = String::new();
    let mut model = String::new();
    let mut id = String::new();
    
    for line in response_str.lines() {
        if line.starts_with("data: ") {
            let data_str = &line[6..];
            if data_str == "[DONE]" {
                break;
            }
            
            if let Ok(chunk) = serde_json::from_str::<Value>(data_str) {
                if id.is_empty() {
                    id = chunk["id"].as_str().unwrap_or("").to_string();
                    model = chunk["model"].as_str().unwrap_or("").to_string();
                }
                
                if let Some(choices) = chunk["choices"].as_array() {
                    for choice in choices {
                        if let Some(delta) = choice.get("delta") {
                            if let Some(content) = delta["content"].as_str() {
                                combined_content.push_str(content);
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Only stream if content is substantial
    let should_stream = !combined_content.trim().is_empty() && combined_content.len() > 10;
    let mut message = ChatMessage::assistant(combined_content);
    message.is_streaming = should_stream;
    messages.push(message);
    
    Ok(ChatResponse {
        id,
        model,
        messages: messages.clone(),
        usage: None,
        created: None,
        provider: Provider::OpenAI,
    })
}

fn parse_anthropic_response(response_str: &str, messages: &mut Vec<ChatMessage>) -> Result<ChatResponse> {
    // Check if this is a streaming response (SSE format)
    if response_str.starts_with("event:") {
        return parse_anthropic_streaming_response(response_str, messages);
    }
    
    let response: AnthropicResponse = serde_json::from_str(response_str)
        .with_context(|| format!("Failed to parse Anthropic response. Response preview: {}", 
            &response_str[..response_str.len().min(200)]))?;
    
    // Collect text content
    let text_content = response.content
        .iter()
        .filter_map(|c| c.text.as_ref())
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("");
    
    // Collect tool calls
    let tool_calls: Vec<ToolCall> = response.content
        .iter()
        .filter(|c| c.content_type == "tool_use")
        .filter_map(|c| {
            match (c.id.as_ref(), c.name.as_ref(), c.input.as_ref()) {
                (Some(id), Some(name), Some(input)) => {
                    Some(ToolCall {
                        id: id.clone(),
                        tool_type: "function".to_string(),
                        function: crate::types::FunctionCall {
                            name: name.clone(),
                            arguments: serde_json::to_string(input).unwrap_or_default(),
                        },
                    })
                }
                _ => None
            }
        })
        .collect();
    
    let mut message = ChatMessage::assistant(text_content);
    if !tool_calls.is_empty() {
        message.tool_calls = Some(tool_calls);
    }
    
    // Set finish reason from stop_reason
    message.finish_reason = response.stop_reason.clone();
    
    messages.push(message);
    
    Ok(ChatResponse {
        id: response.id,
        model: response.model,
        messages: messages.clone(),
        usage: response.usage.map(|u| Usage {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
            total_tokens: u.input_tokens.zip(u.output_tokens).map(|(i, o)| i + o),
        }),
        created: None,
        provider: Provider::Anthropic,
    })
}

fn parse_anthropic_streaming_response(response_str: &str, messages: &mut Vec<ChatMessage>) -> Result<ChatResponse> {
    let mut combined_content = String::new();
    let mut model = String::new();
    let mut id = String::new();
    let mut input_tokens = None;
    let mut output_tokens = None;
    
    // Parse SSE format
    for line in response_str.lines() {
        if line.starts_with("data: ") {
            let data_str = &line[6..];
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(data_str) {
                // Extract model and ID from message_start
                if data["type"] == "message_start" {
                    if let Some(message) = data.get("message") {
                        id = message["id"].as_str().unwrap_or("").to_string();
                        model = message["model"].as_str().unwrap_or("").to_string();
                        if let Some(usage) = message.get("usage") {
                            input_tokens = usage["input_tokens"].as_i64().map(|x| x as i32);
                            output_tokens = usage["output_tokens"].as_i64().map(|x| x as i32);
                        }
                    }
                }
                // Extract text from content_block_delta events
                else if data["type"] == "content_block_delta" {
                    if let Some(delta) = data.get("delta") {
                        if let Some(text) = delta["text"].as_str() {
                            combined_content.push_str(text);
                        }
                    }
                }
                // Extract final usage from message_delta
                else if data["type"] == "message_delta" {
                    if let Some(usage) = data.get("usage") {
                        if let Some(output) = usage["output_tokens"].as_i64() {
                            output_tokens = Some(output as i32);
                        }
                    }
                }
            }
        }
    }
    
    // Only stream if content is substantial
    let should_stream = !combined_content.trim().is_empty() && combined_content.len() > 10;
    let mut message = ChatMessage::assistant(combined_content);
    message.is_streaming = should_stream;
    messages.push(message);
    
    Ok(ChatResponse {
        id,
        model,
        messages: messages.clone(),
        usage: if input_tokens.is_some() || output_tokens.is_some() {
            Some(Usage {
                prompt_tokens: input_tokens,
                completion_tokens: output_tokens,
                total_tokens: input_tokens.zip(output_tokens).map(|(i, o)| i + o),
            })
        } else {
            None
        },
        created: None,
        provider: Provider::Anthropic,
    })
}

fn parse_google_response(response_str: &str, messages: &mut Vec<ChatMessage>) -> Result<ChatResponse> {
    let response: Value = serde_json::from_str(response_str)
        .context("Failed to parse Google response")?;
    
    if let Some(candidates) = response["candidates"].as_array() {
        for candidate in candidates {
            if let Some(content) = candidate.get("content") {
                if let Some(parts) = content["parts"].as_array() {
                    for part in parts {
                        if let Some(text) = part["text"].as_str() {
                            let message = ChatMessage::assistant(text.to_string());
                            // is_streaming is set to false by default
                            messages.push(message);
                        }
                    }
                }
            }
        }
    }
    
    Ok(ChatResponse {
        id: response["id"].as_str().unwrap_or("").to_string(),
        model: response["model"].as_str().unwrap_or("").to_string(),
        messages: messages.clone(),
        usage: None,
        created: None,
        provider: Provider::Google,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_turn_cassette_parsing() {
        let cassette_content = r#"interactions:
- request:
    body: '{"messages":[{"role":"user","content":"What is machine learning?"},{"role":"assistant","content":"Machine learning is a subset of artificial intelligence..."},{"role":"user","content":"Can you give me an example?"}],"model":"gpt-4o","max_completion_tokens":200}'
    uri: https://api.openai.com/v1/chat/completions
  response:
    body:
      string: '{"id": "chatcmpl-test", "object": "chat.completion", "created": 1750105908, "model": "gpt-4o-2024-08-06", "choices": [{"index": 0, "message": {"role": "assistant", "content": "Certainly! One common example...", "refusal": null}, "finish_reason": "length"}], "usage": {"prompt_tokens": 36, "completion_tokens": 200, "total_tokens": 236}}'
"#;

        let result = parse_cassette(cassette_content).unwrap();
        
        println!("Parsed {} messages", result.messages.len());
        for (i, msg) in result.messages.iter().enumerate() {
            println!("Message {}: {:?} - {:?}", i, msg.role, msg.get_text_content());
        }
        
        // Check we have all messages: 2 from request + 1 from response
        assert_eq!(result.messages.len(), 4, "Should have 4 messages total: 3 from request + 1 from response");
        
        assert_eq!(result.messages[0].role, crate::types::MessageRole::User);
        assert_eq!(result.messages[1].role, crate::types::MessageRole::Assistant); 
        assert_eq!(result.messages[2].role, crate::types::MessageRole::User);
        assert_eq!(result.messages[3].role, crate::types::MessageRole::Assistant);
    }

    #[test]
    fn test_openai_responses_api_parsing() {
        // Test parsing an OpenAI Responses API cassette (uses /v1/responses endpoint)
        let responses_cassette = r#"interactions:
- request:
    body: '{"input":"Hello, how are you?","model":"gpt-4o"}'
    uri: https://api.openai.com/v1/responses
  response:
    body:
      string: '{"id": "resp_123", "model": "gpt-4o-2024-08-06", "output": [{"id": "msg_456", "type": "message", "content": [{"type": "output_text", "text": "Hello! I am just a program, so I do not have feelings, but I am here and ready to help."}], "role": "assistant"}], "usage": {"input_tokens": 13, "output_tokens": 29, "total_tokens": 42}}'
"#;

        let result = parse_cassette(responses_cassette).unwrap();
        
        println!("OpenAI Responses API parsed {} messages", result.messages.len());
        for (i, msg) in result.messages.iter().enumerate() {
            println!("Message {}: {:?} - {:?}", i, msg.role, msg.get_text_content());
        }
        
        // Should have 1 message from request input + 1 from response output
        assert_eq!(result.messages.len(), 2, "Should have 2 messages total");
        
        assert_eq!(result.messages[0].role, crate::types::MessageRole::User);
        assert_eq!(result.messages[1].role, crate::types::MessageRole::Assistant);
        
        // Verify content is parsed correctly
        assert_eq!(result.messages[0].get_text_content().unwrap(), "Hello, how are you?");
        assert!(result.messages[1].get_text_content().unwrap().contains("Hello! I am just a program"));
        
        // Verify provider detection
        assert_eq!(result.provider, crate::types::Provider::OpenAI);
        
        // Verify usage parsing
        assert!(result.usage.is_some());
        let usage = result.usage.unwrap();
        assert_eq!(usage.prompt_tokens, Some(13));
        assert_eq!(usage.completion_tokens, Some(29));
        assert_eq!(usage.total_tokens, Some(42));
    }

    #[test]
    fn test_real_multi_turn_cassette_file() {
        let cassette_content = include_str!("../../samples/cassettes/openai_chat_completions/test_multi_turn_conversation.yaml");
        
        let result = parse_cassette(cassette_content).unwrap();
        
        println!("Real cassette parsed {} messages", result.messages.len());
        for (i, msg) in result.messages.iter().enumerate() {
            println!("Message {}: {:?} - {:?}", i, msg.role, msg.get_text_content().map(|s| s.chars().take(100).collect::<String>()));
        }
        
        // Check we have all messages: 3 from request + 1 from response  
        assert_eq!(result.messages.len(), 4, "Should have 4 messages total: 3 from request + 1 from response");
        
        assert_eq!(result.messages[0].role, crate::types::MessageRole::User);
        assert_eq!(result.messages[1].role, crate::types::MessageRole::Assistant); 
        assert_eq!(result.messages[2].role, crate::types::MessageRole::User);
        assert_eq!(result.messages[3].role, crate::types::MessageRole::Assistant);
        
        // Verify content is parsed correctly
        assert!(result.messages[0].get_text_content().unwrap().contains("What is machine learning?"));
        assert!(result.messages[2].get_text_content().unwrap().contains("Can you give me an example?"));
    }

    #[test]
    fn test_unknown_provider_handling() {
        let cassette_content = r#"interactions:
- request:
    body: '{"messages":[{"role":"user","content":"Hello"}]}'
    uri: https://unknown-llm-provider.com/v1/chat
  response:
    body:
      string: '{"id": "test", "message": "Response from unknown provider"}'
"#;

        let result = parse_cassette(cassette_content).unwrap();
        
        // Should handle unknown provider gracefully
        assert!(matches!(result.provider, crate::types::Provider::Unknown(_)));
        // Unknown providers don't parse messages from the request body by default
        assert_eq!(result.messages.len(), 0);
    }

    #[test]
    fn test_unknown_content_types() {
        // Test OpenAI with unknown content type
        let cassette_content = r#"interactions:
- request:
    body: '{"messages":[{"role":"user","content":[{"type":"text","text":"Hello"},{"type":"audio","audio_url":"http://example.com/audio.mp3"}]}]}'
    uri: https://api.openai.com/v1/chat/completions
  response:
    body:
      string: '{"id": "test", "model": "gpt-4", "choices": [{"message": {"role": "assistant", "content": "Hi there!"}}]}'
"#;

        let result = parse_cassette(cassette_content).unwrap();
        
        // Should parse successfully, ignoring unknown content type
        assert_eq!(result.provider, crate::types::Provider::OpenAI);
        assert_eq!(result.messages.len(), 2);
        
        // Check that the text part was parsed correctly
        if let Some(crate::types::MessageContent::Parts(parts)) = &result.messages[0].content {
            assert_eq!(parts.len(), 1); // Only text part should be parsed
            assert!(matches!(&parts[0], crate::types::MessagePart::Text { text, .. } if text == "Hello"));
        } else {
            panic!("Expected Parts content");
        }
    }

    #[test]
    fn test_malformed_multimodal_content() {
        // Test with malformed image content
        let cassette_content = r#"interactions:
- request:
    body: '{"messages":[{"role":"user","content":[{"type":"text","text":"Look at this"},{"type":"image_url","image_url":{"bad_field":"no_url"}}]}]}'
    uri: https://api.openai.com/v1/chat/completions
  response:
    body:
      string: '{"id": "test", "model": "gpt-4", "choices": [{"message": {"role": "assistant", "content": "I cannot see the image"}}]}'
"#;

        let result = parse_cassette(cassette_content).unwrap();
        
        // Should handle malformed image gracefully
        assert_eq!(result.messages.len(), 2);
        
        // Only text should be parsed from the malformed multimodal content
        if let Some(crate::types::MessageContent::Parts(parts)) = &result.messages[0].content {
            assert_eq!(parts.len(), 1); // Only text part
            assert!(matches!(&parts[0], crate::types::MessagePart::Text { text, .. } if text == "Look at this"));
        } else {
            panic!("Expected Parts content");
        }
    }

    #[test]
    fn test_empty_content_array() {
        let cassette_content = r#"interactions:
- request:
    body: '{"messages":[{"role":"user","content":[]}]}'
    uri: https://api.openai.com/v1/chat/completions
  response:
    body:
      string: '{"id": "test", "model": "gpt-4", "choices": [{"message": {"role": "assistant", "content": "Empty content?"}}]}'
"#;

        let result = parse_cassette(cassette_content).unwrap();
        
        // Should handle empty content array
        assert_eq!(result.messages.len(), 2);
        
        // First message should have empty Parts
        if let Some(crate::types::MessageContent::Parts(parts)) = &result.messages[0].content {
            assert_eq!(parts.len(), 0);
        } else {
            panic!("Expected Parts content");
        }
    }

    #[test]
    fn test_openai_error_response() {
        let cassette_content = r#"interactions:
- request:
    body: '{"messages":[{"role":"user","content":"Hello"}]}'
    uri: https://api.openai.com/v1/chat/completions
  response:
    body:
      string: '{"error": {"message": "Invalid API key", "type": "invalid_request_error", "param": null, "code": "invalid_api_key"}}'
"#;

        let result = parse_cassette(cassette_content).unwrap();
        
        // Should handle error response
        assert_eq!(result.messages.len(), 2); // Request + error message
        assert_eq!(result.messages[1].role, crate::types::MessageRole::System);
        assert!(result.messages[1].get_text_content().unwrap().contains("Invalid API key"));
    }

    #[test]
    fn test_anthropic_tool_use_parsing() {
        let cassette_content = r#"interactions:
- request:
    body: '{"messages":[{"role":"user","content":"What is the weather in Tokyo?"}],"tools":[{"name":"get_weather","input_schema":{"type":"object"}}]}'
    uri: https://api.anthropic.com/v1/messages
  response:
    body:
      string: '{"id":"msg_123","type":"message","role":"assistant","model":"claude-3-sonnet-20240229","content":[{"type":"text","text":"I will check the weather for you."},{"type":"tool_use","id":"tool_123","name":"get_weather","input":{"location":"Tokyo"}}],"usage":{"input_tokens":100,"output_tokens":50}}'
"#;

        let result = parse_cassette(cassette_content).unwrap();
        
        // Should parse tool use correctly
        assert_eq!(result.messages.len(), 2);
        let assistant_msg = &result.messages[1];
        assert_eq!(assistant_msg.role, crate::types::MessageRole::Assistant);
        assert_eq!(assistant_msg.get_text_content().unwrap(), "I will check the weather for you.");
        
        // Check tool calls
        assert!(assistant_msg.tool_calls.is_some());
        let tool_calls = assistant_msg.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "tool_123");
        assert_eq!(tool_calls[0].function.name, "get_weather");
        assert!(tool_calls[0].function.arguments.contains("Tokyo"));
    }
}