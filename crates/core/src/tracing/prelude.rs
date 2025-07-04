//! Prelude for common tracing functionality
//!
//! This module re-exports commonly used types and functions for convenience.

pub use crate::tracing::correlation::CorrelationId;
pub use crate::tracing::metrics::{
    Counter, Gauge, Histogram, Timer, counter, gauge, histogram, log_all_metrics,
};
pub use crate::tracing::trace_context::{
    TraceContext, TraceContextError, extract_trace_context, inject_trace_context,
};

// Re-export common tracing macros and types
pub use tracing::{Instrument, debug, error, info, instrument, trace, warn};

// Native-only exports
#[cfg(not(target_arch = "wasm32"))]
pub use crate::tracing::correlation::{
    CorrelationScope, current_correlation_id, set_current_correlation_id,
};

// Convenience macros for logging with correlation ID
#[macro_export]
macro_rules! trace_with_correlation {
    ($($arg:tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(correlation_id) = $crate::tracing::prelude::current_correlation_id() {
                tracing::trace!(correlation_id = %correlation_id, $($arg)*);
            } else {
                tracing::trace!($($arg)*);
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            tracing::trace!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! debug_with_correlation {
    ($($arg:tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(correlation_id) = $crate::tracing::prelude::current_correlation_id() {
                tracing::debug!(correlation_id = %correlation_id, $($arg)*);
            } else {
                tracing::debug!($($arg)*);
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            tracing::debug!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! info_with_correlation {
    ($($arg:tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(correlation_id) = $crate::tracing::prelude::current_correlation_id() {
                tracing::info!(correlation_id = %correlation_id, $($arg)*);
            } else {
                tracing::info!($($arg)*);
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            tracing::info!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! error_with_correlation {
    ($($arg:tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(correlation_id) = $crate::tracing::prelude::current_correlation_id() {
                tracing::error!(correlation_id = %correlation_id, $($arg)*);
            } else {
                tracing::error!($($arg)*);
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            tracing::error!($($arg)*);
        }
    };
}
