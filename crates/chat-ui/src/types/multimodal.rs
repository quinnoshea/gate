//! Types for multimodal message content (text, images, files)

use serde::{Deserialize, Serialize};

/// Represents different types of content that can be sent in a message
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text content
    Text { text: String },
    /// Image content (base64 encoded or URL)
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageData },
    /// Image content for Anthropic format
    Image { source: ImageSource },
}

/// Image data that can be either a URL or base64 encoded
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageData {
    pub url: String, // Can be a URL or data:image/jpeg;base64,... format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>, // OpenAI specific: "low", "high", "auto"
}

/// Anthropic-style image source
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String, // "base64"
    pub media_type: String, // "image/jpeg", "image/png", etc.
    pub data: String,       // base64 encoded image data
}

/// Represents an attachment that can be added to a message
#[derive(Clone, Debug, PartialEq)]
pub struct Attachment {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>, // Raw file data before encoding
    pub size: usize,
}

impl Attachment {
    /// Convert to base64 data URL format
    pub fn to_data_url(&self) -> String {
        use base64::Engine;
        let base64_data = base64::engine::general_purpose::STANDARD.encode(&self.data);
        format!("data:{};base64,{}", self.mime_type, base64_data)
    }

    /// Check if this attachment is an image
    pub fn is_image(&self) -> bool {
        self.mime_type.starts_with("image/")
    }

    /// Get a human-readable size string
    pub fn size_string(&self) -> String {
        if self.size < 1024 {
            format!("{} B", self.size)
        } else if self.size < 1024 * 1024 {
            format!("{:.1} KB", self.size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", self.size as f64 / (1024.0 * 1024.0))
        }
    }
}

/// Message content that can include text and attachments
#[derive(Clone, Debug, PartialEq)]
pub struct MultimodalMessage {
    pub text: Option<String>,
    pub attachments: Vec<Attachment>,
}

impl MultimodalMessage {
    /// Create a text-only message
    pub fn text(text: String) -> Self {
        Self {
            text: Some(text),
            attachments: Vec::new(),
        }
    }

    /// Create a message with attachments
    pub fn with_attachments(text: Option<String>, attachments: Vec<Attachment>) -> Self {
        Self { text, attachments }
    }

    /// Convert to content blocks for API submission
    pub fn to_content_blocks(&self) -> Vec<ContentBlock> {
        let mut blocks = Vec::new();

        // Add images first (following best practices)
        for attachment in &self.attachments {
            if attachment.is_image() {
                blocks.push(ContentBlock::ImageUrl {
                    image_url: ImageData {
                        url: attachment.to_data_url(),
                        detail: None,
                    },
                });
            }
        }

        // Add text content after images
        if let Some(text) = &self.text {
            if !text.trim().is_empty() {
                blocks.push(ContentBlock::Text { text: text.clone() });
            }
        }

        blocks
    }
}
