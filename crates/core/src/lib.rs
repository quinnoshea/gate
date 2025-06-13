//! Gate core types and utilities

pub mod error;
pub mod task;
pub mod types;

pub use error::{CoreError, CoreResult, ErrorContext};
pub use task::{IdempotencyKey, TaskEvent, TaskId, TaskStatus};
pub use types::{GateAddr, GateId};
