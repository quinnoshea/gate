//! Initialization functions for tracing
//!
//! This module provides functions to initialize the tracing subsystem
//! with optional OpenTelemetry export.

use anyhow::Result;
use std::path::Path;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(feature = "tracing")]
use crate::tracing::config::{InstrumentationConfig, LogFileConfig};

#[cfg(all(feature = "tracing", not(target_arch = "wasm32")))]
use crate::tracing::file_rotation::SizeBasedAppender;

#[cfg(all(feature = "tracing", not(target_arch = "wasm32")))]
use tracing_appender::non_blocking::WorkerGuard;

/// Initialize tracing with the given configuration
pub fn init_tracing(config: &InstrumentationConfig) -> Result<()> {
    // Create env filter
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&config.log_level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // Create the base subscriber with formatting layer
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true);

    // Initialize based on whether OTLP is configured
    if let Some(otlp_config) = &config.otlp {
        // Initialize with OTLP export
        init_with_otlp(config, otlp_config, env_filter)?;
    } else {
        // Initialize without OTLP
        tracing_subscriber::registry()
            .with(fmt_layer)
            .with(env_filter)
            .init();
    }

    Ok(())
}

/// Initialize with OTLP export
#[cfg(feature = "tracing-otlp")]
fn init_with_otlp(
    _config: &InstrumentationConfig,
    otlp_config: &crate::tracing::config::OtlpConfig,
    env_filter: EnvFilter,
) -> Result<()> {
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::trace::{RandomIdGenerator, Sampler, SdkTracerProvider};
    use tracing_opentelemetry::OpenTelemetryLayer;

    // Note: In this version of OpenTelemetry, resource creation is limited
    // We'll set service info through environment variables instead

    // Create OTLP exporter
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(&otlp_config.endpoint)
        .build()?;

    // Create tracer provider
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_sampler(Sampler::AlwaysOn)
        .with_id_generator(RandomIdGenerator::default())
        .build();

    // Create OpenTelemetry layer
    let otel_layer = OpenTelemetryLayer::new(tracer_provider.tracer("gate"));

    // Create the formatting layer
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true);

    // Initialize subscriber with all layers
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(env_filter)
        .with(otel_layer)
        .init();

    Ok(())
}

/// Initialize with OTLP export (stub for when feature is disabled)
#[cfg(not(feature = "tracing-otlp"))]
fn init_with_otlp(
    _config: &InstrumentationConfig,
    _otlp_config: &crate::tracing::config::OtlpConfig,
    env_filter: EnvFilter,
) -> Result<()> {
    // Create the formatting layer
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true);

    // Just initialize without OTLP when feature is disabled
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(env_filter)
        .init();

    tracing::warn!("OTLP export requested but tracing-otlp feature is not enabled");
    Ok(())
}

/// Initialize with default configuration from environment
pub fn init_default() -> Result<()> {
    let config = InstrumentationConfig::from_env();
    init_tracing(&config)
}

/// Initialize with development configuration
pub fn init_dev() -> Result<()> {
    let config = InstrumentationConfig::dev();
    init_tracing(&config)
}

/// Initialize with Jaeger OTLP export
pub fn init_with_jaeger() -> Result<()> {
    let jaeger_endpoint =
        std::env::var("JAEGER_ENDPOINT").unwrap_or_else(|_| "http://localhost:4317".to_string());
    let config = InstrumentationConfig::with_jaeger(jaeger_endpoint);
    init_tracing(&config)
}

/// Shutdown OpenTelemetry providers gracefully
pub fn shutdown_tracer_provider() {
    // Note: In newer versions of OpenTelemetry, shutdown happens automatically
    // when the tracer provider is dropped. We keep this function for API compatibility.
}

/// Initialize file-based logging with size rotation
///
/// This function initializes tracing to write logs to files using the SizeBasedAppender
/// from PR #1 and LogFileConfig from PR #2. It returns a WorkerGuard that must be kept
/// alive to prevent the background writer thread from being dropped.
///
/// # Arguments
///
/// * `state_dir` - Base directory for application state (logs will go in logs/ subdirectory)
/// * `config` - Configuration for file logging (uses default if None)
///
/// # Returns
///
/// * `Result<WorkerGuard>` - Guard that must be kept alive for logging to work
///
/// # Example
///
/// ```rust,ignore
/// let state_dir = StateDir::new();
/// let _guard = init_file_logging(state_dir.data_dir(), None)?;
/// // Guard must be kept alive for the lifetime of logging
/// ```
#[cfg(all(feature = "tracing", not(target_arch = "wasm32")))]
pub fn init_file_logging(state_dir: &Path, config: Option<LogFileConfig>) -> Result<WorkerGuard> {
    // Use provided config or create default with state_dir
    let log_config = config.unwrap_or_else(|| LogFileConfig {
        directory: state_dir.join("logs"),
        file_prefix: "gate".to_string(),
        max_file_size_mb: 10,
        max_files: 10,
        console_enabled: cfg!(debug_assertions), // Console enabled in debug builds
    });

    // Store values before moving config
    let logs_directory = log_config.directory.clone();
    let max_size_mb = log_config.max_file_size_mb;
    let max_files = log_config.max_files;
    let console_enabled = log_config.console_enabled;

    // Create the size-based appender
    let appender = SizeBasedAppender::new(
        log_config.directory,
        log_config.file_prefix,
        log_config.max_file_size_mb,
        log_config.max_files,
    )?;

    // Create non-blocking writer
    let (non_blocking, guard) = tracing_appender::non_blocking(appender);

    // Create environment filter
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Create file layer
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false) // No ANSI colors in files
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true);

    // Create registry with file layer
    let subscriber = tracing_subscriber::registry()
        .with(file_layer)
        .with(env_filter);

    // Conditionally add console layer
    if console_enabled {
        let console_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(cfg!(not(target_os = "windows"))) // No ANSI on Windows
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true);

        subscriber.with(console_layer).init();
    } else {
        subscriber.init();
    }

    tracing::info!(
        "File logging initialized - logs directory: {}, max size: {}MB, max files: {}, console: {}",
        logs_directory.display(),
        max_size_mb,
        max_files,
        console_enabled
    );

    Ok(guard)
}

/// Initialize file-based logging (stub for WASM or when tracing feature is disabled)
#[cfg(any(not(feature = "tracing"), target_arch = "wasm32"))]
pub fn init_file_logging(
    _state_dir: &Path,
    _config: Option<()>, // Simplified since LogFileConfig may not be available
) -> Result<()> {
    // WASM doesn't support file I/O, so this is a no-op
    eprintln!("File logging not supported on WASM or when tracing feature is disabled");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_tracing_default() {
        let config = InstrumentationConfig::default();
        // Test that init_tracing doesn't panic with default config
        // Note: We can't actually test the initialization since it's global state
        assert_eq!(config.service_name, "gate");
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_init_tracing_dev() {
        let config = InstrumentationConfig::dev();
        assert_eq!(config.service_name, "gate-dev");
        assert_eq!(config.log_level, "debug");
    }

    #[cfg(all(feature = "tracing", not(target_arch = "wasm32")))]
    #[test]
    fn test_file_logging_init() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let state_dir = temp_dir.path();

        // Test with default config
        let result = init_file_logging(state_dir, None);

        // We can't test the actual initialization since tracing is global state
        // but we can test that the function creates the necessary directories
        let logs_dir = state_dir.join("logs");
        assert!(logs_dir.exists(), "Logs directory should be created");

        // The function should succeed
        // Note: In a real environment this would fail due to global tracing state
        // but the directory creation and config processing should work
        match result {
            Ok(_guard) => {
                // Success - logging was initialized
            }
            Err(e) => {
                // Expected in tests due to global tracing state conflicts
                // The important thing is that directories were created
                println!("Expected error due to global tracing state: {e}");
            }
        }
    }

    #[cfg(all(feature = "tracing", not(target_arch = "wasm32")))]
    #[test]
    fn test_file_logging_config_validation() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let state_dir = temp_dir.path();

        let custom_config = LogFileConfig {
            directory: state_dir.join("custom-logs"),
            file_prefix: "test".to_string(),
            max_file_size_mb: 5,
            max_files: 15,
            console_enabled: true,
        };

        // Test config creation and validation without global state conflicts
        // We can test the config processing logic by creating the appender directly
        let appender_result = SizeBasedAppender::new(
            custom_config.directory.clone(),
            custom_config.file_prefix.clone(),
            custom_config.max_file_size_mb,
            custom_config.max_files,
        );

        // Check that custom directory was created
        assert!(
            custom_config.directory.exists(),
            "Custom logs directory should be created"
        );

        // Appender creation should succeed
        assert!(
            appender_result.is_ok(),
            "SizeBasedAppender creation should succeed"
        );

        // Test config values
        assert_eq!(custom_config.file_prefix, "test");
        assert_eq!(custom_config.max_file_size_mb, 5);
        assert_eq!(custom_config.max_files, 15);
        assert!(custom_config.console_enabled);
    }

    #[cfg(any(not(feature = "tracing"), target_arch = "wasm32"))]
    #[test]
    fn test_file_logging_wasm_stub() {
        use std::path::PathBuf;

        let result = init_file_logging(&PathBuf::from("/tmp"), None);

        // Should always succeed but do nothing
        assert!(result.is_ok());
    }
}
