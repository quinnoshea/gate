use crate::parsers;
use crate::types::ChatResponse;
use crate::utils::cassette_loader;
use anyhow::Result;

/// Parse a cassette file using the new parsing architecture
pub fn parse_cassette(json_content: &str) -> Result<ChatResponse> {
    // Load the cassette as raw HTTP data
    let interaction = cassette_loader::load_cassette(json_content)?;

    // Parse the interaction into a chat response
    parsers::parse_interaction(&interaction)
}
