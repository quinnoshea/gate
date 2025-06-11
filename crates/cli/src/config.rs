//! CLI configuration utilities

use anyhow::Result;
use hellas_gate_daemon::DaemonConfig;
use std::path::Path;

/// Load daemon configuration from JSON file
#[allow(dead_code)]
pub fn load_daemon_config<P: AsRef<Path>>(path: P) -> Result<DaemonConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: DaemonConfig = serde_json::from_str(&content)?;
    Ok(config)
}

/// Save daemon configuration to JSON file
pub fn save_daemon_config<P: AsRef<Path>>(config: &DaemonConfig, path: P) -> Result<()> {
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Generate a default configuration file
pub fn generate_default_config<P: AsRef<Path>>(path: P) -> Result<()> {
    let config = DaemonConfig::default();
    save_daemon_config(&config, path)
}
