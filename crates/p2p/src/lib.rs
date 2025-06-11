//! P2P networking for Gate using Iroh

pub mod error;
pub mod session;
pub mod stream;

pub use error::P2PError;
pub use session::{P2PSession, PendingRequest};
pub use stream::P2PStream;

// Re-export core types
pub use hellas_gate_core::{GateAddr, GateId};

pub type Result<T> = std::result::Result<T, P2PError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn test_session_creation() {
        let (session, _request_rx) = P2PSession::builder().start().await.unwrap();
        let node_id = session.node_id();

        // Basic sanity check
        assert_ne!(node_id.to_string(), "");
    }
}
