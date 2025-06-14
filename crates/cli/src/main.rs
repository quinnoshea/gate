//! Gate CLI - P2P AI inference network

mod commands;
mod config;
mod logging;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use commands::Commands;
use std::time::Duration;
use tracing::{error, info, Level};

#[derive(Parser)]
#[command(name = "gate")]
#[command(about = "A P2P AI inference network")]
#[command(version)]
struct Cli {
    /// Set logging level
    #[arg(short = 'l', long, global = true, default_value = "info")]
    log_level: LogLevel,

    /// Data directory for all component data (configs, identity, peer info, logs, etc.)
    #[arg(short = 'd', long, global = true)]
    data_dir: Option<std::path::PathBuf>,

    /// Timeout for operations in seconds (0 = no timeout)
    #[arg(short = 't', long, global = true, default_value = "5")]
    timeout: u64,

    /// Disable file logging (only log to stderr)
    #[arg(long, global = true)]
    no_file_log: bool,

    #[command(subcommand)]
    command: Commands,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install default crypto provider for rustls
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("Failed to install rustls crypto provider"))?;

    let cli = Cli::parse();

    // Initialize logging with component name
    let component = match &cli.command {
        Commands::Daemon { .. } => "daemon",
        Commands::Relay { .. } => "relay",
        _ => "cli",
    };
    logging::init_logging(cli.log_level.into(), cli.data_dir.clone(), component, cli.no_file_log)?;

    info!("Starting Gate CLI");

    // Execute command with optional timeout
    if cli.timeout == 0 {
        // No timeout - run indefinitely
        match cli.command.execute(cli.data_dir).await {
            Ok(()) => {
                info!("Command completed successfully");
            }
            Err(e) => {
                error!("Command failed: {e}");
                std::process::exit(1);
            }
        }
    } else {
        // Execute command with timeout
        let timeout_duration = Duration::from_secs(cli.timeout);
        match tokio::time::timeout(timeout_duration, cli.command.execute(cli.data_dir)).await {
            Ok(Ok(())) => {
                info!("Command completed successfully");
            }
            Ok(Err(e)) => {
                error!("Command failed: {e}");
                std::process::exit(1);
            }
            Err(_) => {
                error!("Command timed out after {} seconds", cli.timeout);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

#[derive(Clone, Debug, ValueEnum)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for Level {
    fn from(log_level: LogLevel) -> Self {
        match log_level {
            LogLevel::Error => Level::ERROR,
            LogLevel::Warn => Level::WARN,
            LogLevel::Info => Level::INFO,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Trace => Level::TRACE,
        }
    }
}
