//! Inference stream protocol for chat completions and other AI requests

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Request ID for correlating requests and responses
pub type RequestId = String;

/// Envelope for inference requests sent over P2P
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub request_id: RequestId,
    pub payload: InferencePayload,
}

/// Envelope for inference responses sent over P2P
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResponse {
    pub request_id: RequestId,
    pub payload: InferenceResponsePayload,
}

/// Types of inference requests
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum InferencePayload {
    /// Chat completion request (`OpenAI` compatible)
    ChatCompletion(ChatCompletionRequest),
    /// List available models
    ListModels,
    /// Get model information
    ModelInfo { model: String },
}

/// Types of inference responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum InferenceResponsePayload {
    /// Chat completion response or chunk
    ChatCompletion(ChatCompletionResponse),
    /// Available models list
    ModelsList(ModelsResponse),
    /// Model information
    ModelInfo(ModelInfoResponse),
    /// Error occurred
    Error(ErrorResponse),
}

/// Chat completion request (subset of `OpenAI` API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

/// Chat message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system", "user", "assistant"
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Chat completion response (`OpenAI` compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String, // "chat.completion" or "chat.completion.chunk"
    pub created: u64,   // Unix timestamp
    pub model: String,
    pub choices: Vec<ChatChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// Individual choice in chat completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<ChatMessage>, // For non-streaming
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<ChatDelta>, // For streaming
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>, // "stop", "length", "content_filter"
}

/// Delta for streaming responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// List of available models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub object: String, // "list"
    pub data: Vec<ModelData>,
}

/// Information about a specific model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelData {
    pub id: String,
    pub object: String, // "model"
    pub created: u64,   // Unix timestamp
    pub owned_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<Vec<String>>,
    // Gate-specific extensions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>, // "ollama", "lmstudio"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,
}

/// Model information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfoResponse {
    pub model: String,
    pub provider: String,
    pub context_length: Option<u32>,
    pub capabilities: Vec<String>,
    pub metadata: HashMap<String, String>,
}

/// Error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

/// Error detail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, String>>,
}

impl InferenceRequest {
    /// Create a new chat completion request
    #[must_use]
    pub const fn chat_completion(request_id: RequestId, request: ChatCompletionRequest) -> Self {
        Self {
            request_id,
            payload: InferencePayload::ChatCompletion(request),
        }
    }

    /// Create a list models request
    #[must_use]
    pub const fn list_models(request_id: RequestId) -> Self {
        Self {
            request_id,
            payload: InferencePayload::ListModels,
        }
    }

    /// Create a model info request
    #[must_use]
    pub const fn model_info(request_id: RequestId, model: String) -> Self {
        Self {
            request_id,
            payload: InferencePayload::ModelInfo { model },
        }
    }

    /// Serialize to JSON bytes
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails
    pub fn to_bytes(&self) -> crate::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Deserialize from JSON bytes
    ///
    /// # Errors
    ///
    /// Returns an error if JSON deserialization fails or the data is malformed
    pub fn from_bytes(bytes: &[u8]) -> crate::Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

impl InferenceResponse {
    /// Create a chat completion response
    #[must_use]
    pub const fn chat_completion(request_id: RequestId, response: ChatCompletionResponse) -> Self {
        Self {
            request_id,
            payload: InferenceResponsePayload::ChatCompletion(response),
        }
    }

    /// Create a models list response
    #[must_use]
    pub const fn models_list(request_id: RequestId, models: ModelsResponse) -> Self {
        Self {
            request_id,
            payload: InferenceResponsePayload::ModelsList(models),
        }
    }

    /// Create an error response
    #[must_use]
    pub const fn error(request_id: RequestId, error: ErrorResponse) -> Self {
        Self {
            request_id,
            payload: InferenceResponsePayload::Error(error),
        }
    }

    /// Serialize to JSON bytes
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails
    pub fn to_bytes(&self) -> crate::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Deserialize from JSON bytes
    ///
    /// # Errors
    ///
    /// Returns an error if JSON deserialization fails or the data is malformed
    pub fn from_bytes(bytes: &[u8]) -> crate::Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

/// Generate a unique request ID
#[must_use]
pub fn generate_request_id() -> RequestId {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_completion_request_serialization() {
        let request = InferenceRequest::chat_completion(
            "test-123".to_string(),
            ChatCompletionRequest {
                model: "llama2".to_string(),
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: "Hello!".to_string(),
                    name: None,
                }],
                stream: true,
                temperature: Some(0.7),
                max_tokens: Some(100),
                top_p: None,
                frequency_penalty: None,
                presence_penalty: None,
                stop: None,
            },
        );

        let bytes = request.to_bytes().unwrap();
        let deserialized = InferenceRequest::from_bytes(&bytes).unwrap();

        assert_eq!(request.request_id, deserialized.request_id);

        if let (InferencePayload::ChatCompletion(orig), InferencePayload::ChatCompletion(deser)) =
            (&request.payload, &deserialized.payload)
        {
            assert_eq!(orig.model, deser.model);
            assert_eq!(orig.stream, deser.stream);
            assert_eq!(orig.messages.len(), deser.messages.len());
        } else {
            panic!("Payload type mismatch");
        }
    }

    #[test]
    fn test_streaming_response() {
        let response = InferenceResponse::chat_completion(
            "test-123".to_string(),
            ChatCompletionResponse {
                id: "chatcmpl-123".to_string(),
                object: "chat.completion.chunk".to_string(),
                created: 1_677_652_288,
                model: "llama2".to_string(),
                choices: vec![ChatChoice {
                    index: 0,
                    message: None,
                    delta: Some(ChatDelta {
                        role: None,
                        content: Some("Hello".to_string()),
                    }),
                    finish_reason: None,
                }],
                usage: None,
            },
        );

        let bytes = response.to_bytes().unwrap();
        let deserialized = InferenceResponse::from_bytes(&bytes).unwrap();

        assert_eq!(response.request_id, deserialized.request_id);

        if let InferenceResponsePayload::ChatCompletion(chat_response) = &deserialized.payload {
            assert_eq!(chat_response.object, "chat.completion.chunk");
            assert_eq!(chat_response.choices.len(), 1);
            assert!(chat_response.choices[0].delta.is_some());
        } else {
            panic!("Expected ChatCompletion response");
        }
    }

    #[test]
    fn test_error_response() {
        let error_response = InferenceResponse::error(
            "test-123".to_string(),
            ErrorResponse {
                error: ErrorDetail {
                    code: "model_not_found".to_string(),
                    message: "The requested model was not found".to_string(),
                    details: Some([("model".to_string(), "nonexistent-model".to_string())].into()),
                },
            },
        );

        let bytes = error_response.to_bytes().unwrap();
        let deserialized = InferenceResponse::from_bytes(&bytes).unwrap();

        if let InferenceResponsePayload::Error(error) = &deserialized.payload {
            assert_eq!(error.error.code, "model_not_found");
            assert!(error.error.details.is_some());
        } else {
            panic!("Expected Error response");
        }
    }

    #[test]
    fn test_request_id_generation() {
        let id1 = generate_request_id();
        let id2 = generate_request_id();

        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 32); // 16 bytes as hex = 32 chars
        assert_eq!(id2.len(), 32);
    }
}
