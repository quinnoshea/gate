//! Request tracing middleware

use axum::{extract::Request, middleware::Next, response::Response};

/// Tracing middleware configuration
#[derive(Debug, Clone)]
pub struct TraceMiddleware {
    pub service_name: String,
}

impl Default for TraceMiddleware {
    fn default() -> Self {
        Self {
            service_name: "gate-http".to_string(),
        }
    }
}

/// Middleware function for request tracing
pub async fn trace_middleware(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path().to_string();

    let span = tracing::info_span!(
        "http_request",
        http.method = %method,
        http.path = %path,
        http.status_code = tracing::field::Empty,
    );

    let _enter = span.enter();

    tracing::debug!("Processing request");
    let response = next.run(req).await;

    // Record the status code
    span.record("http.status_code", response.status().as_u16());

    response
}
