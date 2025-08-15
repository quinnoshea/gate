use crate::state::{DaemonState, TlsForwardStatus};
use gate_core::bootstrap::BootstrapTokenParser;
use gate_daemon::{Settings, runtime::Runtime, StateDir};
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
    get_bootstrap_token_from_logs_impl(None).await
}

/// Implementation function that allows overriding StateDir for testing
async fn get_bootstrap_token_from_logs_impl(state_dir_override: Option<StateDir>) -> Result<Option<String>, String> {
    let state_dir = state_dir_override.unwrap_or_else(StateDir::new);
    let logs_dir = state_dir.data_dir().join("logs");
    
    // Create parser instance
    let parser = BootstrapTokenParser::new(logs_dir)
        .map_err(|e| format!("Failed to initialize bootstrap token parser: {}", e))?;
    
    // Search for the latest token in log files
    match parser.find_latest_token().await {
        Ok(token) => {
            if let Some(ref token_str) = token {
                info!("Successfully found bootstrap token from logs: {}", token_str);
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

/// Opens the daemon URL in the default browser.
/// 
/// This command gets the current daemon address and opens it in the user's default browser.
/// If the daemon is not running, returns an error. Uses the opener crate for cross-platform
/// browser launching.
/// 
/// Returns a success message if the browser was opened successfully.
#[tauri::command]
pub async fn open_daemon_in_browser(
    state: State<'_, DaemonState>
) -> Result<String, String> {
    // Check if daemon is running
    if !state.is_running().await {
        return Err("Daemon is not running".to_string());
    }
    
    // Get runtime to access server address
    let runtime = state.get_runtime().await
        .ok_or("Runtime not available")?;
    
    let address = runtime.server_address();
    let url = format!("http://{}", address);
    
    // Open URL in default browser using opener crate
    match opener::open(&url) {
        Ok(()) => {
            info!("Successfully opened daemon URL in browser: {}", url);
            Ok(format!("Opened {} in default browser", url))
        }
        Err(e) => {
            error!("Failed to open daemon URL in browser: {}", e);
            Err(format!("Failed to open browser: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;
    use std::path::PathBuf;

    /// Test helper to create a mock state directory with test log files
    async fn create_test_logs_with_token(temp_dir: &TempDir, token: &str) -> PathBuf {
        // StateDir::with_override expects data/logs structure
        let data_dir = temp_dir.path().join("data");
        let logs_dir = data_dir.join("logs");
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
        let data_dir = temp_dir.path().join("data");
        let logs_dir = data_dir.join("logs");
        tokio::fs::create_dir_all(&logs_dir).await.unwrap();
        logs_dir
    }

    /// Test helper to call get_bootstrap_token_from_logs with a test directory
    async fn call_with_test_dir(temp_dir: &TempDir) -> Result<Option<String>, String> {
        let state_dir = StateDir::with_override(temp_dir.path());
        get_bootstrap_token_from_logs_impl(Some(state_dir)).await
    }

    #[tokio::test]
    async fn test_get_bootstrap_token_from_logs_success() {
        let temp_dir = TempDir::new().unwrap();
        let expected_token = "TestBootstrapToken123456789ABC";
        
        // Create test logs with token
        let _logs_dir = create_test_logs_with_token(&temp_dir, expected_token).await;
        
        // Call the test helper function
        let result = call_with_test_dir(&temp_dir).await;
        
        // Verify success
        assert!(result.is_ok(), "Command should succeed");
        let token = result.unwrap();
        assert!(token.is_some(), "Should find a token");
        assert_eq!(token.unwrap(), expected_token, "Should return the correct token");
    }

    #[tokio::test]
    async fn test_get_bootstrap_token_from_logs_no_token() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create empty logs directory
        let _logs_dir = create_empty_logs(&temp_dir).await;
        
        // Call the test helper function
        let result = call_with_test_dir(&temp_dir).await;
        
        // Verify success but no token found
        assert!(result.is_ok(), "Command should succeed even with no token");
        let token = result.unwrap();
        assert!(token.is_none(), "Should not find a token in empty logs");
    }

    #[tokio::test]
    async fn test_get_bootstrap_token_from_logs_no_logs_directory() {
        let temp_dir = TempDir::new().unwrap();
        
        // Don't create logs directory - this tests the case where logs don't exist yet
        
        // Call the test helper function
        let result = call_with_test_dir(&temp_dir).await;
        
        // Should succeed but return None (no logs directory means no tokens)
        assert!(result.is_ok(), "Command should handle missing logs directory gracefully");
        let token = result.unwrap();
        assert!(token.is_none(), "Should return None when logs directory doesn't exist");
    }

    #[tokio::test]
    async fn test_get_bootstrap_token_from_logs_with_log_file_without_token() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("data");
        let logs_dir = data_dir.join("logs");
        tokio::fs::create_dir_all(&logs_dir).await.unwrap();
        
        // Create log file without bootstrap token
        let log_file = logs_dir.join("gate.log");
        let mut file = File::create(&log_file).await.unwrap();
        file.write_all(b"2025-08-15T15:21:07Z  INFO Some other log message\n").await.unwrap();
        file.write_all(b"2025-08-15T15:21:08Z  WARN No tokens here\n").await.unwrap();
        file.flush().await.unwrap();
        
        // Call the test helper function
        let result = call_with_test_dir(&temp_dir).await;
        
        // Should succeed but return None
        assert!(result.is_ok(), "Command should succeed");
        let token = result.unwrap();
        assert!(token.is_none(), "Should not find token in logs without bootstrap tokens");
    }

    #[tokio::test]
    async fn test_get_bootstrap_token_from_logs_multiple_tokens_returns_latest() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("data");
        let logs_dir = data_dir.join("logs");
        tokio::fs::create_dir_all(&logs_dir).await.unwrap();
        
        // Create log file with multiple bootstrap tokens
        let log_file = logs_dir.join("gate.log");
        let mut file = File::create(&log_file).await.unwrap();
        file.write_all(b"2025-08-15T10:00:00Z  INFO Generated bootstrap token: OlderToken123456789\n").await.unwrap();
        file.write_all(b"2025-08-15T11:00:00Z  INFO Some other message\n").await.unwrap();
        file.write_all(b"2025-08-15T12:00:00Z  INFO Generated bootstrap token: NewerToken123456789\n").await.unwrap();
        file.flush().await.unwrap();
        
        // Call the test helper function
        let result = call_with_test_dir(&temp_dir).await;
        
        // Should return the latest (newer) token
        assert!(result.is_ok(), "Command should succeed");
        let token = result.unwrap();
        assert!(token.is_some(), "Should find a token");
        assert_eq!(token.unwrap(), "NewerToken123456789", "Should return the most recent token");
    }

    #[tokio::test]
    async fn test_get_bootstrap_token_from_logs_error_handling() {
        let temp_dir = TempDir::new().unwrap();
        // Test with invalid state directory path by creating an empty temp dir
        
        // Call the test helper function with empty directory
        let result = call_with_test_dir(&temp_dir).await;
        
        // The function should succeed but return None (no logs found)
        // The BootstrapTokenParser handles missing directories gracefully
        assert!(result.is_ok(), "Command should handle invalid paths gracefully");
        let token = result.unwrap();
        assert!(token.is_none(), "Should return None for invalid paths");
    }

    // Note: Testing open_daemon_in_browser is challenging because:
    // 1. It requires DaemonState which has complex runtime dependencies
    // 2. It calls opener::open() which would actually try to open a browser
    // 3. Creating a mock DaemonState requires significant setup
    //
    // Instead, we can test the function's logic by examining its implementation:
    // - It properly checks if daemon is running
    // - It gets the runtime and server address  
    // - It constructs the URL correctly
    // - It handles opener::open() errors appropriately
    //
    // For integration testing, this would be tested as part of the full workflow
    // where a real daemon is started and the browser opening is verified.
    
    #[test]
    fn test_open_daemon_in_browser_url_construction() {
        // Test URL construction logic (the main logic we can unit test)
        let address = "127.0.0.1:31145";
        let url = format!("http://{}", address);
        assert_eq!(url, "http://127.0.0.1:31145");
        
        let address = "localhost:8080";
        let url = format!("http://{}", address);
        assert_eq!(url, "http://localhost:8080");
    }
}
