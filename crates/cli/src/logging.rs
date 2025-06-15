use anyhow::Result;
use std::fs::OpenOptions;
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use opentelemetry::{global, trace::TracerProvider};
use opentelemetry::KeyValue;
use opentelemetry_sdk::{runtime, trace as sdk_trace, Resource};
use tracing_opentelemetry::OpenTelemetryLayer;

/// Initialize logging for the CLI
pub fn init_logging(log_level: Level, data_dir: Option<PathBuf>, component: &str, no_file_log: bool) -> Result<()> {
    // Initialize OpenTelemetry if OTEL_ENDPOINT is set
    let otel_layer = init_opentelemetry(component)?;
    
    if no_file_log {
        // Only log to stderr
        init_stderr_logging(log_level, otel_layer)
    } else {
        let is_long_running = matches!(component, "daemon" | "relay");

        if is_long_running {
            // For daemon/relay commands - write to .state/daemon.log or .state/relay.log
            init_file_logging(log_level, data_dir, component, otel_layer)
        } else {
            // For CLI commands - write to .state/cli.log
            init_file_logging(log_level, data_dir, "cli", otel_layer)
        }
    }
}

/// Initialize OpenTelemetry if OTEL_ENDPOINT environment variable is set
fn init_opentelemetry(component: &str) -> Result<Option<OpenTelemetryLayer<tracing_subscriber::Registry, sdk_trace::Tracer>>> {
    if let Ok(endpoint) = std::env::var("OTEL_ENDPOINT") {
        let service_name = format!("gate-{}", component);
        
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .build()?;
            
        let provider = sdk_trace::TracerProvider::builder()
            .with_batch_exporter(exporter, runtime::Tokio)
            .with_resource(Resource::new(vec![
                KeyValue::new("service.name", service_name),
            ]))
            .build();
            
        global::set_tracer_provider(provider.clone());
        let tracer = provider.tracer("gate");

        Ok(Some(tracing_opentelemetry::layer().with_tracer(tracer)))
    } else {
        Ok(None)
    }
}

/// Shutdown OpenTelemetry gracefully
pub fn shutdown_opentelemetry() {
    global::shutdown_tracer_provider();
}

fn init_file_logging(level: Level, data_dir: Option<PathBuf>, component: &str, otel_layer: Option<OpenTelemetryLayer<tracing_subscriber::Registry, sdk_trace::Tracer>>) -> Result<()> {
    let log_file_path = get_log_file_path(data_dir, component)?;

    // Ensure parent directory exists
    if let Some(parent) = log_file_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Create/truncate the log file
    let log_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_file_path)?;

    let level_str = level.as_str().to_lowercase();
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        format!("gate={level_str},hellas_gate_daemon={level_str},hellas_gate_p2p={level_str},hellas_gate_relay={level_str}").into()
    });

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(log_file)
        .with_ansi(false); // No color codes in files
        
    let console_layer = tracing_subscriber::fmt::layer()
        .with_ansi(true); // Keep colors for console

    if let Some(otel) = otel_layer {
        tracing_subscriber::registry()
            .with(otel)
            .with(env_filter)
            .with(file_layer)
            .with(console_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .with(console_layer)
            .init();
    }

    Ok(())
}

fn init_stderr_logging(level: Level, otel_layer: Option<OpenTelemetryLayer<tracing_subscriber::Registry, sdk_trace::Tracer>>) -> Result<()> {
    let level_str = level.as_str().to_lowercase();
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        format!("gate={level_str},hellas_gate_daemon={level_str},hellas_gate_p2p={level_str},hellas_gate_relay={level_str}").into()
    });

    let console_layer = tracing_subscriber::fmt::layer()
        .with_ansi(true); // Keep colors for console

    if let Some(otel) = otel_layer {
        tracing_subscriber::registry()
            .with(otel)
            .with(env_filter)
            .with(console_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(console_layer)
            .init();
    }

    Ok(())
}

fn get_log_file_path(data_dir: Option<PathBuf>, component: &str) -> Result<PathBuf> {
    let base_dir = data_dir.unwrap_or_else(|| {
        // Check environment variable first, then fall back to system data dir
        if let Ok(gate_data_dir) = std::env::var("GATE_STATE_DIR") {
            PathBuf::from(gate_data_dir)
        } else {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("gate")
        }
    });

    let log_filename = format!("{}.log", component);
    Ok(base_dir.join(log_filename))
}
