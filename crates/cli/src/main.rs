//! Gate CLI - P2P AI inference network

mod commands;
mod config;

use anyhow::Result;
use clap::Parser;
use commands::Commands;
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "gate")]
#[command(about = "A P2P AI inference network")]
#[command(version)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Configuration file path
    #[arg(short, long, global = true)]
    config: Option<std::path::PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(cli.verbose);

    info!("Starting Gate CLI");

    // Execute command
    if let Err(e) = cli.command.execute(cli.config).await {
        error!("Command failed: {e}");
        std::process::exit(1);
    }

    Ok(())
}

fn init_logging(verbose: bool) {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let level = if verbose { "debug" } else { "info" };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("gate={level},hellas_gate_daemon={level},hellas_gate_p2p={level}").into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}
