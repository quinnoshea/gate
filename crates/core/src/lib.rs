//! Gate core types and utilities

pub mod error;
pub mod identity;
pub mod task;
pub mod types;

pub use error::{CoreError, CoreResult, ErrorContext};
pub use identity::{load_identity_from_file, load_or_generate_identity, node_id_from_identity};
pub use task::{IdempotencyKey, TaskEvent, TaskId, TaskStatus};
pub use types::{GateAddr, GateId};
