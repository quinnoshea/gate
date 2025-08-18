//! Tracing module root
//!
//! This module groups tracing-related utilities including configuration,
//! initialization helpers, OpenTelemetry/Prometheus integration, correlation
//! context helpers, and a size-based file rotation appender.

pub mod config;
pub mod correlation;
pub mod file_rotation;
pub mod init;
pub mod metrics;
pub mod prelude;
pub mod prometheus;
pub mod trace_context;
