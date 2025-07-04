//! Configuration for tracing and instrumentation
//!
//! This module provides configuration types for setting up tracing with
//! optional OpenTelemetry export.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
}
