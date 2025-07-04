//! Configuration validation support

use config::ConfigError;
use serde::{Deserialize, Serialize};

/// Trait for validating configuration values
pub trait ValidateConfig: Serialize + for<'de> Deserialize<'de> {
    /// Validate the configuration
    ///
    /// Returns Ok(()) if valid, or an error describing what's wrong
    fn validate(&self) -> Result<(), ConfigError>;

    /// Validate a partial update at a specific path
    ///
    /// This is called when updating a nested value to ensure the update
    /// doesn't break invariants. The default implementation deserializes
    /// to the full type and validates that.
    fn validate_path(&self, path: &str, value: &serde_json::Value) -> Result<(), ConfigError> {
        // Default: clone current config, apply update, validate full config
        let mut current_value = serde_json::to_value(self)
            .map_err(|e| ConfigError::Message(format!("Failed to serialize config: {e}")))?;

        // Apply the update to the cloned value
        if path.is_empty() {
            current_value = value.clone();
        } else {
            apply_path_update(&mut current_value, path, value.clone())?;
        }

        // Deserialize and validate
        let updated: Self = serde_json::from_value(current_value)
            .map_err(|e| ConfigError::Message(format!("Invalid config after update: {e}")))?;

        updated.validate()
    }
}

/// Apply an update to a specific path in a JSON value
fn apply_path_update(
    target: &mut serde_json::Value,
    path: &str,
    value: serde_json::Value,
) -> Result<(), ConfigError> {
    let segments: Vec<&str> = path.split('.').collect();
    let mut current = target;

    for (i, segment) in segments.iter().enumerate() {
        if i == segments.len() - 1 {
            // Last segment - update the value
            if let Some(obj) = current.as_object_mut() {
                obj.insert(segment.to_string(), value);
                return Ok(());
            } else {
                return Err(ConfigError::Message(format!(
                    "Cannot set '{}' on non-object at path '{}'",
                    segment,
                    segments[..i].join(".")
                )));
            }
        } else {
            // Navigate deeper
            current = current
                .as_object_mut()
                .and_then(|obj| obj.get_mut(*segment))
                .ok_or_else(|| {
                    ConfigError::Message(format!("Path '{}' not found", segments[..=i].join(".")))
                })?;
        }
    }

    Ok(())
}

/// Common validation helpers
pub mod validators {
    use config::ConfigError;

    /// Validate that a port number is valid (1-65535)
    pub fn validate_port(port: u16, field: &str) -> Result<(), ConfigError> {
        if port == 0 {
            return Err(ConfigError::Message(format!(
                "{field}: port must be between 1 and 65535"
            )));
        }
        Ok(())
    }

    /// Validate that a string is not empty
    pub fn validate_not_empty(value: &str, field: &str) -> Result<(), ConfigError> {
        if value.trim().is_empty() {
            return Err(ConfigError::Message(format!("{field}: cannot be empty")));
        }
        Ok(())
    }

    /// Validate URL format
    pub fn validate_url(url: &str, field: &str) -> Result<(), ConfigError> {
        url::Url::parse(url)
            .map_err(|e| ConfigError::Message(format!("{field}: invalid URL - {e}")))?;
        Ok(())
    }

    /// Validate email format (basic check)
    pub fn validate_email(email: &str, field: &str) -> Result<(), ConfigError> {
        if !email.contains('@') || email.split('@').count() != 2 {
            return Err(ConfigError::Message(format!(
                "{field}: invalid email format"
            )));
        }
        Ok(())
    }

    /// Validate that a value is within range
    pub fn validate_range<T: PartialOrd + std::fmt::Display>(
        value: T,
        min: T,
        max: T,
        field: &str,
    ) -> Result<(), ConfigError> {
        if value < min || value > max {
            return Err(ConfigError::Message(format!(
                "{field}: must be between {min} and {max}"
            )));
        }
        Ok(())
    }
}
