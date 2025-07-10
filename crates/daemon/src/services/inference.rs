use crate::config::{LocalInferenceConfig, LocalModelConfig};
use anyhow;
use async_trait::async_trait;
use gate_core::inference::{
    AnthropicContent, AnthropicMessage, AnthropicMessageRequest, AnthropicMessageResponse,
    AnthropicMessageStream, AnthropicUsage, ChatChoice, ChatCompletionRequest,
    ChatCompletionResponse, ChatCompletionStream, ChatMessage, InferenceBackend, LocalModel, Usage,
};
use gate_core::{Error, Result};
use std::collections::HashMap;
use tracing::{debug, instrument, warn};

pub struct LocalInferenceService {
    #[allow(dead_code)]
    config: LocalInferenceConfig,
    model_configs: HashMap<String, LocalModelConfig>,
}

impl LocalInferenceService {
    pub fn new(config: LocalInferenceConfig) -> Result<Self> {
        let mut model_configs = HashMap::new();
        for model_config in &config.models {
            model_configs.insert(model_config.id.clone(), model_config.clone());
        }

        Ok(Self {
            config,
            model_configs,
        })
    }

    fn convert_chat_to_prompt(&self, messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();

        for message in messages {
            match message.role.as_str() {
                "system" => {
                    prompt.push_str(&format!("System: {}\n\n", message.content));
                }
                "user" => {
                    prompt.push_str(&format!("User: {}\n\n", message.content));
                }
                "assistant" => {
                    prompt.push_str(&format!("Assistant: {}\n\n", message.content));
                }
                _ => {
                    warn!("Unknown role: {}", message.role);
                }
            }
        }

        prompt.push_str("Assistant: ");
        prompt
    }

    fn convert_anthropic_to_prompt(
        &self,
        messages: &[AnthropicMessage],
        system: Option<&str>,
    ) -> String {
        let mut prompt = String::new();

        if let Some(system) = system {
            prompt.push_str(&format!("System: {system}\n\n"));
        }

        for message in messages {
            match message.role.as_str() {
                "user" => {
                    prompt.push_str(&format!("User: {}\n\n", message.content));
                }
                "assistant" => {
                    prompt.push_str(&format!("Assistant: {}\n\n", message.content));
                }
                _ => {
                    warn!("Unknown role: {}", message.role);
                }
            }
        }

        prompt.push_str("Assistant: ");
        prompt
    }
}

#[async_trait]
impl InferenceBackend for LocalInferenceService {
    #[instrument(skip(self, request))]
    async fn chat_completions(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        let model_config = self
            .model_configs
            .get(&request.model)
            .ok_or_else(|| Error::ModelNotFound(request.model.clone()))?;

        let prompt = self.convert_chat_to_prompt(&request.messages);
        let temperature = request
            .temperature
            .unwrap_or(model_config.default_temperature);
        let max_tokens = request
            .max_tokens
            .unwrap_or(model_config.default_max_tokens);

        debug!("Running inference with prompt length: {}", prompt.len());

        // TODO: Integrate with actual inference engine (catgrad, llama.cpp, etc.)
        let response = format!(
            "This is a mock response from local model '{}'. Temperature: {}, Max tokens: {}. \
             In a real implementation, this would run inference on the prompt: '{}'",
            request.model,
            temperature,
            max_tokens,
            prompt.chars().take(100).collect::<String>()
        );

        let usage = Usage {
            prompt_tokens: prompt.split_whitespace().count() as u32,
            completion_tokens: response.split_whitespace().count() as u32,
            total_tokens: (prompt.split_whitespace().count() + response.split_whitespace().count())
                as u32,
        };

        Ok(ChatCompletionResponse {
            id: format!("local-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: request.model,
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: response,
                    name: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Some(usage),
        })
    }

    async fn chat_completions_stream(
        &self,
        _request: ChatCompletionRequest,
    ) -> Result<ChatCompletionStream> {
        // TODO: Implement streaming when we integrate a real inference engine
        Err(Error::Internal(
            "Streaming not yet implemented for local inference".to_string(),
        ))
    }

    async fn messages(&self, request: AnthropicMessageRequest) -> Result<AnthropicMessageResponse> {
        let model_config = self
            .model_configs
            .get(&request.model)
            .ok_or_else(|| Error::ModelNotFound(request.model.clone()))?;

        let prompt = self.convert_anthropic_to_prompt(&request.messages, request.system.as_deref());
        let temperature = request
            .temperature
            .unwrap_or(model_config.default_temperature);
        let max_tokens = request.max_tokens;

        // TODO: Integrate with actual inference engine
        let response = format!(
            "This is a mock response from local model '{}' using Anthropic format. \
             Temperature: {}, Max tokens: {}. Prompt preview: '{}'",
            request.model,
            temperature,
            max_tokens,
            prompt.chars().take(100).collect::<String>()
        );

        let usage = AnthropicUsage {
            input_tokens: prompt.split_whitespace().count() as u32,
            output_tokens: response.split_whitespace().count() as u32,
        };

        Ok(AnthropicMessageResponse {
            id: format!("msg_{}", uuid::Uuid::new_v4()),
            model: request.model,
            role: "assistant".to_string(),
            content: vec![AnthropicContent {
                content_type: "text".to_string(),
                text: response,
            }],
            stop_reason: Some("stop_sequence".to_string()),
            stop_sequence: None,
            usage,
        })
    }

    async fn messages_stream(
        &self,
        _request: AnthropicMessageRequest,
    ) -> Result<AnthropicMessageStream> {
        // TODO: Implement streaming when we integrate a real inference engine
        Err(Error::Internal(
            "Streaming not yet implemented for local inference".to_string(),
        ))
    }

    async fn list_models(&self) -> Result<Vec<LocalModel>> {
        let mut models = Vec::new();

        for (id, config) in &self.model_configs {
            models.push(LocalModel {
                id: id.clone(),
                name: config.name.clone(),
                provider: "local".to_string(),
                path: Some(config.path.to_string_lossy().to_string()),
                context_length: config.context_length,
                supports_chat: config.supports_chat,
                supports_completion: config.supports_completion,
            });
        }

        Ok(models)
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<LocalModel>> {
        if let Some(config) = self.model_configs.get(model_id) {
            Ok(Some(LocalModel {
                id: model_id.to_string(),
                name: config.name.clone(),
                provider: "local".to_string(),
                path: Some(config.path.to_string_lossy().to_string()),
                context_length: config.context_length,
                supports_chat: config.supports_chat,
                supports_completion: config.supports_completion,
            }))
        } else {
            Ok(None)
        }
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }
}

pub struct LocalInferenceServiceBuilder {
    config: LocalInferenceConfig,
}

impl LocalInferenceServiceBuilder {
    pub fn new(config: LocalInferenceConfig) -> Self {
        Self { config }
    }

    pub fn build(self) -> anyhow::Result<LocalInferenceService> {
        LocalInferenceService::new(self.config).map_err(|e| anyhow::anyhow!("{}", e))
    }
}
