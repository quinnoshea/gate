mod multimodal;

pub use multimodal::{Attachment, ContentBlock, ImageData, ImageSource, MultimodalMessage};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Chat response that captures both typed and dynamic fields
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatResponse {
    // Core fields the UI needs
    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub model: String,

    #[serde(default)]
    pub messages: Vec<ChatMessage>,

    // Provider is determined from URI, not from response
    #[serde(skip)]
    pub provider: Provider,

    // Usage is common enough to deserve its own field
    #[serde(default)]
    pub usage: Option<Usage>,

    // Everything else (service_tier, system_fingerprint, created, annotations, etc.)
    #[serde(flatten)]
    pub metadata: HashMap<String, Value>,
}

/// Message that captures both typed and dynamic fields
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,

    #[serde(default)]
    pub content: Option<Value>, // Can be string, array, or object

    // Common fields
    #[serde(default)]
    pub tool_calls: Option<Vec<Value>>,

    #[serde(default)]
    pub name: Option<String>,

    // Everything else (refusal, function_call, tool_call_id, etc.)
    #[serde(flatten)]
    pub metadata: HashMap<String, Value>,
}

/// Usage information
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    // Common fields across providers
    #[serde(alias = "prompt_tokens", alias = "input_tokens")]
    pub prompt_tokens: Option<i32>,

    #[serde(alias = "completion_tokens", alias = "output_tokens")]
    pub completion_tokens: Option<i32>,

    #[serde(default)]
    pub total_tokens: Option<i32>,

    // Everything else (cache info, token details, etc.)
    #[serde(flatten)]
    pub metadata: HashMap<String, Value>,
}

/// Provider types
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Provider {
    OpenAI,
    Anthropic,
    Google,
    Unknown(String),
}

impl Default for Provider {
    fn default() -> Self {
        Provider::Unknown("unknown".to_string())
    }
}

impl Provider {
    pub fn from_uri(uri: &str) -> Self {
        if uri.contains("openai.com") {
            Provider::OpenAI
        } else if uri.contains("anthropic.com") {
            Provider::Anthropic
        } else if uri.contains("googleapis.com") || uri.contains("google.com") {
            Provider::Google
        } else {
            Provider::Unknown(uri.to_string())
        }
    }
}

/// Helper methods for ChatMessage
impl ChatMessage {
    /// Create a new message with role and content
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: Some(Value::String(content.into())),
            tool_calls: None,
            name: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self::new("system", content)
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self::new("user", content)
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new("assistant", content)
    }

    /// Extract text content from various message formats
    pub fn get_text_content(&self) -> Option<String> {
        match &self.content {
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Array(parts)) => {
                // Handle multipart content
                let texts: Vec<String> = parts
                    .iter()
                    .filter_map(|part| {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            Some(text.to_string())
                        } else {
                            part.get("content")
                                .and_then(|t| t.as_str())
                                .map(|text| text.to_string())
                        }
                    })
                    .collect();

                if texts.is_empty() {
                    None
                } else {
                    Some(texts.join(" "))
                }
            }
            Some(Value::Object(obj)) => {
                // Handle object with text field
                obj.get("text")
                    .or_else(|| obj.get("content"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            }
            _ => None,
        }
    }

    /// Check if this message indicates streaming
    pub fn is_streaming(&self) -> bool {
        self.metadata
            .get("is_streaming")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Get finish reason if available
    pub fn finish_reason(&self) -> Option<String> {
        self.metadata
            .get("finish_reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

// Legacy type aliases for compatibility
pub type MessageRole = String;
pub type MessageContent = Value;
pub type ToolCall = Value;
pub type ToolResponse = Value;
