pub mod bootstrap;
pub mod context;
pub mod errors;
pub mod plugins;
pub mod state;
pub mod types;
pub mod validation;
pub mod webauthn;

#[cfg(feature = "tracing")]
pub mod tracing;

#[cfg(any(test, feature = "tests"))]
pub mod tests;

pub use bootstrap::BootstrapTokenValidator;
pub use context::RequestContext;
pub use errors::{Error, Result};
pub use plugins::{GatePlugin, HookRegistry, PluginContext, PluginManager};
pub use state::StateBackend;
pub use validation::{ValidateConfig, validators};
pub use webauthn::{StoredCredential, WebAuthnBackend};

// Re-export types for convenience
pub use types::{
    ApiKey, ChatCompletionRequest, ChatCompletionResponse, Error as ProtoError, HookAction,
    HookResponse, MessagesRequest, MessagesResponse, Model, ModelType, Organization, Provider,
    ProviderType, RequestHookContext, ResponseHookContext, StreamingChatCompletionResponse,
    StreamingMessagesResponse, TimeRange, UsageRecord, User,
};
