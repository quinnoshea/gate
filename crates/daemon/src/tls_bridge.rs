//! Bridge between P2P TLS streams and local HTTP server

use crate::http::HttpServer;
use crate::tls::TlsHandler;
use crate::{DaemonError, Result};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info, warn};

/// Bridge that processes TLS data from P2P streams and forwards to HTTP server
pub struct TlsBridge {
    tls_handler: Arc<TlsHandler>,
    http_client: reqwest::Client,
    local_http_addr: std::net::SocketAddr,
}

impl TlsBridge {
    /// Create a new TLS bridge
    ///
    /// # Errors
    ///
    /// Returns an error if bridge initialization fails
    pub fn new(
        tls_handler: Arc<TlsHandler>,
        local_http_addr: std::net::SocketAddr,
    ) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| DaemonError::Http(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            tls_handler,
            http_client,
            local_http_addr,
        })
    }

    /// Process raw HTTPS bytes from P2P stream and return HTTP response
    ///
    /// This function:
    /// 1. Terminates TLS to extract the HTTP request
    /// 2. Forwards the request to the local HTTP server
    /// 3. Returns the HTTP response bytes
    ///
    /// # Errors
    ///
    /// Returns an error if TLS termination or HTTP forwarding fails
    pub async fn process_https_bytes(&self, https_bytes: &[u8]) -> Result<Vec<u8>> {
        debug!("Processing {} bytes of HTTPS data", https_bytes.len());

        // Step 1: Terminate TLS to get the HTTP request
        let http_request = self
            .tls_handler
            .terminate_tls(https_bytes)
            .await
            .map_err(|e| DaemonError::Other(anyhow::anyhow!("TLS termination failed: {e}")))?;

        debug!("Extracted HTTP request:\n{}", http_request);

        // Step 2: Parse the HTTP request to extract method, path, headers, and body
        let (method, path, headers, body) = self.parse_http_request(&http_request)?;

        // Step 3: Forward to local HTTP server
        let response_bytes = self
            .forward_to_local_server(method, path, headers, body)
            .await?;

        info!(
            "Successfully processed HTTPS request, returning {} bytes",
            response_bytes.len()
        );
        Ok(response_bytes)
    }

    /// Parse HTTP request string into components
    fn parse_http_request(
        &self,
        request: &str,
    ) -> Result<(String, String, Vec<(String, String)>, String)> {
        let lines: Vec<&str> = request.split("\r\n").collect();

        if lines.is_empty() {
            return Err(DaemonError::Http("Empty HTTP request".to_string()));
        }

        // Parse request line (e.g., "GET /path HTTP/1.1")
        let request_line_parts: Vec<&str> = lines[0].split_whitespace().collect();
        if request_line_parts.len() < 2 {
            return Err(DaemonError::Http("Invalid HTTP request line".to_string()));
        }

        let method = request_line_parts[0].to_string();
        let path = request_line_parts[1].to_string();

        // Parse headers
        let mut headers = Vec::new();
        let mut header_end_index = 1;

        for (i, line) in lines.iter().enumerate().skip(1) {
            if line.is_empty() {
                header_end_index = i;
                break;
            }

            if let Some(colon_pos) = line.find(':') {
                let name = line[..colon_pos].trim().to_string();
                let value = line[colon_pos + 1..].trim().to_string();
                headers.push((name, value));
            }
        }

        // Extract body (everything after the empty line)
        let body = if header_end_index + 1 < lines.len() {
            lines[header_end_index + 1..].join("\r\n")
        } else {
            String::new()
        };

        Ok((method, path, headers, body))
    }

    /// Forward the HTTP request to the local server
    async fn forward_to_local_server(
        &self,
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: String,
    ) -> Result<Vec<u8>> {
        let url = format!("http://{}{}", self.local_http_addr, path);
        debug!("Forwarding {} {} to local server", method, url);

        // Build the HTTP request
        let mut request_builder = match method.as_str() {
            "GET" => self.http_client.get(&url),
            "POST" => self.http_client.post(&url),
            "PUT" => self.http_client.put(&url),
            "DELETE" => self.http_client.delete(&url),
            "PATCH" => self.http_client.patch(&url),
            "HEAD" => self.http_client.head(&url),
            _ => {
                return Err(DaemonError::Http(format!(
                    "Unsupported HTTP method: {method}"
                )));
            }
        };

        // Add headers
        for (name, value) in headers {
            // Skip headers that reqwest handles automatically
            if !["host", "content-length", "connection"].contains(&name.to_lowercase().as_str()) {
                request_builder = request_builder.header(&name, &value);
            }
        }

        // Add body if present
        if !body.is_empty() {
            request_builder = request_builder.body(body);
        }

        // Send the request
        let response = request_builder
            .send()
            .await
            .map_err(|e| DaemonError::Http(format!("HTTP request failed: {e}")))?;

        // Build HTTP response
        let status_code = response.status().as_u16();
        let status_text = response.status().canonical_reason().unwrap_or("Unknown");

        let mut response_lines = vec![format!("HTTP/1.1 {} {}", status_code, status_text)];

        // Add response headers
        for (name, value) in response.headers() {
            response_lines.push(format!(
                "{}: {}",
                name.as_str(),
                value.to_str().unwrap_or("")
            ));
        }

        // Get response body
        let response_body = response
            .bytes()
            .await
            .map_err(|e| DaemonError::Http(format!("Failed to read response body: {e}")))?;

        // Add Content-Length header if not present
        if !response_lines
            .iter()
            .any(|line| line.to_lowercase().starts_with("content-length:"))
        {
            response_lines.push(format!("Content-Length: {}", response_body.len()));
        }

        // Construct complete HTTP response
        response_lines.push(String::new()); // Empty line before body
        let mut response_bytes = response_lines.join("\r\n").into_bytes();
        response_bytes.extend_from_slice(&response_body);

        debug!("Local server responded with {} bytes", response_bytes.len());
        Ok(response_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_http_request() {
        let bridge = TlsBridge {
            tls_handler: Arc::new(crate::tls::TlsHandler::new("test", b"test_key").unwrap()),
            http_client: reqwest::Client::new(),
            local_http_addr: "127.0.0.1:8080".parse().unwrap(),
        };

        let request =
            "GET /test HTTP/1.1\r\nHost: example.com\r\nUser-Agent: test\r\n\r\ntest body";
        let (method, path, headers, body) = bridge.parse_http_request(request).unwrap();

        assert_eq!(method, "GET");
        assert_eq!(path, "/test");
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0], ("Host".to_string(), "example.com".to_string()));
        assert_eq!(headers[1], ("User-Agent".to_string(), "test".to_string()));
        assert_eq!(body, "test body");
    }
}
