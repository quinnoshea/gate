//! Test utilities for connection pool testing

use iroh::endpoint::ConnectionError;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Mock connection for testing
#[derive(Clone, Debug)]
pub struct TestConnection {
    pub id: String,
    pub healthy: Arc<Mutex<bool>>,
    pub stalled: Arc<Mutex<bool>>,
}

impl TestConnection {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            healthy: Arc::new(Mutex::new(true)),
            stalled: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn is_healthy(&self) -> bool {
        *self.healthy.lock().await
    }

    pub async fn set_healthy(&self, healthy: bool) {
        *self.healthy.lock().await = healthy;
    }

    pub async fn stall(&self) {
        *self.stalled.lock().await = true;
    }

    pub async fn unstall(&self) {
        *self.stalled.lock().await = false;
    }

    pub async fn is_stalled(&self) -> bool {
        *self.stalled.lock().await
    }

    /// Simulate a connection reset
    pub async fn reset(&self) {
        self.set_healthy(false).await;
    }
}

// Helper to convert TestConnection to something that looks like an iroh Connection
// Note: This is a mock implementation for testing only
impl TestConnection {
    pub fn close_reason(&self) -> Option<ConnectionError> {
        // In a real test setup, we'd check if healthy
        // For now, return None to indicate connection is open
        None
    }
}
