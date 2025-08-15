//! Shared tracing functionality for Gate
//!
//! This module provides common tracing utilities that work across both
//! WASM and native environments.

#[cfg(feature = "tracing")]
pub mod correlation;
#[cfg(feature = "tracing")]
pub mod metrics;
#[cfg(feature = "tracing")]
pub mod prelude;
#[cfg(feature = "tracing")]
pub mod trace_context;

#[cfg(all(feature = "tracing-otlp", not(target_arch = "wasm32")))]
pub mod config;
#[cfg(all(feature = "tracing-otlp", not(target_arch = "wasm32")))]
pub mod init;

#[cfg(all(feature = "tracing", not(target_arch = "wasm32")))]
pub mod file_rotation;

#[cfg(all(feature = "tracing-prometheus", not(target_arch = "wasm32")))]
pub mod prometheus;

// Re-export commonly used types
#[cfg(feature = "tracing")]
pub use correlation::CorrelationId;
#[cfg(feature = "tracing")]
pub use trace_context::{TraceContext, TraceContextError};
