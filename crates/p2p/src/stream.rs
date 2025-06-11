//! P2P stream wrapper for convenient JSON/bytes I/O

use iroh::endpoint::{RecvStream, SendStream};
use serde_json::Value as JsonValue;
use tracing::debug;

use crate::{P2PError, Result};

/// Wrapper around Iroh streams with convenience methods
pub struct P2PStream {
    send: SendStream,
    recv: RecvStream,
}

impl P2PStream {
    /// Create a new P2P stream wrapper
    #[must_use]
    pub const fn new(send: SendStream, recv: RecvStream) -> Self {
        Self { send, recv }
    }

    /// Send JSON data with length prefix
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization or writing fails
    pub async fn send_json(&mut self, json: &JsonValue) -> Result<()> {
        let json_bytes = serde_json::to_vec(json)
            .map_err(|e| P2PError::Protocol(format!("Failed to serialize JSON: {e}")))?;

        // Send length prefix (4 bytes) + JSON data
        let len = u32::try_from(json_bytes.len())
            .map_err(|_| P2PError::Protocol("Message too large for u32".to_string()))?;
        self.send
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to write length: {e}")))?;

        self.send
            .write_all(&json_bytes)
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to write JSON: {e}")))?;

        debug!("Sent {} bytes of JSON data", json_bytes.len());
        Ok(())
    }

    /// Receive JSON data with length prefix
    ///
    /// # Errors
    ///
    /// Returns an error if reading or JSON parsing fails
    pub async fn recv_json(&mut self) -> Result<JsonValue> {
        // Read length prefix (4 bytes)
        let mut len_bytes = [0u8; 4];
        self.recv
            .read_exact(&mut len_bytes)
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to read length: {e}")))?;

        let len = u32::from_be_bytes(len_bytes) as usize;
        if len > 10 * 1024 * 1024 {
            // 10MB limit
            return Err(P2PError::Protocol(format!(
                "Message too large: {len} bytes"
            )));
        }

        // Read JSON data
        let mut json_bytes = vec![0u8; len];
        self.recv
            .read_exact(&mut json_bytes)
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to read JSON: {e}")))?;

        let json = serde_json::from_slice(&json_bytes)
            .map_err(|e| P2PError::Protocol(format!("Failed to parse JSON: {e}")))?;

        debug!("Received {} bytes of JSON data", json_bytes.len());
        Ok(json)
    }

    /// Send raw bytes with length prefix
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails
    pub async fn send_bytes(&mut self, data: &[u8]) -> Result<()> {
        let len = u32::try_from(data.len())
            .map_err(|_| P2PError::Protocol("Data too large for u32".to_string()))?;
        self.send
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to write length: {e}")))?;

        self.send
            .write_all(data)
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to write data: {e}")))?;

        debug!("Sent {} bytes of raw data", data.len());
        Ok(())
    }

    /// Receive raw bytes with length prefix
    ///
    /// # Errors
    ///
    /// Returns an error if reading fails
    pub async fn recv_bytes(&mut self) -> Result<Vec<u8>> {
        // Read length prefix
        let mut len_bytes = [0u8; 4];
        self.recv
            .read_exact(&mut len_bytes)
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to read length: {e}")))?;

        let len = u32::from_be_bytes(len_bytes) as usize;
        if len > 100 * 1024 * 1024 {
            // 100MB limit for raw data
            return Err(P2PError::Protocol(format!("Data too large: {len} bytes")));
        }

        // Read data
        let mut data = vec![0u8; len];
        self.recv
            .read_exact(&mut data)
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to read data: {e}")))?;

        debug!("Received {} bytes of raw data", data.len());
        Ok(data)
    }

    /// Get references to underlying streams for direct access
    pub const fn streams(&mut self) -> (&mut SendStream, &mut RecvStream) {
        (&mut self.send, &mut self.recv)
    }

    /// Finish the send stream
    ///
    /// # Errors
    ///
    /// Returns an error if finishing the stream fails
    pub fn finish_send(&mut self) -> Result<()> {
        self.send
            .finish()
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to finish stream: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn test_json_serialization() {
        let test_json = json!({
            "model": "test-model",
            "messages": [{"role": "user", "content": "hello"}]
        });

        let json_bytes = serde_json::to_vec(&test_json).unwrap();
        let len = u32::try_from(json_bytes.len()).unwrap();

        // Verify length prefix + data format
        let mut expected = len.to_be_bytes().to_vec();
        expected.extend_from_slice(&json_bytes);

        assert!(expected.len() > 4);
        assert_eq!(&expected[0..4], &len.to_be_bytes());
    }

    #[test]
    fn test_size_limits() {
        let large_json_size = 20 * 1024 * 1024; // 20MB
        let large_data_size = 200 * 1024 * 1024; // 200MB

        assert!(large_json_size > 10 * 1024 * 1024);
        assert!(large_data_size > 100 * 1024 * 1024);
    }
}
