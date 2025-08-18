//! Tracing prelude exports
//!
//! Import this prelude in crates that want convenient access to
//! tracing instrumentation, correlation IDs, and lightweight metrics helpers.

// Core tracing imports
pub use tracing::Instrument;
pub use tracing::instrument;
pub use tracing::{debug, error, info, trace, warn};

// Correlation ID and helpers
pub use crate::tracing::correlation::CorrelationId;
#[cfg(not(target_arch = "wasm32"))]
pub use crate::tracing::correlation::{
    CorrelationScope, current_correlation_id, set_current_correlation_id,
};

// W3C trace context helpers
pub use crate::tracing::trace_context::{
    TRACEPARENT_HEADER, TRACESTATE_HEADER, TraceContext, extract_trace_context,
    inject_trace_context,
};

// Lightweight metrics helpers
pub use crate::tracing::metrics::{
    Counter, Gauge, Histogram, Timer, counter, gauge, global as global_metrics, histogram,
    log_all_metrics,
};
