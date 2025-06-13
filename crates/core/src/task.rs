//! Streaming task management types and utilities

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for an async task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl TaskId {
    /// Generate a new random task ID
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a task ID from a UUID
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TaskId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// Events emitted during streaming task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskEvent<T> {
    /// Task has been started
    Started {
        task_id: TaskId,
        message: String,
    },
    /// Progress update during task execution
    Progress {
        task_id: TaskId,
        message: String,
        /// Optional completion percentage (0-100)
        percent: Option<u8>,
    },
    /// Task completed successfully with result
    Completed {
        task_id: TaskId,
        result: T,
    },
    /// Task failed with error
    Failed {
        task_id: TaskId,
        error: crate::error::CoreError,
    },
}

impl<T> TaskEvent<T> {
    /// Get the task ID for this event
    #[must_use]
    pub const fn task_id(&self) -> TaskId {
        match self {
            Self::Started { task_id, .. }
            | Self::Progress { task_id, .. }
            | Self::Completed { task_id, .. }
            | Self::Failed { task_id, .. } => *task_id,
        }
    }

    /// Check if this is a terminal event (completion or failure)
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed { .. } | Self::Failed { .. })
    }

    /// Get a human-readable description of the event
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Self::Started { message, .. } => format!("Started: {message}"),
            Self::Progress { message, percent, .. } => {
                if let Some(p) = percent {
                    format!("Progress ({p}%): {message}")
                } else {
                    format!("Progress: {message}")
                }
            }
            Self::Completed { .. } => "Completed successfully".to_string(),
            Self::Failed { error, .. } => format!("Failed: {error}"),
        }
    }

    /// Create a started event
    #[must_use]
    pub fn started(task_id: TaskId, message: impl Into<String>) -> Self {
        Self::Started {
            task_id,
            message: message.into(),
        }
    }

    /// Create a progress event without percentage
    #[must_use]
    pub fn progress(task_id: TaskId, message: impl Into<String>) -> Self {
        Self::Progress {
            task_id,
            message: message.into(),
            percent: None,
        }
    }

    /// Create a progress event with percentage
    #[must_use]
    pub fn progress_with_percent(
        task_id: TaskId,
        message: impl Into<String>,
        percent: u8,
    ) -> Self {
        Self::Progress {
            task_id,
            message: message.into(),
            percent: Some(percent.min(100)),
        }
    }

    /// Create a completed event
    #[must_use]
    pub fn completed(task_id: TaskId, result: T) -> Self {
        Self::Completed { task_id, result }
    }

    /// Create a failed event
    #[must_use]
    pub fn failed(task_id: TaskId, error: crate::error::CoreError) -> Self {
        Self::Failed { task_id, error }
    }
}

/// Key for ensuring task idempotency
/// 
/// Tasks with the same idempotency key will return the same task ID
/// and stream the same events, preventing duplicate work.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// Create a new idempotency key from operation name and parameters
    #[must_use]
    pub fn new(operation: &str, params: &[&str]) -> Self {
        let combined = format!("{}:{}", operation, params.join(":"));
        Self(combined)
    }

    /// Create an idempotency key from a raw string
    #[must_use]
    pub fn from_string(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Get the key as a string
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IdempotencyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for IdempotencyKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for IdempotencyKey {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Status of a task at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus<T> {
    /// Task is queued but not yet started
    Pending,
    /// Task is currently running
    Running {
        /// Last progress message, if any
        last_message: Option<String>,
        /// Last progress percentage, if any
        last_percent: Option<u8>,
    },
    /// Task completed successfully
    Completed(T),
    /// Task failed with error
    Failed(crate::error::CoreError),
}

impl<T> TaskStatus<T> {
    /// Check if the task is still active (not completed or failed)
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Pending | Self::Running { .. })
    }

    /// Check if the task is terminal (completed or failed)
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        !self.is_active()
    }

    /// Get a human-readable description of the status
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Self::Pending => "Pending".to_string(),
            Self::Running {
                last_message,
                last_percent,
            } => {
                let mut desc = "Running".to_string();
                if let Some(percent) = last_percent {
                    desc.push_str(&format!(" ({percent}%)"));
                }
                if let Some(message) = last_message {
                    desc.push_str(&format!(": {message}"));
                }
                desc
            }
            Self::Completed(_) => "Completed".to_string(),
            Self::Failed(error) => format!("Failed: {error}"),
        }
    }

    /// Update status from a task event
    #[must_use]
    pub fn update_from_event(self, event: &TaskEvent<T>) -> Self
    where
        T: Clone,
    {
        match event {
            TaskEvent::Started { .. } => Self::Running {
                last_message: None,
                last_percent: None,
            },
            TaskEvent::Progress { message, percent, .. } => Self::Running {
                last_message: Some(message.clone()),
                last_percent: *percent,
            },
            TaskEvent::Completed { result, .. } => Self::Completed(result.clone()),
            TaskEvent::Failed { error, .. } => Self::Failed(error.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_id_generation() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();
        assert_ne!(id1, id2);
        
        let uuid = Uuid::new_v4();
        let id3 = TaskId::from_uuid(uuid);
        assert_eq!(id3.uuid(), uuid);
    }

    #[test]
    fn test_task_id_string_conversion() {
        let id = TaskId::new();
        let id_str = id.to_string();
        let parsed_id: TaskId = id_str.parse().unwrap();
        assert_eq!(id, parsed_id);
    }

    #[test]
    fn test_idempotency_key() {
        let key1 = IdempotencyKey::new("create_dns_challenge", &["example.com", "txt_value"]);
        let key2 = IdempotencyKey::new("create_dns_challenge", &["example.com", "txt_value"]);
        let key3 = IdempotencyKey::new("create_dns_challenge", &["other.com", "txt_value"]);
        
        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_task_event_creation() {
        let task_id = TaskId::new();
        
        let started = TaskEvent::<()>::started(task_id, "Starting task");
        assert_eq!(started.task_id(), task_id);
        assert!(!started.is_terminal());
        
        let progress = TaskEvent::<()>::progress_with_percent(task_id, "Working", 50);
        assert_eq!(progress.task_id(), task_id);
        assert!(!progress.is_terminal());
        
        let completed = TaskEvent::completed(task_id, "result");
        assert_eq!(completed.task_id(), task_id);
        assert!(completed.is_terminal());
    }

    #[test]
    fn test_task_status_updates() {
        let task_id = TaskId::new();
        let mut status = TaskStatus::<String>::Pending;
        
        assert!(status.is_active());
        assert!(!status.is_terminal());
        
        let started_event = TaskEvent::started(task_id, "Starting");
        status = status.update_from_event(&started_event);
        assert!(matches!(status, TaskStatus::Running { .. }));
        
        let progress_event = TaskEvent::progress_with_percent(task_id, "Working", 75);
        status = status.update_from_event(&progress_event);
        if let TaskStatus::Running { last_percent, .. } = &status {
            assert_eq!(*last_percent, Some(75));
        } else {
            panic!("Expected Running status");
        }
        
        let completed_event = TaskEvent::completed(task_id, "done".to_string());
        status = status.update_from_event(&completed_event);
        assert!(status.is_terminal());
        assert!(matches!(status, TaskStatus::Completed(_)));
    }
}