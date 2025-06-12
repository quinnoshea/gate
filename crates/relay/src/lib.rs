// Gate Relay Server Library
//
// The relay server provides public HTTPS endpoints for Gate nodes by:
// 1. Listening on :443 for TLS connections
// 2. Extracting SNI from TLS ClientHello to identify target node
// 3. Forwarding raw TLS bytes to target node via P2P connection
// 4. Managing DNS records and SSL certificates automatically

pub mod cert_store;
pub mod cloudflare_dns;
pub mod config;
pub mod dns;
pub mod error;
pub mod registry;
pub mod relay;
pub mod sni;
pub mod tls_proxy;

pub use config::RelayConfig;
pub use error::{RelayError, Result};
pub use relay::RelayServer;

// Re-export key types for convenience
pub use cert_store::{CertificateInfo, CertificateStore};
pub use dns::DnsManager;
pub use registry::NodeRegistry;
pub use sni::SniExtractor;
pub use tls_proxy::TlsProxy;
