//! Dynamic TLS acceptor that can be reloaded with new certificates

use gate_tlsforward::TlsAcceptorProvider;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_rustls::TlsAcceptor;

/// A TLS acceptor that can be dynamically reloaded with new certificates
#[derive(Clone)]
pub struct ReloadableTlsAcceptor {
    inner: Arc<RwLock<TlsAcceptor>>,
}

impl ReloadableTlsAcceptor {
    /// Create a new reloadable TLS acceptor
    pub fn new(acceptor: TlsAcceptor) -> Self {
        Self {
            inner: Arc::new(RwLock::new(acceptor)),
        }
    }

    /// Get the current TLS acceptor
    pub async fn get(&self) -> TlsAcceptor {
        self.inner.read().await.clone()
    }

    /// Update the TLS acceptor with a new one
    pub async fn reload(&self, new_acceptor: TlsAcceptor) {
        *self.inner.write().await = new_acceptor;
    }
}

impl TlsAcceptorProvider for ReloadableTlsAcceptor {
    fn get_acceptor(&self) -> Pin<Box<dyn Future<Output = TlsAcceptor> + Send + '_>> {
        Box::pin(self.get())
    }
}
