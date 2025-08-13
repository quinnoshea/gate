/// Initialize the default crypto provider for rustls
/// 
/// This must be called once at the start of the application before any TLS operations.
/// Panics if initialization fails.
pub fn init_rustls() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
}