//! Identity management utilities

use std::path::Path;
use tracing::info;

/// Load existing identity or generate a new one
pub fn load_or_generate_identity(component_dir: &Path) -> Result<Vec<u8>, std::io::Error> {
    let identity_file = component_dir.join("identity.key");

    if identity_file.exists() {
        let key_data = std::fs::read(&identity_file)?;
        info!("Loaded identity from: {:?}", identity_file);
        Ok(key_data)
    } else {
        // Generate new identity and save it
        let mut rng = rand::thread_rng();
        let secret_key = iroh::SecretKey::generate(&mut rng);
        let key_bytes = secret_key.to_bytes();

        // Create directory and save the key
        std::fs::create_dir_all(component_dir)?;
        std::fs::write(&identity_file, &key_bytes)?;
        info!("Generated and saved new identity to: {:?}", identity_file);

        Ok(key_bytes.to_vec())
    }
}

/// Load identity from specific file path
pub fn load_identity_from_file(identity_path: &Path) -> Result<Vec<u8>, std::io::Error> {
    if !identity_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Identity file not found: {}", identity_path.display()),
        ));
    }

    let key_data = std::fs::read(identity_path)?;
    info!("Loaded identity from: {:?}", identity_path);
    Ok(key_data)
}

/// Derive node ID hex string from identity bytes
pub fn node_id_from_identity(identity: &[u8]) -> Result<String, String> {
    if identity.len() != 32 {
        return Err(format!("Identity must be 32 bytes, got {}", identity.len()));
    }

    let key_array: [u8; 32] = identity
        .try_into()
        .map_err(|_| "Failed to convert identity to key array".to_string())?;
    let secret_key = iroh::SecretKey::from_bytes(&key_array);
    Ok(hex::encode(secret_key.public().as_bytes()))
}
