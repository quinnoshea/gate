#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod commands;
mod state;

use gate_core::tracing::init::init_file_logging;
use gate_daemon::StateDir;
use state::DaemonState;
use tauri::Manager;

fn main() {
    // Initialize rustls crypto provider for TLS connections
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Initialize file-based logging for the GUI app
    let state_dir = StateDir::new();
    let data_dir = state_dir.data_dir();
    let log_guard = init_file_logging(&data_dir, None).expect("Failed to initialize file logging");

    tracing::info!("Gate GUI starting - logs: {}/logs/", data_dir.display());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(DaemonState::new())
        .manage(log_guard) // Keep log guard alive for the application lifetime
        .invoke_handler(tauri::generate_handler![
            commands::start_daemon,
            commands::stop_daemon,
            commands::daemon_status,
            commands::get_daemon_config,
            commands::restart_daemon,
            commands::get_daemon_status,
            commands::get_daemon_runtime_config,
            commands::get_tlsforward_status,
            commands::configure_tlsforward,
            commands::enable_tlsforward,
            commands::disable_tlsforward,
            commands::get_bootstrap_url,
            commands::get_bootstrap_token,
            commands::get_bootstrap_token_from_logs,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // Get the app handle from the window
                let app = window.app_handle();
                if let Some(state) = app.try_state::<DaemonState>() {
                    let _ = tauri::async_runtime::block_on(commands::stop_daemon(state));
                }
            }
        })
        .setup(|app| {
            // Optionally start the daemon automatically on app launch
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Wait a moment for the app to fully initialize
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                if let Some(state) = handle.try_state::<DaemonState>() {
                    // Auto-start daemon with default config
                    match commands::start_daemon(state, handle.clone(), None).await {
                        Ok(msg) => tracing::info!("{}", msg),
                        Err(e) => tracing::error!("Failed to auto-start daemon: {}", e),
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
