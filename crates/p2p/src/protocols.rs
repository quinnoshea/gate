//! Protocol definitions and constants

/// Control protocol for handshakes and peer coordination
pub const CONTROL_PROTOCOL: &[u8] = b"gate/control/1.0";

/// Inference protocol for AI chat completions and model queries
pub const INFERENCE_PROTOCOL: &[u8] = b"gate/inference/1.0";

/// SNI proxy protocol for raw TLS traffic forwarding
pub const SNI_PROXY_PROTOCOL: &[u8] = b"gate/sni-proxy/1.0";

/// Domain registration protocol for requesting subdomains from relay
pub const DOMAIN_REGISTRATION_PROTOCOL: &[u8] = b"gate/domain-registration/1.0";
