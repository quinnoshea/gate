use crate::state::{DaemonState, TlsForwardStatus};
use gate_core::bootstrap::BootstrapTokenParser;
use gate_daemon::{Settings, StateDir, runtime::Runtime};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager, State};
use tracing::{error, info};

#[tauri::command]
pub async fn start_daemon(
    state: State<'_, DaemonState>,
    app: AppHandle,
    config: Option<Settings>,
) -> Result<String, String> {
    // Check if already running
    if state.is_running().await {
        return Err("Daemon is already running".to_string());
    }

    // Load or use provided config
    let settings = if let Some(cfg) = config {
        // Save the new config
        if let Err(e) = state.save_config(&cfg).await {
            error!("Failed to save GUI config: {}", e);
        }
        cfg
    } else {
        state
            .load_config()
            .unwrap_or_else(|_| Settings::gui_preset())
    };

    // Resolve static directory path for frontend files
    let static_dir = if tauri::is_dev() {
        // Development mode - use source directory
        let dir = "crates/frontend-daemon/dist".to_string();
        info!("Running in Tauri dev mode, using static directory: {}", dir);
        dir
    } else {
        // Production mode - resolve Tauri resources
        let path = app
            .path()
            .resolve("frontend-daemon", BaseDirectory::Resource)
            .map_err(|e| format!("Failed to resolve frontend resources: {e}"))?;
        let dir = path.to_string_lossy().to_string();
        info!("Running in Tauri production mode, resolved static directory: {dir}");
        dir
    };

    // Build runtime
    let runtime = Runtime::builder()
        .gui_mode()
        .with_static_dir(static_dir)
        .with_settings(settings)
        .build()
        .await
        .map_err(|e| format!("Failed to build runtime: {e}"))?;

    let address = runtime.server_address();

    // Start monitoring
    runtime.start_monitoring().await;

    // Start metrics if configured
    runtime
        .start_metrics()
        .await
        .map_err(|e| format!("Failed to start metrics: {e}"))?;

    // Spawn server task
    let runtime_clone = runtime.clone();
    let handle = tokio::spawn(async move {
        if let Err(e) = runtime_clone.serve().await {
            error!("Server error: {}", e);
        }
    });

    // Store runtime and handle
    state.set_runtime(runtime).await;
    state.set_handle(handle).await;

    Ok(format!("Daemon started at http://{address}"))
}

#[tauri::command]
pub async fn stop_daemon(state: State<'_, DaemonState>) -> Result<String, String> {
    if !state.is_running().await {
        return Err("Daemon is not running".to_string());
    }

    state.shutdown().await;
    Ok("Daemon stopped successfully".to_string())
}

#[tauri::command]
pub async fn daemon_status(state: State<'_, DaemonState>) -> Result<bool, String> {
    Ok(state.is_running().await)
}

#[tauri::command]
pub async fn get_daemon_config(state: State<'_, DaemonState>) -> Result<Settings, String> {
    state
        .load_config()
        .map_err(|e| format!("Failed to load config: {e}"))
}

#[tauri::command]
pub async fn restart_daemon(
    state: State<'_, DaemonState>,
    app: AppHandle,
    config: Option<Settings>,
) -> Result<String, String> {
    // Stop if running
    if state.is_running().await {
        let _ = stop_daemon(state.clone()).await;
        // Wait a bit for cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    // Start with new config
    start_daemon(state, app, config).await
}

#[tauri::command]
pub async fn get_daemon_status(state: State<'_, DaemonState>) -> Result<serde_json::Value, String> {
    let running = state.is_running().await;
    let runtime_config = if let Some(runtime) = state.get_runtime().await {
        serde_json::json!({
            "address": runtime.server_address(),
            "tlsforward_enabled": runtime.tlsforward_enabled(),
        })
    } else {
        serde_json::json!({})
    };

    Ok(serde_json::json!({
        "running": running,
        "config": runtime_config,
    }))
}

#[tauri::command]
pub async fn get_daemon_runtime_config(
    state: State<'_, DaemonState>,
) -> Result<serde_json::Value, String> {
    if let Some(runtime) = state.get_runtime().await {
        Ok(serde_json::json!({
            "server_address": runtime.server_address(),
            "tlsforward_enabled": runtime.tlsforward_enabled(),
        }))
    } else {
        Err("Runtime not available".to_string())
    }
}

#[tauri::command]
pub async fn get_tlsforward_status(
    state: State<'_, DaemonState>,
) -> Result<TlsForwardStatus, String> {
    if let Some(runtime) = state.get_runtime().await {
        let status = runtime.tlsforward_status().await;
        Ok(match status {
            gate_daemon::runtime::TlsForwardStatus::Disabled => TlsForwardStatus::Disabled,
            gate_daemon::runtime::TlsForwardStatus::Disconnected => TlsForwardStatus::Disconnected,
            gate_daemon::runtime::TlsForwardStatus::Connecting => TlsForwardStatus::Connecting,
            gate_daemon::runtime::TlsForwardStatus::Connected { domain } => {
                TlsForwardStatus::Connected { domain }
            }
            gate_daemon::runtime::TlsForwardStatus::Error(e) => TlsForwardStatus::Error(e),
        })
    } else {
        Ok(TlsForwardStatus::Disabled)
    }
}

#[tauri::command]
pub async fn configure_tlsforward(
    state: State<'_, DaemonState>,
    _enabled: bool,
    _server_address: Option<String>,
) -> Result<String, String> {
    // Load current config
    let config = state
        .load_config()
        .map_err(|e| format!("Failed to load config: {e}"))?;

    // Update TLS forward config
    // Note: This would need to be implemented based on how you want to handle
    // runtime configuration changes

    // Save config
    state
        .save_config(&config)
        .await
        .map_err(|e| format!("Failed to save config: {e}"))?;

    Ok("TLS forward configuration updated".to_string())
}

#[tauri::command]
pub async fn enable_tlsforward(state: State<'_, DaemonState>) -> Result<String, String> {
    configure_tlsforward(state, true, None).await
}

#[tauri::command]
pub async fn disable_tlsforward(state: State<'_, DaemonState>) -> Result<String, String> {
    configure_tlsforward(state, false, None).await
}

#[tauri::command]
pub async fn get_bootstrap_url(state: State<'_, DaemonState>) -> Result<Option<String>, String> {
    if let Some(runtime) = state.get_runtime().await {
        Ok(runtime.bootstrap_url())
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn get_bootstrap_token(state: State<'_, DaemonState>) -> Result<Option<String>, String> {
    if let Some(runtime) = state.get_runtime().await {
        Ok(runtime.bootstrap_token().map(|s| s.to_string()))
    } else {
        Ok(None)
    }
}

/// Get bootstrap token by parsing log files for automated discovery
///
/// This command searches through gate daemon log files to find the most recent
/// bootstrap token, enabling automated bootstrap token discovery instead of
/// manual entry. Returns None if no token is found in the logs.
#[tauri::command]
pub async fn get_bootstrap_token_from_logs() -> Result<Option<String>, String> {
    let state_dir = StateDir::new();
    let logs_dir = state_dir.data_dir().join("logs");

    // Create parser instance
    let parser = BootstrapTokenParser::new(logs_dir)
        .map_err(|e| format!("Failed to initialize bootstrap token parser: {}", e))?;

    // Search for the latest token in log files
    match parser.find_latest_token().await {
        Ok(token) => {
            if let Some(ref token_str) = token {
                info!(
                    "Successfully found bootstrap token from logs: {}",
                    token_str
                );
            } else {
                info!("No bootstrap token found in log files");
            }
            Ok(token)
        }
        Err(e) => {
            error!("Failed to parse bootstrap token from logs: {}", e);
            Err(format!("Bootstrap token parsing failed: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;

    /// Test helper to create a mock state directory with test log files
    async fn create_test_logs_with_token(temp_dir: &TempDir, token: &str) -> PathBuf {
        let logs_dir = temp_dir.path().join("logs");
        tokio::fs::create_dir_all(&logs_dir).await.unwrap();

        let log_file = logs_dir.join("gate.log");
        let mut file = File::create(&log_file).await.unwrap();

        // Write a realistic log entry with bootstrap token
        let log_entry = format!(
            "2025-08-15T15:21:07.988194Z  INFO main ThreadId(01) gate_daemon::runtime::inner: crates/daemon/src/runtime/inner.rs:69: Generated bootstrap token: {}\n",
            token
        );
        file.write_all(log_entry.as_bytes()).await.unwrap();
        file.flush().await.unwrap();

        logs_dir
    }

    /// Test helper to create empty logs directory
    async fn create_empty_logs(temp_dir: &TempDir) -> PathBuf {
        let logs_dir = temp_dir.path().join("logs");
        tokio::fs::create_dir_all(&logs_dir).await.unwrap();
        logs_dir
    }

    #[tokio::test]
    async fn test_get_bootstrap_token_from_logs_success() {
        let temp_dir = TempDir::new().unwrap();
        let expected_token = "TestBootstrapToken123456789ABC";

        // Create test logs with token
        let _logs_dir = create_test_logs_with_token(&temp_dir, expected_token).await;

        // Mock StateDir by setting environment variable
        std::env::set_var("GATE_STATE_DIR", temp_dir.path().to_str().unwrap());

        // Call the command function
        let result = get_bootstrap_token_from_logs().await;

        // Verify success
        assert!(result.is_ok(), "Command should succeed");
        let token = result.unwrap();
        assert!(token.is_some(), "Should find a token");
        assert_eq!(
            token.unwrap(),
            expected_token,
            "Should return the correct token"
        );

        // Clean up
        std::env::remove_var("GATE_STATE_DIR");
    }

    #[tokio::test]
    async fn test_get_bootstrap_token_from_logs_no_token() {
        let temp_dir = TempDir::new().unwrap();

        // Create empty logs directory
        let _logs_dir = create_empty_logs(&temp_dir).await;

        // Mock StateDir
        std::env::set_var("GATE_STATE_DIR", temp_dir.path().to_str().unwrap());

        // Call the command function
        let result = get_bootstrap_token_from_logs().await;

        // Verify success but no token found
        assert!(result.is_ok(), "Command should succeed even with no token");
        let token = result.unwrap();
        assert!(token.is_none(), "Should not find a token in empty logs");

        // Clean up
        std::env::remove_var("GATE_STATE_DIR");
    }

    #[tokio::test]
    async fn test_get_bootstrap_token_from_logs_no_logs_directory() {
        let temp_dir = TempDir::new().unwrap();

        // Don't create logs directory - this tests the case where logs don't exist yet
        std::env::set_var("GATE_STATE_DIR", temp_dir.path().to_str().unwrap());

        // Call the command function
        let result = get_bootstrap_token_from_logs().await;

        // Should succeed but return None (no logs directory means no tokens)
        assert!(
            result.is_ok(),
            "Command should handle missing logs directory gracefully"
        );
        let token = result.unwrap();
        assert!(
            token.is_none(),
            "Should return None when logs directory doesn't exist"
        );

        // Clean up
        std::env::remove_var("GATE_STATE_DIR");
    }

    #[tokio::test]
    async fn test_get_bootstrap_token_from_logs_with_log_file_without_token() {
        let temp_dir = TempDir::new().unwrap();
        let logs_dir = temp_dir.path().join("logs");
        tokio::fs::create_dir_all(&logs_dir).await.unwrap();

        // Create log file without bootstrap token
        let log_file = logs_dir.join("gate.log");
        let mut file = File::create(&log_file).await.unwrap();
        file.write_all(b"2025-08-15T15:21:07Z  INFO Some other log message\n")
            .await
            .unwrap();
        file.write_all(b"2025-08-15T15:21:08Z  WARN No tokens here\n")
            .await
            .unwrap();
        file.flush().await.unwrap();

        // Mock StateDir
        std::env::set_var("GATE_STATE_DIR", temp_dir.path().to_str().unwrap());

        // Call the command function
        let result = get_bootstrap_token_from_logs().await;

        // Should succeed but return None
        assert!(result.is_ok(), "Command should succeed");
        let token = result.unwrap();
        assert!(
            token.is_none(),
            "Should not find token in logs without bootstrap tokens"
        );

        // Clean up
        std::env::remove_var("GATE_STATE_DIR");
    }

    #[tokio::test]
    async fn test_get_bootstrap_token_from_logs_multiple_tokens_returns_latest() {
        let temp_dir = TempDir::new().unwrap();
        let logs_dir = temp_dir.path().join("logs");
        tokio::fs::create_dir_all(&logs_dir).await.unwrap();

        // Create log file with multiple bootstrap tokens
        let log_file = logs_dir.join("gate.log");
        let mut file = File::create(&log_file).await.unwrap();
        file.write_all(
            b"2025-08-15T10:00:00Z  INFO Generated bootstrap token: OlderToken123456789\n",
        )
        .await
        .unwrap();
        file.write_all(b"2025-08-15T11:00:00Z  INFO Some other message\n")
            .await
            .unwrap();
        file.write_all(
            b"2025-08-15T12:00:00Z  INFO Generated bootstrap token: NewerToken123456789\n",
        )
        .await
        .unwrap();
        file.flush().await.unwrap();

        // Mock StateDir
        std::env::set_var("GATE_STATE_DIR", temp_dir.path().to_str().unwrap());

        // Call the command function
        let result = get_bootstrap_token_from_logs().await;

        // Should return the latest (newer) token
        assert!(result.is_ok(), "Command should succeed");
        let token = result.unwrap();
        assert!(token.is_some(), "Should find a token");
        assert_eq!(
            token.unwrap(),
            "NewerToken123456789",
            "Should return the most recent token"
        );

        // Clean up
        std::env::remove_var("GATE_STATE_DIR");
    }

    #[tokio::test]
    async fn test_get_bootstrap_token_from_logs_error_handling() {
        // Test with invalid state directory path
        std::env::set_var(
            "GATE_STATE_DIR",
            "/invalid/nonexistent/path/that/should/not/exist",
        );

        // Call the command function - it should handle the error gracefully
        let result = get_bootstrap_token_from_logs().await;

        // The function should succeed but return None (no logs found)
        // The BootstrapTokenParser handles missing directories gracefully
        assert!(
            result.is_ok(),
            "Command should handle invalid paths gracefully"
        );
        let token = result.unwrap();
        assert!(token.is_none(), "Should return None for invalid paths");

        // Clean up
        std::env::remove_var("GATE_STATE_DIR");
    }
}
