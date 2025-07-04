use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("cassette_list.rs");

    // Generate code to get cassettes from the fixtures crate
    let code = r#"
use gate_fixtures;

pub fn get_all_cassettes() -> Vec<CassetteInfo> {
    let mut cassettes = Vec::new();
    
    // Get all cassettes from the fixtures crate
    let all_cassettes = gate_fixtures::list_all_cassettes();
    
    for (provider, names) in all_cassettes {
        // Map provider names to display names
        let display_provider = match provider {
            "openai" => "OpenAI Responses",
            _ => provider,
        };
        
        for name in names {
            // Get the cassette content
            if let Some(content) = gate_fixtures::get_cassette(provider, name) {
                // Generate a nice display name
                let display_name = name
                    .replace("_", " ")
                    .split_whitespace()
                    .map(|word| {
                        let mut chars = word.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(first) => first.to_uppercase().chain(chars).collect()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                
                cassettes.push(CassetteInfo {
                    provider: display_provider,
                    name: display_name,
                    content,
                });
            }
        }
    }
    
    cassettes
}
"#;

    fs::write(&dest_path, code).unwrap();

    // The fixtures crate will handle rerun-if-changed for cassettes
}
