//! Inference API routes for LLM providers

use crate::{error::HttpError, state::AppState, types::*};
use axum::{
    body::Body,
    extract::{Json, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use gate_core::tracing::{prelude::*, trace_context::inject_trace_context};
use gate_core::{ChatCompletionRequest, MessagesRequest};
use serde_json::{Value as JsonValue, json};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
use tracing::{Instrument, debug, info, instrument, warn};
use utoipa_axum::{router::OpenApiRouter, routes};

/// Create an HTTP client with optional timeout based on target architecture
#[allow(unused_variables)]
fn create_http_client(timeout_seconds: u64) -> Result<reqwest::Client, HttpError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|e| {
                HttpError::InternalServerError(format!("Failed to create HTTP client: {e}"))
            })
    }

    #[cfg(target_arch = "wasm32")]
    {
        // WASM doesn't support timeout on Client::builder()
        reqwest::Client::builder().build().map_err(|e| {
            HttpError::InternalServerError(format!("Failed to create HTTP client: {e}"))
        })
    }
}

/// Handle Anthropic messages requests
#[utoipa::path(
    post,
    path = "/v1/messages",
    request_body = AnthropicMessagesRequest,
    responses(
        (status = 200, description = "Successful response", body = JsonValue),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "inference"
)]
#[instrument(
    name = "anthropic_messages",
    skip(app_state, headers),
    fields(
        model = %request.model,
        request_id = tracing::field::Empty
    )
)]
pub async fn messages_handler<T>(
    State(app_state): State<AppState<T>>,
    headers: HeaderMap,
    axum::Extension(correlation_id): axum::Extension<CorrelationId>,
    Json(request): Json<AnthropicMessagesRequest>,
) -> Result<Response, HttpError>
where
    T: Clone + Send + Sync + 'static,
{
    info!(
        "Received Anthropic messages request for model: {}",
        request.model
    );
    debug!(
        "Request body: {:?}",
        serde_json::to_string(&request).unwrap_or_default()
    );
    let _internal_request = MessagesRequest {
        model: request.model.clone(),
        body: serde_json::to_value(&request)
            .map_err(|e| HttpError::BadRequest(format!("Failed to serialize request: {e}")))?,
        stream: request.stream,
        headers: None,
        api_key: None,
        request_id: None,
        trace_id: None,
        user_id: None,
        organization_id: None,
    };

    // Check if any upstreams are configured
    if app_state.upstream_registry.has_upstreams().await {
        // Get the upstream configuration for this model
        let forwarding_config = match app_state
            .upstream_registry
            .get_upstream_for_model(&request.model)
            .await
        {
            Some(config) => {
                info!(
                    "Found upstream for model {} - forwarding request",
                    request.model
                );
                config
            }
            None => {
                warn!("No upstream configured for model: {}", request.model);
                let all_upstreams = app_state.upstream_registry.get_all_upstreams().await;
                for (name, info) in all_upstreams {
                    debug!("Upstream '{}' has models: {:?}", name, info.models);
                }
                return Err(HttpError::BadRequest(format!(
                    "No upstream configured for model: {}",
                    request.model
                )));
            }
        };
        // Forward the request to the upstream provider
        let client = create_http_client(forwarding_config.timeout_seconds)?;

        let url = format!("{}/messages", forwarding_config.base_url);
        let mut req = client.post(&url).json(&request);

        // Add authentication header
        if let Some((header_name, header_value)) = forwarding_config.auth_header() {
            req = req.header(header_name, header_value);
        }

        // Add provider-specific headers
        for (name, value) in forwarding_config.provider_headers() {
            req = req.header(name, value);
        }

        // Forward relevant headers from the original request
        for (name, value) in headers.iter() {
            let name_str = name.as_str();
            // Skip certain headers that shouldn't be forwarded
            if !matches!(
                name_str,
                "host" | "content-length" | "content-type" | "authorization" | "x-api-key"
            ) {
                req = req.header(name.clone(), value.clone());
            }
        }

        // Inject W3C trace context headers
        let mut trace_headers = HeaderMap::new();
        if let Err(e) = inject_trace_context(correlation_id.trace_context(), &mut trace_headers) {
            warn!("Failed to inject trace context headers: {}", e);
        }
        for (name, value) in trace_headers.iter() {
            req = req.header(name.clone(), value.clone());
        }

        let response = {
            let span = tracing::info_span!(
                "upstream_request",
                url = %url,
                provider = %forwarding_config.provider,
                model = %request.model,
                method = "POST",
                endpoint = "/messages",
                timeout_seconds = forwarding_config.timeout_seconds,
                stream = request.stream,
                request_type = "anthropic_messages"
            );
            req.send().instrument(span).await.map_err(|e| {
                HttpError::ServiceUnavailable(format!("Failed to forward request: {e}"))
            })?
        };

        let status = response.status();
        let headers = response.headers().clone();

        if request.stream {
            // TODO: Implement streaming response handling
            return Err(HttpError::NotImplemented(
                "Streaming not yet implemented".into(),
            ));
        }

        let body = response.json::<JsonValue>().await.map_err(|e| {
            HttpError::InternalServerError(format!("Failed to parse response: {e}"))
        })?;

        let mut response_builder = axum::http::Response::builder().status(status.as_u16());

        // Forward relevant headers from upstream response
        for (name, value) in headers.iter() {
            let name_str = name.as_str();
            // Skip headers that Axum will set automatically
            if !matches!(
                name_str,
                "content-length" | "transfer-encoding" | "connection"
            ) {
                response_builder = response_builder.header(name, value);
            }
        }

        response_builder
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .map_err(|e| HttpError::InternalServerError(format!("Failed to build response: {e}")))
    } else {
        // No forwarding configured, return mock response
        if request.stream {
            return Err(HttpError::NotImplemented(
                "Streaming not yet implemented".into(),
            ));
        }

        let response = json!({
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "text",
                "text": "This is a mock response from the Gate API gateway in Anthropic format."
            }],
            "model": request.model,
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {
                "input_tokens": 10,
                "output_tokens": 15
            }
        });

        Ok(Json(response).into_response())
    }
}

/// Handle OpenAI chat completions requests
#[utoipa::path(
    post,
    path = "/v1/chat/completions",
    request_body = OpenAIChatCompletionRequest,
    responses(
        (status = 200, description = "Successful completion", body = JsonValue),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "inference"
)]
#[instrument(
    name = "openai_chat_completions",
    skip(app_state, headers),
    fields(
        model = %request.model,
        stream = %request.stream
    )
)]
pub async fn chat_completions_handler<T>(
    State(app_state): State<AppState<T>>,
    headers: HeaderMap,
    axum::Extension(correlation_id): axum::Extension<CorrelationId>,
    Json(request): Json<OpenAIChatCompletionRequest>,
) -> Result<Response, HttpError>
where
    T: Clone + Send + Sync + 'static,
{
    info!(
        "Received OpenAI chat completions request for model: {}",
        request.model
    );
    debug!("Request headers: {:?}", headers);
    debug!(
        "Request body: {:?}",
        serde_json::to_string(&request).unwrap_or_default()
    );
    let _internal_request = ChatCompletionRequest {
        model: request.model.clone(),
        body: request.extra.clone(),
        stream: request.stream,
        headers: None,
        api_key: None,
        request_id: None,
        trace_id: None,
        user_id: None,
        organization_id: None,
    };

    // Check if any upstreams are configured
    if app_state.upstream_registry.has_upstreams().await {
        // Get the upstream configuration for this model
        let forwarding_config = match app_state
            .upstream_registry
            .get_upstream_for_model(&request.model)
            .await
        {
            Some(config) => {
                info!(
                    "Found upstream for model {} - forwarding request",
                    request.model
                );
                config
            }
            None => {
                warn!("No upstream configured for model: {}", request.model);
                let all_upstreams = app_state.upstream_registry.get_all_upstreams().await;
                for (name, info) in all_upstreams {
                    debug!("Upstream '{}' has models: {:?}", name, info.models);
                }
                return Err(HttpError::BadRequest(format!(
                    "No upstream configured for model: {}",
                    request.model
                )));
            }
        };
        // Forward the request to the upstream provider
        let client = create_http_client(forwarding_config.timeout_seconds)?;

        let url = format!("{}/chat/completions", forwarding_config.base_url);
        info!("Forwarding request to: {}", url);
        let mut req = client.post(&url).json(&request);

        // Add authentication header
        if let Some((header_name, header_value)) = forwarding_config.auth_header() {
            req = req.header(header_name, header_value);
        }

        // Add provider-specific headers
        for (name, value) in forwarding_config.provider_headers() {
            req = req.header(name, value);
        }

        // Forward relevant headers from the original request
        for (name, value) in headers.iter() {
            let name_str = name.as_str();
            // Skip certain headers that shouldn't be forwarded
            if !matches!(
                name_str,
                "host" | "content-length" | "content-type" | "authorization" | "x-api-key"
            ) {
                req = req.header(name.clone(), value.clone());
            }
        }

        // Inject W3C trace context headers
        let mut trace_headers = HeaderMap::new();
        if let Err(e) = inject_trace_context(correlation_id.trace_context(), &mut trace_headers) {
            warn!("Failed to inject trace context headers: {}", e);
        }
        for (name, value) in trace_headers.iter() {
            req = req.header(name.clone(), value.clone());
        }

        let response = {
            let span = tracing::info_span!(
                "upstream_request",
                url = %url,
                provider = %forwarding_config.provider,
                model = %request.model,
                method = "POST",
                endpoint = "/chat/completions",
                timeout_seconds = forwarding_config.timeout_seconds,
                stream = request.stream,
                request_type = "openai_chat_completions"
            );
            req.send().instrument(span).await.map_err(|e| {
                HttpError::ServiceUnavailable(format!("Failed to forward request: {e}"))
            })?
        };

        let status = response.status();
        let headers = response.headers().clone();

        if request.stream {
            // TODO: Implement streaming response handling
            return Err(HttpError::NotImplemented(
                "Streaming not yet implemented".into(),
            ));
        }

        // Check if the response is an error
        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error response".to_string());
            warn!("Upstream returned error status {}: {}", status, error_body);

            // Try to parse as JSON to get better error details
            if let Ok(error_json) = serde_json::from_str::<JsonValue>(&error_body) {
                return Err(HttpError::InternalServerError(format!(
                    "Upstream error: {status} - {error_json}"
                )));
            }

            return Err(HttpError::InternalServerError(format!(
                "Upstream error: {status} - {error_body}"
            )));
        }

        let body = response.json::<JsonValue>().await.map_err(|e| {
            HttpError::InternalServerError(format!("Failed to parse response: {e}"))
        })?;

        let mut response_builder = axum::http::Response::builder().status(status.as_u16());

        // Forward relevant headers from upstream response
        for (name, value) in headers.iter() {
            let name_str = name.as_str();
            // Skip headers that Axum will set automatically
            if !matches!(
                name_str,
                "content-length" | "transfer-encoding" | "connection"
            ) {
                response_builder = response_builder.header(name, value);
            }
        }

        response_builder
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .map_err(|e| HttpError::InternalServerError(format!("Failed to build response: {e}")))
    } else {
        // No forwarding configured, return mock response
        if request.stream {
            return Err(HttpError::NotImplemented(
                "Streaming not yet implemented".into(),
            ));
        }

        let response = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": chrono::Utc::now().timestamp(),
            "model": request.model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "I'm a mock response from the Gate API gateway."
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 10,
                "total_tokens": 20
            }
        });

        Ok(Json(response).into_response())
    }
}

/// Handle OpenAI responses requests
#[utoipa::path(
    post,
    path = "/v1/responses",
    request_body = OpenAICompletionRequest,
    responses(
        (status = 200, description = "Successful response", body = JsonValue),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "inference"
)]
#[instrument(
    name = "openai_responses",
    skip(app_state, headers),
    fields(
        model = %request.model,
        stream = %request.stream
    )
)]
pub async fn responses_handler<T>(
    State(app_state): State<AppState<T>>,
    headers: HeaderMap,
    axum::Extension(correlation_id): axum::Extension<CorrelationId>,
    Json(request): Json<OpenAICompletionRequest>,
) -> Result<Response, HttpError>
where
    T: Clone + Send + Sync + 'static,
{
    // Check if any upstreams are configured
    if app_state.upstream_registry.has_upstreams().await {
        // Get the upstream configuration for this model
        let forwarding_config = match app_state
            .upstream_registry
            .get_upstream_for_model(&request.model)
            .await
        {
            Some(config) => {
                info!(
                    "Found upstream for model {} - forwarding request",
                    request.model
                );
                config
            }
            None => {
                warn!("No upstream configured for model: {}", request.model);
                let all_upstreams = app_state.upstream_registry.get_all_upstreams().await;
                for (name, info) in all_upstreams {
                    debug!("Upstream '{}' has models: {:?}", name, info.models);
                }
                return Err(HttpError::BadRequest(format!(
                    "No upstream configured for model: {}",
                    request.model
                )));
            }
        };
        // Forward the request to the upstream provider
        let client = create_http_client(forwarding_config.timeout_seconds)?;

        let url = format!("{}/responses", forwarding_config.base_url);
        let mut req = client.post(&url).json(&request);

        // Add authentication header
        if let Some((header_name, header_value)) = forwarding_config.auth_header() {
            req = req.header(header_name, header_value);
        }

        // Add provider-specific headers
        for (name, value) in forwarding_config.provider_headers() {
            req = req.header(name, value);
        }

        // Forward relevant headers from the original request
        for (name, value) in headers.iter() {
            let name_str = name.as_str();
            // Skip certain headers that shouldn't be forwarded
            if !matches!(
                name_str,
                "host" | "content-length" | "content-type" | "authorization" | "x-api-key"
            ) {
                req = req.header(name.clone(), value.clone());
            }
        }

        // Inject W3C trace context headers
        let mut trace_headers = HeaderMap::new();
        if let Err(e) = inject_trace_context(correlation_id.trace_context(), &mut trace_headers) {
            warn!("Failed to inject trace context headers: {}", e);
        }
        for (name, value) in trace_headers.iter() {
            req = req.header(name.clone(), value.clone());
        }

        let response = {
            let span = tracing::info_span!(
                "upstream_request",
                url = %url,
                provider = %forwarding_config.provider,
                model = %request.model,
                method = "POST",
                endpoint = "/responses",
                timeout_seconds = forwarding_config.timeout_seconds,
                stream = request.stream,
                request_type = "openai_responses"
            );
            req.send().instrument(span).await.map_err(|e| {
                HttpError::ServiceUnavailable(format!("Failed to forward request: {e}"))
            })?
        };

        let status = response.status();
        let headers = response.headers().clone();

        if request.stream {
            // TODO: Implement streaming response handling
            return Err(HttpError::NotImplemented(
                "Streaming not yet implemented".into(),
            ));
        }

        let body = response.json::<JsonValue>().await.map_err(|e| {
            HttpError::InternalServerError(format!("Failed to parse response: {e}"))
        })?;

        let mut response_builder = axum::http::Response::builder().status(status.as_u16());

        // Forward relevant headers from upstream response
        for (name, value) in headers.iter() {
            let name_str = name.as_str();
            // Skip headers that Axum will set automatically
            if !matches!(
                name_str,
                "content-length" | "transfer-encoding" | "connection"
            ) {
                response_builder = response_builder.header(name, value);
            }
        }

        response_builder
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .map_err(|e| HttpError::InternalServerError(format!("Failed to build response: {e}")))
    } else {
        // No forwarding configured, return mock response
        if request.stream {
            return Err(HttpError::NotImplemented(
                "Streaming not yet implemented".into(),
            ));
        }

        let response = json!({
            "id": "cmpl-123",
            "object": "text_completion",
            "created": chrono::Utc::now().timestamp(),
            "model": request.model,
            "choices": [{
                "text": "This is a mock completion from the Gate API gateway.",
                "index": 0,
                "logprobs": null,
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 10,
                "total_tokens": 20
            }
        });

        Ok(Json(response).into_response())
    }
}

/// Add inference routes to router
pub fn add_routes<T: Send + Sync + Clone + 'static>(
    router: OpenApiRouter<AppState<T>>,
) -> OpenApiRouter<AppState<T>> {
    router
        .routes(routes!(chat_completions_handler))
        .routes(routes!(responses_handler))
        .routes(routes!(messages_handler))
}
