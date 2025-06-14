use anyhow::Result;
use std::fs::OpenOptions;
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize logging for the CLI
pub fn init_logging(log_level: Level, data_dir: Option<PathBuf>, component: &str) -> Result<()> {
    let is_long_running = matches!(component, "daemon" | "relay");

    if is_long_running {
        // For daemon/relay commands - write to .state/daemon.log or .state/relay.log
        init_file_logging(log_level, data_dir, component)
    } else {
        // For CLI commands - write to .state/cli.log
        init_file_logging(log_level, data_dir, "cli")
    }
}

fn init_file_logging(level: Level, data_dir: Option<PathBuf>, component: &str) -> Result<()> {
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

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("gate={level_str},hellas_gate_daemon={level_str},hellas_gate_p2p={level_str},hellas_gate_relay={level_str}").into()
            }),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(log_file)
                .with_ansi(false) // No color codes in files
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(true) // Keep colors for console
        )
        .init();

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
