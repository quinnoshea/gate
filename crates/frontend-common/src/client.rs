//! Client configuration and initialization

use gate_http::client::{error::ClientError, GateClient, GateClientBuilder};
use once_cell::sync::Lazy;
use std::sync::Mutex;
use web_sys::window;

/// Global client instance
static CLIENT: Lazy<Mutex<Option<GateClient>>> = Lazy::new(|| Mutex::new(None));

/// Get the base URL for API calls
fn get_base_url() -> String {
    // Try to get from window location
    if let Some(window) = window() {
        if let Ok(location) = window.location().origin() {
            return location;
        }
    }

    // Default to relative URLs
    String::new()
}

/// Get the client instance
pub fn create_client() -> Result<GateClient, ClientError> {
    let mut client_lock = CLIENT.lock().expect("Failed to acquire client lock");

    if client_lock.is_none() {
        // Initialize on first use
        let client = GateClientBuilder::default()
            .base_url(get_base_url())
            .build()?;
        *client_lock = Some(client.clone());
        Ok(client)
    } else {
        Ok(client_lock
            .as_ref()
            .expect("Client should be initialized")
            .clone())
    }
}

/// Update the client with an authentication token
pub fn set_auth_token(token: Option<&str>) -> Result<(), ClientError> {
    let mut builder = GateClientBuilder::default().base_url(get_base_url());

    if let Some(token) = token {
        builder = builder.api_key(token);
    }

    let client = builder.build()?;
    *CLIENT.lock().expect("Failed to acquire client lock") = Some(client);
    Ok(())
}
