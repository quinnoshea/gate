pub mod components;
pub mod parsers;
pub mod styles;
pub mod types;
pub mod utils;

// Re-export main components
pub use components::{ChatContainer, Message, MessageList, StreamingIndicator};
pub use types::{
    Attachment, ChatMessage, ChatResponse, ContentBlock, MessageContent, MessageRole,
    MultimodalMessage, Provider, ToolCall, ToolResponse, Usage,
};
pub use utils::flexible_parser::parse_cassette;
