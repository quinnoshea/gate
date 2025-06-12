use anyhow::Result;
use std::path::PathBuf;
use tracing::Level;
use tracing_appender::rolling;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize logging for the CLI
pub fn init_logging(log_level: Level, data_dir: Option<PathBuf>, component: &str) -> Result<()> {
    let is_long_running = matches!(component, "daemon" | "relay");

    if is_long_running {
        // For daemon/relay commands - write to component-specific file
        init_file_logging(log_level, data_dir, component)
    } else {
        // For CLI commands - write to cli log file
        init_file_logging(log_level, data_dir, "cli")
    }
}

fn init_file_logging(level: Level, data_dir: Option<PathBuf>, component: &str) -> Result<()> {
    let logs_dir = get_logs_dir(data_dir, component)?;
    std::fs::create_dir_all(&logs_dir)?;

    let level_str = level.as_str().to_lowercase();

    // Create file appender with daily rotation (blocking for guaranteed writes)
    // Files will be named YYYY-MM-DD in the logs directory
    let file_appender = rolling::daily(&logs_dir, "");

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("gate={level_str},hellas_gate_daemon={level_str},hellas_gate_p2p={level_str},hellas_gate_relay={level_str}").into()
            }),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false) // No color codes in files
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(true) // Keep colors for console
        )
        .init();

    Ok(())
}

fn get_logs_dir(data_dir: Option<PathBuf>, component: &str) -> Result<PathBuf> {
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

    Ok(base_dir.join(component).join("logs"))
}
