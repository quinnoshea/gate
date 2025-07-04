use serde_json::Value;
use std::collections::HashMap;

pub mod parser;

// Include the auto-generated cassettes list
include!(concat!(env!("OUT_DIR"), "/cassettes.rs"));

/// Get a cassette by provider and name
pub fn get_cassette(provider: &str, name: &str) -> Option<&'static str> {
    CASSETTES
        .iter()
        .find(|(p, n, _)| p == &provider && n == &name)
        .map(|(_, _, content)| *content)
}

/// Get a cassette as parsed JSON
pub fn get_cassette_json(provider: &str, name: &str) -> Option<Value> {
    get_cassette(provider, name).and_then(|content| serde_json::from_str(content).ok())
}

/// List all cassettes for a given provider
pub fn list_cassettes(provider: &str) -> Vec<&'static str> {
    CASSETTES
        .iter()
        .filter(|(p, _, _)| p == &provider)
        .map(|(_, name, _)| *name)
        .collect()
}

/// List all available cassettes grouped by provider
pub fn list_all_cassettes() -> HashMap<&'static str, Vec<&'static str>> {
    let mut result = HashMap::new();

    for (provider, name, _) in CASSETTES {
        result.entry(*provider).or_insert_with(Vec::new).push(*name);
    }

    result
}

/// Get a conversation from a cassette
pub fn get_conversation(provider: &str, name: &str) -> Option<parser::Conversation> {
    get_cassette(provider, name).and_then(|content| parser::Conversation::from_json(content).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_all_cassettes() {
        let all_cassettes = list_all_cassettes();
        let mut total_parsed = 0;
        let mut total_failed = 0;

        for (provider, cassette_names) in all_cassettes {
            println!("\nTesting provider: {provider}");

            for cassette_name in cassette_names {
                match get_conversation(provider, cassette_name) {
                    Some(conversation) => {
                        total_parsed += 1;
                        println!(
                            "  ✓ {cassette_name} - {} interactions, streaming: {}",
                            conversation.interactions.len(),
                            conversation.is_streaming()
                        );

                        // Verify we can access request/response bodies
                        for (i, interaction) in conversation.interactions.iter().enumerate() {
                            assert!(
                                !interaction.request.body.is_empty(),
                                "Request body should not be empty for {cassette_name} interaction {i}"
                            );
                            assert!(
                                !interaction.response.body.string.is_empty(),
                                "Response body should not be empty for {cassette_name} interaction {i}"
                            );

                            // Try to parse request body
                            if let Ok(req_json) =
                                serde_json::from_str::<Value>(&interaction.request.body)
                            {
                                // Request body is valid JSON
                                assert!(req_json.is_object() || req_json.is_array());
                            }

                            // For non-streaming, response body should be valid JSON
                            if !conversation.is_streaming()
                                && let Ok(resp_json) =
                                    serde_json::from_str::<Value>(&interaction.response.body.string)
                            {
                                assert!(resp_json.is_object() || resp_json.is_array());
                            }
                        }
                    }
                    None => {
                        total_failed += 1;
                        println!("  ✗ {cassette_name} - Failed to parse");
                    }
                }
            }
        }

        println!("\n=== Summary ===");
        println!("Total cassettes parsed: {total_parsed}");
        println!("Total failures: {total_failed}");

        assert_eq!(total_failed, 0, "All cassettes should parse successfully");
        assert!(
            total_parsed > 0,
            "Should have at least one cassette to parse"
        );
    }

    #[test]
    fn test_conversation_accessors() {
        // Test with a known cassette
        if let Some(conversation) = get_conversation("openai", "basic_response") {
            // Test request body access
            let req_body = conversation
                .request_body(0)
                .expect("Should have request body");
            assert!(req_body.get("input").is_some());
            assert!(req_body.get("model").is_some());

            // Test response body access
            let resp_body = conversation
                .response_body(0)
                .expect("Should have response body");
            assert!(resp_body.get("id").is_some());
            assert!(resp_body.get("output").is_some());

            // Test that it's not streaming
            assert!(!conversation.is_streaming());
        }
    }
}
