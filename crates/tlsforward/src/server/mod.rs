//! TLS forward server implementation

pub mod config;
pub mod dns;
pub mod dns_challenge;
pub mod http_handler;
pub mod https_proxy;
pub mod registry;
pub mod router;
pub mod sni;
pub mod state_dir;

// Re-export main server components
pub use config::TlsForwardConfig;
pub use https_proxy::{HttpsProxy, HttpsProxyConfig};
pub use registry::{ProxyRegistry, RegistryEntry};

#[cfg(test)]
mod dns_challenge_tests;
#[cfg(test)]
mod router_tests;
