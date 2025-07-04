use std::env;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("cassettes.rs");

    let mut cassette_list = String::new();
    cassette_list.push_str("// Auto-generated file listing all cassettes\n\n");
    cassette_list.push_str("pub const CASSETTES: &[(&str, &str, &str)] = &[\n");

    // Walk through the data directory
    for entry in WalkDir::new("data")
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let relative_path = path.strip_prefix("data").unwrap();

        // Extract provider and test name from path
        let components: Vec<&str> = relative_path
            .to_str()
            .unwrap()
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        if components.len() >= 2 {
            let provider = components[0];
            let filename = components[components.len() - 1];
            let test_name = filename
                .trim_end_matches(".json")
                .trim_end_matches(".enhanced_json")
                .trim_end_matches(".yaml");

            cassette_list.push_str(&format!(
                "    (\"{}\", \"{}\", include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{}\"))),\n",
                provider, test_name, path.display()
            ));
        }
    }

    cassette_list.push_str("];\n");

    fs::write(&dest_path, cassette_list).unwrap();

    // Tell cargo to rerun this script if the data directory changes
    println!("cargo:rerun-if-changed=data");
}
