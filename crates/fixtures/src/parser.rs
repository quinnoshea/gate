use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Represents a conversation from a cassette recording
#[derive(Debug, Clone)]
pub struct Conversation {
    pub interactions: Vec<Interaction>,
}

impl Conversation {
    /// Create a conversation from a cassette JSON string
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        let cassette: Cassette = serde_json::from_str(json_str)?;
        Ok(Self {
            interactions: cassette.interactions,
        })
    }

    /// Get the request body as a JSON Value for a specific interaction
    pub fn request_body(&self, index: usize) -> Option<Value> {
        self.interactions
            .get(index)
            .and_then(|i| serde_json::from_str(&i.request.body).ok())
    }

    /// Get the response body as a JSON Value for a specific interaction
    pub fn response_body(&self, index: usize) -> Option<Value> {
        self.interactions
            .get(index)
            .and_then(|i| serde_json::from_str(&i.response.body.string).ok())
    }

    /// Get the raw response body string (useful for streaming responses)
    pub fn raw_response_body(&self, index: usize) -> Option<&str> {
        self.interactions
            .get(index)
            .map(|i| i.response.body.string.as_str())
    }

    /// Check if this is a streaming conversation
    pub fn is_streaming(&self) -> bool {
        self.interactions
            .first()
            .map(|i| {
                i.response.body.string.contains("event:")
                    && i.response.body.string.contains("data:")
            })
            .unwrap_or(false)
    }
}

/// Internal cassette structure from pytest-recording
#[derive(Debug, Clone, Deserialize, Serialize)]
struct Cassette {
    pub interactions: Vec<Interaction>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Interaction {
    pub request: Request,
    pub response: Response,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Request {
    pub body: String,
    pub headers: HashMap<String, Vec<String>>,
    pub method: String,
    pub uri: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Response {
    pub body: ResponseBody,
    pub headers: HashMap<String, Vec<String>>,
    pub status: StatusCode,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponseBody {
    pub string: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StatusCode {
    pub code: u16,
    pub message: String,
}
