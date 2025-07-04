pub mod anthropic;
pub mod google;
pub mod openai;
pub mod streaming;

use crate::types::{ChatResponse, Provider};
use crate::utils::cassette_loader::CassetteInteraction;
use anyhow::Result;
use serde_json::Value;

/// Decompress gzip data if needed
fn decompress_response_body(
    body: &str,
    headers: &std::collections::HashMap<String, Vec<String>>,
) -> Result<String> {
    // Check if the response is gzip-compressed
    let is_gzipped = headers
        .get("Content-Encoding")
        .or_else(|| headers.get("content-encoding"))
        .map(|values| values.iter().any(|v| v.to_lowercase().contains("gzip")))
        .unwrap_or(false);

    if !is_gzipped {
        return Ok(body.to_string());
    }

    // Try to decode base64 first (for binary YAML data)
    // Since we decompress when capturing, just return the body as-is
    Ok(body.to_string())
}

/// Parse a cassette interaction into a flexible chat response
pub fn parse_interaction(interaction: &CassetteInteraction) -> Result<ChatResponse> {
    let provider = Provider::from_uri(&interaction.request.uri);
    let request_body: Value = serde_json::from_str(&interaction.request.body)?;

    // Decompress response body if needed
    let response_body = decompress_response_body(
        &interaction.response.body.string,
        &interaction.response.headers,
    )?;

    // Check if this is a streaming response (SSE format)
    if response_body.starts_with("data: ")
        || response_body.starts_with("event: ")
        || response_body.contains("\ndata: ")
    {
        streaming::parse_streaming_response(&response_body, provider, request_body)
    } else {
        // Parse as regular JSON response
        let response_json: Value = serde_json::from_str(&response_body)?;

        match provider {
            Provider::OpenAI => openai::parse_response(response_json, request_body),
            Provider::Anthropic => anthropic::parse_response(response_json, request_body),
            Provider::Google => google::parse_response(response_json, request_body),
            Provider::Unknown(_) => {
                // Generic parsing - just try to deserialize directly
                let mut response: ChatResponse = serde_json::from_value(response_json)?;
                response.provider = provider;
                Ok(response)
            }
        }
    }
}
