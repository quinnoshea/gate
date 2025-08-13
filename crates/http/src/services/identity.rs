use crate::error::HttpError;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use gate_core::access::{IdentityContext, SubjectIdentity};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Deref;

/// Generic HTTP context that can be used by any deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpContext {
    pub attributes: HashMap<String, String>,
}

impl HttpContext {
    pub fn new() -> Self {
        Self {
            attributes: HashMap::new(),
        }
    }

    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }
}

impl IdentityContext for HttpContext {
    fn to_attributes(&self) -> HashMap<String, String> {
        self.attributes.clone()
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.attributes.get(key).map(|s| s.as_str())
    }
}

impl Default for HttpContext {
    fn default() -> Self {
        Self::new()
    }
}

/// HTTP identity wrapper that can implement FromRequestParts
#[derive(Debug, Clone)]
pub struct HttpIdentity(pub SubjectIdentity<HttpContext>);

impl HttpIdentity {
    pub fn new(id: String, source: String, context: HttpContext) -> Self {
        Self(SubjectIdentity {
            id,
            source,
            context,
        })
    }
}

impl Deref for HttpIdentity {
    type Target = SubjectIdentity<HttpContext>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S> FromRequestParts<S> for HttpIdentity
where
    S: Send + Sync,
{
    type Rejection = HttpError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<HttpIdentity>()
            .cloned()
            .ok_or_else(|| HttpError::AuthenticationFailed("User not authenticated".to_string()))
    }
}
