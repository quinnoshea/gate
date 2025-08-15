//! Configuration for tracing and instrumentation
//!
//! This module provides configuration types for setting up tracing with
//! optional OpenTelemetry export.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main instrumentation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentationConfig {
    /// Service name for tracing
    pub service_name: String,
    /// Service version
    pub service_version: String,
    /// Log level filter (e.g., "info", "debug", "trace")
    pub log_level: String,
    /// Optional OTLP configuration for OpenTelemetry export
    #[serde(default)]
    pub otlp: Option<OtlpConfig>,
}

/// OpenTelemetry Protocol (OTLP) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpConfig {
    /// OTLP endpoint URL
    pub endpoint: String,
    /// Optional headers to send with OTLP requests
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
}

/// File-based logging configuration
///
/// This configuration controls how logs are written to files with size-based rotation.
/// When logs exceed max_file_size_mb, they are rotated and older logs are maintained
/// up to max_files total files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFileConfig {
    /// Directory where log files should be written
    pub directory: PathBuf,
    /// Prefix for log file names (e.g., "gate" creates "gate.log")
    pub file_prefix: String,
    /// Maximum size in MB before rotating to a new file
    pub max_file_size_mb: u64,
    /// Maximum number of rotated log files to keep
    pub max_files: usize,
    /// Whether to also output logs to console (in addition to files)
    pub console_enabled: bool,
}

impl Default for InstrumentationConfig {
    fn default() -> Self {
        Self {
            service_name: "gate".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            log_level: "info".to_string(),
            otlp: None,
        }
    }
}

impl Default for LogFileConfig {
    fn default() -> Self {
        // Use the same state directory pattern as existing Gate components
        let state_dir = std::env::var("GATE_STATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                if cfg!(target_os = "windows") {
                    dirs::data_dir()
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join("hellas")
                        .join("gate")
                } else if cfg!(target_os = "macos") {
                    dirs::data_dir()
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join("com.hellas.gate")
                } else {
                    dirs::data_dir()
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join("gate")
                }
            });

        Self {
            directory: state_dir.join("logs"),
            file_prefix: "gate".to_string(),
            max_file_size_mb: 10,
            max_files: 10,
            console_enabled: cfg!(debug_assertions), // Console enabled in debug, not in release
        }
    }
}

impl InstrumentationConfig {
    /// Create configuration from environment variables
    ///
    /// Supports the following environment variables:
    /// - `OTEL_SERVICE_NAME` or `SERVICE_NAME`: Service name
    /// - `OTEL_SERVICE_VERSION` or `SERVICE_VERSION`: Service version
    /// - `RUST_LOG`: Log level filter
    /// - `OTEL_EXPORTER_OTLP_ENDPOINT`: OTLP endpoint URL
    /// - `OTEL_EXPORTER_OTLP_HEADERS`: Comma-separated headers (key=value)
    pub fn from_env() -> Self {
        let service_name = std::env::var("OTEL_SERVICE_NAME")
            .or_else(|_| std::env::var("SERVICE_NAME"))
            .unwrap_or_else(|_| "gate".to_string());

        let service_version = std::env::var("OTEL_SERVICE_VERSION")
            .or_else(|_| std::env::var("SERVICE_VERSION"))
            .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());

        let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

        let otlp = if let Ok(endpoint) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
            let headers = std::env::var("OTEL_EXPORTER_OTLP_HEADERS")
                .ok()
                .and_then(|h| {
                    let map: HashMap<String, String> = h
                        .split(',')
                        .filter_map(|pair| {
                            let parts: Vec<&str> = pair.splitn(2, '=').collect();
                            if parts.len() == 2 {
                                Some((parts[0].to_string(), parts[1].to_string()))
                            } else {
                                None
                            }
                        })
                        .collect();
                    if map.is_empty() { None } else { Some(map) }
                });

            Some(OtlpConfig { endpoint, headers })
        } else {
            None
        };

        Self {
            service_name,
            service_version,
            log_level,
            otlp,
        }
    }

    /// Create a development configuration with sensible defaults
    pub fn dev() -> Self {
        Self {
            service_name: "gate-dev".to_string(),
            service_version: "dev".to_string(),
            log_level: "debug".to_string(),
            otlp: None,
        }
    }

    /// Create a configuration for Jaeger export
    pub fn with_jaeger(endpoint: impl Into<String>) -> Self {
        Self {
            otlp: Some(OtlpConfig {
                endpoint: endpoint.into(),
                headers: None,
            }),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = InstrumentationConfig::default();
        assert_eq!(config.service_name, "gate");
        assert_eq!(config.service_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(config.log_level, "info");
        assert!(config.otlp.is_none());
    }

    #[test]
    fn test_dev_config() {
        let config = InstrumentationConfig::dev();
        assert_eq!(config.service_name, "gate-dev");
        assert_eq!(config.service_version, "dev");
        assert_eq!(config.log_level, "debug");
        assert!(config.otlp.is_none());
    }

    #[test]
    fn test_with_jaeger() {
        let config = InstrumentationConfig::with_jaeger("http://localhost:4317");
        assert!(config.otlp.is_some());
        let otlp = config.otlp.unwrap();
        assert_eq!(otlp.endpoint, "http://localhost:4317");
        assert!(otlp.headers.is_none());
    }

    #[test]
    fn test_log_file_config_default() {
        let config = LogFileConfig::default();
        assert_eq!(config.file_prefix, "gate");
        assert_eq!(config.max_file_size_mb, 10);
        assert_eq!(config.max_files, 10);
        assert_eq!(config.console_enabled, cfg!(debug_assertions));
        assert!(config.directory.ends_with("logs"));
    }

    #[test]
    fn test_log_file_config_custom() {
        use std::path::PathBuf;

        let config = LogFileConfig {
            directory: PathBuf::from("/tmp/test-logs"),
            file_prefix: "test".to_string(),
            max_file_size_mb: 5,
            max_files: 15,
            console_enabled: true,
        };

        assert_eq!(config.directory, PathBuf::from("/tmp/test-logs"));
        assert_eq!(config.file_prefix, "test");
        assert_eq!(config.max_file_size_mb, 5);
        assert_eq!(config.max_files, 15);
        assert!(config.console_enabled);
    }

    #[test]
    fn test_log_file_config_serialization() {
        let config = LogFileConfig::default();

        // Test serialization to JSON
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("gate"));
        assert!(json.contains("10"));

        // Test deserialization from JSON
        let deserialized: LogFileConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.file_prefix, config.file_prefix);
        assert_eq!(deserialized.max_file_size_mb, config.max_file_size_mb);
        assert_eq!(deserialized.max_files, config.max_files);
        assert_eq!(deserialized.console_enabled, config.console_enabled);
        assert_eq!(deserialized.directory, config.directory);
    }
}
