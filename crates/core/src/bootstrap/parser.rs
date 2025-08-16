//! Bootstrap token parser for extracting tokens from log files
//!
//! This module provides functionality to parse bootstrap tokens from gate daemon
//! log files, enabling automated bootstrap token discovery instead of manual entry.

use anyhow::{Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

/// Bootstrap token parser that searches through log files for bootstrap tokens
///
/// The parser uses multiple strategies to find the most recent bootstrap token:
/// 1. Parse recent log files for token generation messages
/// 2. Check multiple log file formats (current, rotated)
/// 3. Handle different token patterns from various log sources
pub struct BootstrapTokenParser {
    log_dir: PathBuf,
    #[cfg(test)]
    pub token_pattern: Regex,
    #[cfg(not(test))]
    token_pattern: Regex,
}

impl BootstrapTokenParser {
    /// Create a new bootstrap token parser
    ///
    /// # Arguments
    ///
    /// * `log_dir` - Directory containing log files to search
    ///
    /// # Returns
    ///
    /// * `Result<Self>` - New parser instance or error
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let state_dir = StateDir::new();
    /// let parser = BootstrapTokenParser::new(state_dir.data_dir().join("logs"))?;
    /// ```
    pub fn new(log_dir: PathBuf) -> Result<Self> {
        Ok(Self {
            log_dir,
            // Pattern to match various bootstrap token formats in logs
            // Matches: "Generated bootstrap token: <token>" and similar variants
            token_pattern: Regex::new(
                r"(?i)(?:generated\s+bootstrap\s+token|bootstrap\s+token|token)\s*:\s*([a-zA-Z0-9_\-]{15,})",
            )?,
        })
    }

    /// Find the most recent bootstrap token from log files
    ///
    /// This method searches through log files in the specified directory,
    /// looking for bootstrap token generation messages. It returns the most
    /// recently generated token found.
    ///
    /// # Returns
    ///
    /// * `Result<Option<String>>` - Most recent token if found, None if no tokens found
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let parser = BootstrapTokenParser::new(logs_dir)?;
    /// if let Some(token) = parser.find_latest_token().await? {
    ///     println!("Found bootstrap token: {}", token);
    /// }
    /// ```
    pub async fn find_latest_token(&self) -> Result<Option<String>> {
        // Get all log files sorted by modification time
        let mut log_files = self.get_log_files().await?;

        if log_files.is_empty() {
            return Ok(None);
        }

        // Sort by modification time, newest first
        log_files.sort_by_key(|entry| {
            std::cmp::Reverse(
                entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH),
            )
        });

        // Search through recent log files for tokens
        for log_file in log_files.iter().take(5) {
            // Only check 5 most recent files
            if let Some(token) = self.extract_token_from_file(&log_file.path()).await? {
                return Ok(Some(token));
            }
        }

        Ok(None)
    }

    /// Get all log files from the log directory
    async fn get_log_files(&self) -> Result<Vec<std::fs::DirEntry>> {
        let mut files = Vec::new();

        if !self.log_dir.exists() {
            return Ok(files);
        }

        // Use sync std::fs since we need DirEntry for metadata
        let entries = std::fs::read_dir(&self.log_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Check if this looks like a gate log file
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str())
                && file_name.starts_with("gate") && file_name.ends_with(".log") {
                files.push(entry);
            }
        }

        Ok(files)
    }

    /// Extract the most recent bootstrap token from a specific log file
    async fn extract_token_from_file(&self, path: &Path) -> Result<Option<String>> {
        let file = File::open(path)
            .await
            .with_context(|| format!("Failed to open log file: {}", path.display()))?;

        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // We want the most recent token, so we'll collect all matches and return the last one
        let mut found_tokens = Vec::new();

        while let Some(line) = lines.next_line().await? {
            // Check for bootstrap token patterns
            if let Some(captures) = self.token_pattern.captures(&line) {
                // Extract the token from the first (and only) capture group
                if let Some(token_match) = captures.get(1) {
                    let token = token_match.as_str().trim().to_string();
                    if !token.is_empty() && token.len() >= 15 {
                        found_tokens.push((token, line.clone()));
                    }
                }
            }
        }

        // Return the last (most recent) token found in this file
        if let Some((token, _line)) = found_tokens.last() {
            Ok(Some(token.clone()))
        } else {
            Ok(None)
        }
    }

    /// Find all bootstrap tokens with their context lines (for debugging/testing)
    pub async fn find_all_tokens_with_context(&self) -> Result<Vec<(String, String, PathBuf)>> {
        let mut results = Vec::new();
        let log_files = self.get_log_files().await?;

        for log_file in log_files {
            let path = log_file.path();
            if let Ok(file) = File::open(&path).await {
                let reader = BufReader::new(file);
                let mut lines = reader.lines();

                while let Some(line) = lines.next_line().await? {
                    if let Some(captures) = self.token_pattern.captures(&line)
                        && let Some(token_match) = captures.get(1) {
                        let token = token_match.as_str().trim().to_string();
                        if !token.is_empty() && token.len() >= 15 {
                            results.push((token, line.clone(), path.clone()));
                        }
                    }
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn test_bootstrap_token_parser_creation() {
        let temp_dir = TempDir::new().unwrap();
        let parser = BootstrapTokenParser::new(temp_dir.path().to_path_buf()).unwrap();

        assert_eq!(parser.log_dir, temp_dir.path());
    }

    #[tokio::test]
    async fn test_find_token_in_log_file() {
        let temp_dir = TempDir::new().unwrap();
        let log_file_path = temp_dir.path().join("gate.log");

        // Create a test log file with a bootstrap token
        let mut file = tokio::fs::File::create(&log_file_path).await.unwrap();
        file.write_all(b"2025-08-15T15:21:07.988194Z  INFO main ThreadId(01) gate_daemon::runtime::inner: crates/daemon/src/runtime/inner.rs:69: Generated bootstrap token: TestToken123456789ABC\n").await.unwrap();
        file.flush().await.unwrap();
        drop(file);

        let parser = BootstrapTokenParser::new(temp_dir.path().to_path_buf()).unwrap();
        let token = parser.find_latest_token().await.unwrap();

        assert_eq!(token, Some("TestToken123456789ABC".to_string()));
    }

    #[tokio::test]
    async fn test_find_latest_token_multiple_files() {
        let temp_dir = TempDir::new().unwrap();

        // Create older log file
        let old_log = temp_dir.path().join("gate.1.log");
        let mut file = tokio::fs::File::create(&old_log).await.unwrap();
        file.write_all(
            b"2025-08-14T10:00:00Z  INFO Generated bootstrap token: OlderToken123456789\n",
        )
        .await
        .unwrap();
        drop(file);

        // Wait a moment to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Create newer log file
        let new_log = temp_dir.path().join("gate.log");
        let mut file = tokio::fs::File::create(&new_log).await.unwrap();
        file.write_all(
            b"2025-08-15T15:21:07Z  INFO Generated bootstrap token: NewerToken123456789\n",
        )
        .await
        .unwrap();
        drop(file);

        let parser = BootstrapTokenParser::new(temp_dir.path().to_path_buf()).unwrap();
        let token = parser.find_latest_token().await.unwrap();

        // Should find the token from the newer file
        assert_eq!(token, Some("NewerToken123456789".to_string()));
    }

    #[tokio::test]
    async fn test_no_tokens_found() {
        let temp_dir = TempDir::new().unwrap();
        let log_file_path = temp_dir.path().join("gate.log");

        // Create a log file without bootstrap tokens
        let mut file = tokio::fs::File::create(&log_file_path).await.unwrap();
        file.write_all(b"2025-08-15T15:21:07Z  INFO Some other log message\n2025-08-15T15:21:08Z  WARN No tokens here\n").await.unwrap();
        drop(file);

        let parser = BootstrapTokenParser::new(temp_dir.path().to_path_buf()).unwrap();
        let token = parser.find_latest_token().await.unwrap();

        assert_eq!(token, None);
    }

    #[tokio::test]
    async fn test_find_all_tokens_with_context() {
        let temp_dir = TempDir::new().unwrap();
        let log_file_path = temp_dir.path().join("gate.log");

        // Create a log file with multiple bootstrap tokens
        let mut file = tokio::fs::File::create(&log_file_path).await.unwrap();
        file.write_all(
            b"2025-08-15T10:00:00Z  INFO Generated bootstrap token: Token1234567890ABC\n",
        )
        .await
        .unwrap();
        file.write_all(b"2025-08-15T11:00:00Z  INFO Some other message\n")
            .await
            .unwrap();
        file.write_all(
            b"2025-08-15T12:00:00Z  INFO Generated bootstrap token: Token2345678901DEF\n",
        )
        .await
        .unwrap();
        drop(file);

        let parser = BootstrapTokenParser::new(temp_dir.path().to_path_buf()).unwrap();
        let results = parser.find_all_tokens_with_context().await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "Token1234567890ABC");
        assert_eq!(results[1].0, "Token2345678901DEF");
        assert!(results[0].1.contains("Generated bootstrap token"));
        assert!(results[1].1.contains("Generated bootstrap token"));
    }

    #[tokio::test]
    async fn test_empty_log_directory() {
        let temp_dir = TempDir::new().unwrap();
        let parser = BootstrapTokenParser::new(temp_dir.path().to_path_buf()).unwrap();

        let token = parser.find_latest_token().await.unwrap();
        assert_eq!(token, None);
    }

    #[tokio::test]
    async fn test_regex_pattern_variations() {
        let temp_dir = TempDir::new().unwrap();
        let log_file_path = temp_dir.path().join("gate.log");

        // Test different token format variations
        let mut file = tokio::fs::File::create(&log_file_path).await.unwrap();
        file.write_all(b"Generated bootstrap token: VariationA123456789\n")
            .await
            .unwrap();
        file.write_all(b"Bootstrap token: VariationB123456789\n")
            .await
            .unwrap();
        file.write_all(b"TOKEN: VariationC123456789\n")
            .await
            .unwrap();
        drop(file);

        let parser = BootstrapTokenParser::new(temp_dir.path().to_path_buf()).unwrap();
        let results = parser.find_all_tokens_with_context().await.unwrap();

        // Should find tokens in different formats
        assert!(results.len() >= 3, "Should find all three token variants");
        assert_eq!(results[0].0, "VariationA123456789");
        assert_eq!(results[1].0, "VariationB123456789");
        assert_eq!(results[2].0, "VariationC123456789");

        // The latest should be available
        let token = parser.find_latest_token().await.unwrap();
        assert!(token.is_some(), "Should find at least one token");
        assert_eq!(token, Some("VariationC123456789".to_string())); // Last token in file
    }
}
