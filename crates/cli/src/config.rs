//! CLI configuration utilities

use anyhow::Result;
use hellas_gate_daemon::DaemonConfig;
use hellas_gate_relay::RelayConfig;
use std::path::Path;

/// Load daemon configuration from JSON file with environment overrides
pub fn load_daemon_config<P: AsRef<Path>>(path: Option<P>) -> Result<DaemonConfig> {
    if let Some(config_path) = path {
        let settings = config::Config::builder()
            .add_source(config::File::from(config_path.as_ref()))
            .add_source(config::Environment::with_prefix("GATE"))
            .build()?;
        Ok(settings.try_deserialize()?)
    } else {
        // Load from environment with defaults
        let defaults = DaemonConfig::default();
        let settings = config::Config::builder()
            .set_default("http.bind_addr", defaults.http.bind_addr.to_string())?
            .set_default("http.cors_enabled", defaults.http.cors_enabled)?
            .set_default("http.timeout_secs", defaults.http.timeout_secs)?
            .set_default("p2p.discovery_enabled", defaults.p2p.discovery_enabled)?
            .set_default("p2p.bootstrap_peers", defaults.p2p.bootstrap_peers)?
            .set_default("p2p.port", defaults.p2p.port)?
            .set_default("upstream.default_url", defaults.upstream.default_url)?
            .set_default("upstream.timeout_secs", defaults.upstream.timeout_secs)?
            .set_default("upstream.test_model", defaults.upstream.test_model)?
            .add_source(config::Environment::with_prefix("GATE"))
            .build()?;
        Ok(settings.try_deserialize()?)
    }
}

/// Load relay configuration from JSON file with environment overrides
pub fn load_relay_config<P: AsRef<Path>>(path: Option<P>) -> Result<RelayConfig> {
    if let Some(config_path) = path {
        let settings = config::Config::builder()
            .add_source(config::File::from(config_path.as_ref()))
            .add_source(config::Environment::with_prefix("GATE_RELAY"))
            .build()?;
        Ok(settings.try_deserialize()?)
    } else {
        // Load from environment with defaults
        let defaults = RelayConfig::default();
        let settings = config::Config::builder()
            .set_default("https.bind_addr", defaults.https.bind_addr.to_string())?
            .set_default("https.timeout_secs", defaults.https.timeout_secs)?
            .set_default("p2p.discovery_enabled", defaults.p2p.discovery_enabled)?
            .set_default("p2p.port", defaults.p2p.port)?
            .set_default("dns.base_domain", defaults.dns.base_domain)?
            .set_default("dns.provider", defaults.dns.provider)?
            .set_default(
                "dns.update_interval_secs",
                defaults.dns.update_interval_secs,
            )?
            .add_source(config::Environment::with_prefix("GATE_RELAY"))
            .build()?;
        Ok(settings.try_deserialize()?)
    }
}

/// Save daemon configuration to JSON file
pub fn save_daemon_config<P: AsRef<Path>>(config: &DaemonConfig, path: P) -> Result<()> {
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Save relay configuration to JSON file
pub fn save_relay_config<P: AsRef<Path>>(config: &RelayConfig, path: P) -> Result<()> {
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Generate a default daemon configuration file
pub fn generate_default_daemon_config<P: AsRef<Path>>(path: P) -> Result<()> {
    let config = DaemonConfig::default();
    save_daemon_config(&config, path)
}

/// Generate a default relay configuration file
pub fn generate_default_relay_config<P: AsRef<Path>>(path: P) -> Result<()> {
    let config = RelayConfig::default();
    save_relay_config(&config, path)
}
