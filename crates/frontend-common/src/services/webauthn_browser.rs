//! WebAuthn browser API wrapper service

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde_json::Value as JsonValue;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    CredentialCreationOptions, CredentialRequestOptions, PublicKeyCredential,
    PublicKeyCredentialCreationOptions, PublicKeyCredentialRequestOptions,
};
/// WebAuthn browser service for credential operations
#[derive(Clone)]
pub struct WebAuthnBrowserService;

/// Error type for WebAuthn browser operations
#[derive(Debug, Clone)]
pub enum WebAuthnBrowserError {
    NotSupported,
    CredentialError(String),
    ConversionError(String),
}

impl std::fmt::Display for WebAuthnBrowserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebAuthnBrowserError::NotSupported => write!(f, "WebAuthn is not supported"),
            WebAuthnBrowserError::CredentialError(msg) => write!(f, "Credential error: {msg}"),
            WebAuthnBrowserError::ConversionError(msg) => write!(f, "Conversion error: {msg}"),
        }
    }
}

impl WebAuthnBrowserService {
    /// Create a new WebAuthn browser service
    pub fn new() -> Self {
        Self
    }
}

impl Default for WebAuthnBrowserService {
    fn default() -> Self {
        Self::new()
    }
}

impl WebAuthnBrowserService {
    /// Check if WebAuthn is supported
    pub fn is_supported(&self) -> bool {
        let window = web_sys::window().expect("No window object available");
        let navigator = window.navigator();

        // Check if credentials API exists
        js_sys::Reflect::has(&navigator, &JsValue::from_str("credentials")).unwrap_or(false)
    }

    /// Create credential from server challenge
    pub async fn create_credential(
        &self,
        challenge_json: JsonValue,
    ) -> Result<JsonValue, WebAuthnBrowserError> {
        let window = web_sys::window().ok_or(WebAuthnBrowserError::NotSupported)?;
        let navigator = window.navigator();

        // Parse the challenge data
        let options = self.parse_creation_options(challenge_json)?;

        // Create credential
        let credentials = navigator.credentials();

        let promise = credentials.create_with_options(&options).map_err(|_| {
            WebAuthnBrowserError::CredentialError("Failed to create credential".to_string())
        })?;

        let credential = JsFuture::from(promise).await.map_err(|e| {
            WebAuthnBrowserError::CredentialError(format!("Credential creation failed: {e:?}"))
        })?;

        let public_key_credential = credential.dyn_into::<PublicKeyCredential>().map_err(|_| {
            WebAuthnBrowserError::CredentialError("Invalid credential type".to_string())
        })?;

        self.credential_to_json(&public_key_credential)
    }

    /// Get credential for authentication
    pub async fn get_credential(
        &self,
        challenge_json: JsonValue,
    ) -> Result<JsonValue, WebAuthnBrowserError> {
        let window = web_sys::window().ok_or(WebAuthnBrowserError::NotSupported)?;
        let navigator = window.navigator();

        // Parse the challenge data
        let options = self.parse_request_options(challenge_json)?;

        // Get credential
        let credentials = navigator.credentials();

        let promise = credentials.get_with_options(&options).map_err(|_| {
            WebAuthnBrowserError::CredentialError("Failed to get credential".to_string())
        })?;

        let credential = JsFuture::from(promise).await.map_err(|e| {
            WebAuthnBrowserError::CredentialError(format!("Credential get failed: {e:?}"))
        })?;

        let public_key_credential = credential.dyn_into::<PublicKeyCredential>().map_err(|_| {
            WebAuthnBrowserError::CredentialError("Invalid credential type".to_string())
        })?;

        self.credential_to_json(&public_key_credential)
    }

    /// Parse creation options from JSON
    fn parse_creation_options(
        &self,
        json: JsonValue,
    ) -> Result<CredentialCreationOptions, WebAuthnBrowserError> {
        // Convert JSON to JsValue
        let js_str = serde_json::to_string(&json).map_err(|e| {
            WebAuthnBrowserError::ConversionError(format!("Failed to serialize JSON: {e}"))
        })?;
        let js_options = js_sys::JSON::parse(&js_str).map_err(|_| {
            WebAuthnBrowserError::ConversionError("Failed to parse JSON".to_string())
        })?;

        // Get the publicKey object
        let public_key = js_sys::Reflect::get(&js_options, &JsValue::from_str("publicKey"))
            .map_err(|_| {
                WebAuthnBrowserError::ConversionError("Missing publicKey field".to_string())
            })?;

        // Convert challenge from base64 to ArrayBuffer
        let challenge_b64 = js_sys::Reflect::get(&public_key, &JsValue::from_str("challenge"))
            .map_err(|_| WebAuthnBrowserError::ConversionError("Missing challenge".to_string()))?
            .as_string()
            .ok_or_else(|| {
                WebAuthnBrowserError::ConversionError("Challenge is not a string".to_string())
            })?;

        let challenge_buffer = self.base64_to_array_buffer(&challenge_b64)?;
        js_sys::Reflect::set(
            &public_key,
            &JsValue::from_str("challenge"),
            &challenge_buffer,
        )
        .map_err(|_| {
            WebAuthnBrowserError::ConversionError("Failed to set challenge".to_string())
        })?;

        // Check if we should override the RP ID
        // Only override for non-relay domains to preserve server's relay-compatible RP ID
        if let Ok(window) = web_sys::window().ok_or(WebAuthnBrowserError::NotSupported)
            && let Ok(location) = window.location().hostname()
            && !location.ends_with(".private.hellas.ai")
            && !location.ends_with(".public.hellas.ai")
        {
            gloo::console::log!(
                "WebAuthn: Overriding RP ID for non-relay domain:",
                &location
            );

            // Create new rp object with current hostname
            let rp = js_sys::Object::new();
            js_sys::Reflect::set(&rp, &JsValue::from_str("id"), &JsValue::from_str(&location))
                .map_err(|_| {
                    WebAuthnBrowserError::ConversionError("Failed to set rp.id".to_string())
                })?;

            // Use existing name or default
            if let Ok(existing_rp) = js_sys::Reflect::get(&public_key, &JsValue::from_str("rp")) {
                if let Ok(rp_name) = js_sys::Reflect::get(&existing_rp, &JsValue::from_str("name"))
                {
                    js_sys::Reflect::set(&rp, &JsValue::from_str("name"), &rp_name).map_err(
                        |_| {
                            WebAuthnBrowserError::ConversionError(
                                "Failed to set rp.name".to_string(),
                            )
                        },
                    )?;
                }
            } else {
                js_sys::Reflect::set(
                    &rp,
                    &JsValue::from_str("name"),
                    &JsValue::from_str("Gate Self-Hosted"),
                )
                .map_err(|_| {
                    WebAuthnBrowserError::ConversionError("Failed to set rp.name".to_string())
                })?;
            }

            // Override the rp in publicKey
            js_sys::Reflect::set(&public_key, &JsValue::from_str("rp"), &rp).map_err(|_| {
                WebAuthnBrowserError::ConversionError("Failed to override rp".to_string())
            })?;

            gloo::console::log!("WebAuthn: Overridden RP ID to:", &location);
        } else if let Ok(window) = web_sys::window().ok_or(WebAuthnBrowserError::NotSupported)
            && let Ok(location) = window.location().hostname()
        {
            // Log that we're using server's RP ID for relay domains
            gloo::console::log!(
                "WebAuthn: Using server's RP ID for relay domain:",
                &location
            );
        }

        // Convert user.id from base64 to ArrayBuffer
        let user = js_sys::Reflect::get(&public_key, &JsValue::from_str("user"))
            .map_err(|_| WebAuthnBrowserError::ConversionError("Missing user".to_string()))?;

        let user_id_b64 = js_sys::Reflect::get(&user, &JsValue::from_str("id"))
            .map_err(|_| WebAuthnBrowserError::ConversionError("Missing user.id".to_string()))?
            .as_string()
            .ok_or_else(|| {
                WebAuthnBrowserError::ConversionError("User ID is not a string".to_string())
            })?;

        let user_id_buffer = self.base64_to_array_buffer(&user_id_b64)?;
        js_sys::Reflect::set(&user, &JsValue::from_str("id"), &user_id_buffer).map_err(|_| {
            WebAuthnBrowserError::ConversionError("Failed to set user.id".to_string())
        })?;

        // Create CredentialCreationOptions and set the publicKey
        let options = CredentialCreationOptions::new();
        options.set_public_key(&public_key.unchecked_into::<PublicKeyCredentialCreationOptions>());

        Ok(options)
    }

    /// Parse request options from JSON
    fn parse_request_options(
        &self,
        json: JsonValue,
    ) -> Result<CredentialRequestOptions, WebAuthnBrowserError> {
        // Convert JSON to JsValue
        let js_str = serde_json::to_string(&json).map_err(|e| {
            WebAuthnBrowserError::ConversionError(format!("Failed to serialize JSON: {e}"))
        })?;
        let js_options = js_sys::JSON::parse(&js_str).map_err(|_| {
            WebAuthnBrowserError::ConversionError("Failed to parse JSON".to_string())
        })?;

        // Get the publicKey object
        let public_key = js_sys::Reflect::get(&js_options, &JsValue::from_str("publicKey"))
            .map_err(|_| {
                WebAuthnBrowserError::ConversionError("Missing publicKey field".to_string())
            })?;

        // Convert challenge from base64 to ArrayBuffer
        let challenge_b64 = js_sys::Reflect::get(&public_key, &JsValue::from_str("challenge"))
            .map_err(|_| WebAuthnBrowserError::ConversionError("Missing challenge".to_string()))?
            .as_string()
            .ok_or_else(|| {
                WebAuthnBrowserError::ConversionError("Challenge is not a string".to_string())
            })?;

        let challenge_buffer = self.base64_to_array_buffer(&challenge_b64)?;
        js_sys::Reflect::set(
            &public_key,
            &JsValue::from_str("challenge"),
            &challenge_buffer,
        )
        .map_err(|_| {
            WebAuthnBrowserError::ConversionError("Failed to set challenge".to_string())
        })?;

        // Check if we should override the RP ID
        // Only override for non-relay domains to preserve server's relay-compatible RP ID
        if let Ok(window) = web_sys::window().ok_or(WebAuthnBrowserError::NotSupported)
            && let Ok(location) = window.location().hostname()
            && !location.ends_with(".private.hellas.ai")
            && !location.ends_with(".public.hellas.ai")
        {
            gloo::console::log!(
                "WebAuthn: Overriding RP ID for authentication on non-relay domain:",
                &location
            );

            // Override the rpId in publicKey
            js_sys::Reflect::set(
                &public_key,
                &JsValue::from_str("rpId"),
                &JsValue::from_str(&location),
            )
            .map_err(|_| {
                WebAuthnBrowserError::ConversionError("Failed to override rpId".to_string())
            })?;
        } else if let Ok(window) = web_sys::window().ok_or(WebAuthnBrowserError::NotSupported)
            && let Ok(location) = window.location().hostname()
        {
            // Log that we're using server's RP ID for relay domains
            gloo::console::log!(
                "WebAuthn: Using server's RP ID for authentication on relay domain:",
                &location
            );
        }

        // Convert allowCredentials[].id from base64 to ArrayBuffer if present
        if let Ok(allow_credentials) =
            js_sys::Reflect::get(&public_key, &JsValue::from_str("allowCredentials"))
            && let Some(array) = allow_credentials.dyn_ref::<js_sys::Array>()
        {
            for i in 0..array.length() {
                if let Some(cred) = array.get(i).dyn_ref::<js_sys::Object>()
                    && let Ok(id_b64) = js_sys::Reflect::get(cred, &JsValue::from_str("id"))
                    && let Some(id_str) = id_b64.as_string()
                {
                    let id_buffer = self.base64_to_array_buffer(&id_str)?;
                    js_sys::Reflect::set(cred, &JsValue::from_str("id"), &id_buffer).map_err(
                        |_| {
                            WebAuthnBrowserError::ConversionError(
                                "Failed to set credential id".to_string(),
                            )
                        },
                    )?;
                }
            }
        }

        // Create CredentialRequestOptions and set the publicKey
        let options = CredentialRequestOptions::new();
        options.set_public_key(&public_key.unchecked_into::<PublicKeyCredentialRequestOptions>());

        Ok(options)
    }

    /// Convert credential to JSON for sending to server
    fn credential_to_json(
        &self,
        credential: &PublicKeyCredential,
    ) -> Result<JsonValue, WebAuthnBrowserError> {
        let response = credential.response();

        let mut json = serde_json::json!({
            "id": credential.id(),
            "rawId": self.array_buffer_to_base64(&credential.raw_id()),
            "type": credential.type_(),
        });

        // Handle attestation response (for registration)
        if let Some(attestation) = response.dyn_ref::<web_sys::AuthenticatorAttestationResponse>() {
            json["response"] = serde_json::json!({
                "clientDataJSON": self.array_buffer_to_base64(&attestation.client_data_json()),
                "attestationObject": self.array_buffer_to_base64(&attestation.attestation_object()),
            });
        }
        // Handle assertion response (for authentication)
        else if let Some(assertion) =
            response.dyn_ref::<web_sys::AuthenticatorAssertionResponse>()
        {
            json["response"] = serde_json::json!({
                "clientDataJSON": self.array_buffer_to_base64(&assertion.client_data_json()),
                "authenticatorData": self.array_buffer_to_base64(&assertion.authenticator_data()),
                "signature": self.array_buffer_to_base64(&assertion.signature()),
            });

            if let Some(user_handle) = assertion.user_handle() {
                json["response"]["userHandle"] =
                    serde_json::Value::String(self.array_buffer_to_base64(&user_handle));
            }
        }

        Ok(json)
    }

    /// Convert ArrayBuffer to base64
    fn array_buffer_to_base64(&self, buffer: &js_sys::ArrayBuffer) -> String {
        let array = js_sys::Uint8Array::new(buffer);
        let bytes: Vec<u8> = array.to_vec();
        URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Convert base64 string to ArrayBuffer
    fn base64_to_array_buffer(
        &self,
        base64: &str,
    ) -> Result<js_sys::ArrayBuffer, WebAuthnBrowserError> {
        let bytes = URL_SAFE_NO_PAD.decode(base64).map_err(|e| {
            WebAuthnBrowserError::ConversionError(format!("Base64 decode error: {e}"))
        })?;

        let array = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
        array.copy_from(&bytes);

        Ok(array.buffer())
    }
}
