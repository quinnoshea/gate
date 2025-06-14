use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = PathBuf::from("proto");

    // Collect all .proto files
    let proto_files = [
        // Common types
        "common/v1/types.proto",
        "common/v1/errors.proto",
        "common/v1/session.proto",
        // Protocol definitions
        "control/v1/control.proto",
        "relay/v1/relay.proto",
        "inference/v1/inference.proto",
    ];

    let proto_paths: Vec<PathBuf> = proto_files.iter().map(|f| proto_root.join(f)).collect();

    // Use tonic-build for service generation
    tonic_build::configure()
        .out_dir("src/pb")
        .include_file("mod.rs")
        .build_transport(false)
        .build_client(true)
        .build_server(true)
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile_protos(&proto_paths, &[proto_root])?;

    // Tell cargo to rerun if any proto files change
    for proto_file in &proto_files {
        println!("cargo:rerun-if-changed=proto/{}", proto_file);
    }

    Ok(())
}
