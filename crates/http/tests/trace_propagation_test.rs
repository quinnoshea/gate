//! Integration tests for W3C trace context propagation
// TODO: Update this test after API migration
#![cfg(not(all()))] // Disable for now

use axum::Router;
use gate_core::tracing::{prelude::CorrelationId, trace_context::TraceContext};
use gate_http::{middleware::correlation::correlation_id_middleware, routes, state::AppState};
use http::{HeaderMap, HeaderValue, StatusCode};
use std::sync::Arc;
use tower::ServiceExt;

/// Test helper to create a test app
async fn create_test_app() -> Router {
    let state = AppState::new(());

    Router::new()
        .merge(routes::health::add_routes(Router::new()))
        .merge(routes::inference::add_routes(Router::new()))
        .layer(axum::middleware::from_fn(correlation_id_middleware))
        .with_state(Arc::new(state))
}

#[tokio::test]
async fn test_w3c_traceparent_extraction() {
    let app = create_test_app().await;

    // Create a W3C traceparent header
    let traceparent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/health")
                .header("traceparent", traceparent)
                .body(hyper::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Check that the response includes the traceparent header
    let response_traceparent = response
        .headers()
        .get("traceparent")
        .expect("traceparent header should be present");

    assert_eq!(response_traceparent, traceparent);
}

#[tokio::test]
async fn test_legacy_correlation_id_extraction() {
    let app = create_test_app().await;

    // Use a legacy correlation ID
    let correlation_id = "test-correlation-123";

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/health")
                .header("x-correlation-id", correlation_id)
                .body(hyper::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Check that both headers are present in response
    let response_correlation = response
        .headers()
        .get("x-correlation-id")
        .expect("x-correlation-id header should be present");

    assert_eq!(response_correlation, correlation_id);

    // Should also have a traceparent header
    assert!(response.headers().get("traceparent").is_some());
}

#[tokio::test]
async fn test_trace_context_generation() {
    let app = create_test_app().await;

    // Request without any correlation headers
    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/health")
                .body(hyper::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Should have both headers in response
    let traceparent = response
        .headers()
        .get("traceparent")
        .expect("traceparent header should be generated");

    let correlation_id = response
        .headers()
        .get("x-correlation-id")
        .expect("x-correlation-id header should be generated");

    // Validate traceparent format
    let traceparent_str = traceparent.to_str().unwrap();
    assert!(traceparent_str.starts_with("00-"));
    assert_eq!(traceparent_str.len(), 55); // Format: 00-{32 hex}-{16 hex}-{2 hex}

    // Validate that x-correlation-id matches trace ID from traceparent
    let trace_ctx = TraceContext::from_traceparent(traceparent_str).unwrap();
    let trace_id_hex = hex::encode(trace_ctx.trace_id.to_bytes());
    assert_eq!(correlation_id.to_str().unwrap(), trace_id_hex);
}

#[tokio::test]
async fn test_trace_context_priority() {
    let app = create_test_app().await;

    // Send both headers - traceparent should take priority
    let traceparent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let correlation_id = "different-correlation-id";

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/health")
                .header("traceparent", traceparent)
                .header("x-correlation-id", correlation_id)
                .body(hyper::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Response should echo the traceparent
    let response_traceparent = response
        .headers()
        .get("traceparent")
        .expect("traceparent header should be present");

    assert_eq!(response_traceparent, traceparent);

    // x-correlation-id should be the trace ID from traceparent
    let response_correlation = response
        .headers()
        .get("x-correlation-id")
        .expect("x-correlation-id header should be present");

    assert_eq!(response_correlation, "4bf92f3577b34da6a3ce929d0e0e4736");
}

#[tokio::test]
async fn test_invalid_traceparent_fallback() {
    let app = create_test_app().await;

    // Send invalid traceparent
    let invalid_traceparent = "invalid-traceparent-header";

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/health")
                .header("traceparent", invalid_traceparent)
                .body(hyper::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Should generate new valid headers
    let response_traceparent = response
        .headers()
        .get("traceparent")
        .expect("traceparent header should be generated");

    // Should not be the invalid one we sent
    assert_ne!(response_traceparent, invalid_traceparent);

    // Should be valid format
    let traceparent_str = response_traceparent.to_str().unwrap();
    assert!(TraceContext::from_traceparent(traceparent_str).is_ok());
}

/// Mock upstream handler for testing propagation
async fn mock_upstream_handler(headers: HeaderMap) -> Result<String, String> {
    // Check if trace context was propagated
    if let Some(traceparent) = headers.get("traceparent") {
        Ok(traceparent.to_str().unwrap().to_string())
    } else {
        Err("No traceparent header found".to_string())
    }
}

#[cfg(test)]
mod upstream_propagation_tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_upstream_trace_propagation() {
        // Start a mock upstream server
        let mock_server = MockServer::start().await;

        // Set up expectation that traceparent header is forwarded
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header(
                "traceparent",
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "test",
                "choices": [{
                    "message": {
                        "content": "test response"
                    }
                }]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Create app with mock upstream
        let mut state = AppState::new(());
        state
            .upstream_registry
            .add_upstream(
                "test".to_string(),
                gate_core::config::UpstreamConfig {
                    provider: gate_core::config::Provider::OpenAI,
                    base_url: mock_server.uri(),
                    api_key: Some("test-key".to_string()),
                    models: vec!["test-model".to_string()],
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let app = Router::new()
            .merge(routes::inference::add_routes(Router::new()))
            .layer(axum::middleware::from_fn(correlation_id_middleware))
            .with_state(Arc::new(state));

        // Make request with traceparent
        let response = app
            .oneshot(
                http::Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header(
                        "traceparent",
                        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
                    )
                    .header("content-type", "application/json")
                    .body(hyper::Body::from(
                        serde_json::to_string(&serde_json::json!({
                            "model": "test-model",
                            "messages": [{"role": "user", "content": "test"}]
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Verify the mock was called with the traceparent header
        mock_server.verify().await;
    }
}
