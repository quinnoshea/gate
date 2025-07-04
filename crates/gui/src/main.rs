#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod daemon;

use daemon::DaemonState;
use tauri::Manager;

fn main() {
    // Initialize rustls crypto provider (required for TLS operations)
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Initialize tracing for the GUI app
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "gate_gui=debug,gate_daemon=debug".to_string()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(DaemonState::new())
        .invoke_handler(tauri::generate_handler![
            daemon::start_daemon,
            daemon::stop_daemon,
            daemon::daemon_status,
            daemon::get_daemon_config,
            daemon::restart_daemon,
            daemon::get_daemon_status,
            daemon::get_daemon_runtime_config,
            daemon::get_tlsforward_status,
            daemon::configure_tlsforward,
            daemon::enable_tlsforward,
            daemon::disable_tlsforward,
            daemon::get_bootstrap_url,
            daemon::get_bootstrap_token,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // Get the app handle from the window
                let app = window.app_handle();
                if let Some(state) = app.try_state::<DaemonState>() {
                    let _ = tauri::async_runtime::block_on(daemon::stop_daemon(state));
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
                    match daemon::start_daemon(state, handle.clone(), None).await {
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
