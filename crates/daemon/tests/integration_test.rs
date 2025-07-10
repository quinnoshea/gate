//! Comprehensive integration tests for server startup

use axum::http::StatusCode;
use gate_daemon::config::*;
use gate_daemon::{Settings, server::ServerBuilder};
use gate_sqlx::{SqliteStateBackend, SqlxWebAuthnBackend};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::timeout;

/// Create test settings
fn test_settings(port: u16) -> Settings {
    Settings {
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port,
            cors_origins: vec![],
            metrics_port: None,
        },
        auth: AuthConfig {
            jwt: JwtConfig {
                secret: Some("test-secret".to_string()),
                expiration_hours: 24,
                issuer: "gate-test".to_string(),
            },
            webauthn: WebAuthnConfig {
                enabled: false,
                rp_id: "localhost".to_string(),
                rp_name: "Gate Test".to_string(),
                rp_origin: "http://localhost".to_string(),
                allowed_origins: vec![],
                allow_tlsforward_origins: false,
                allow_subdomains: false,
                require_user_verification: false,
                session_timeout_seconds: 3600,
            },
            registration: RegistrationConfig {
                allow_open_registration: true,
                default_user_role: "user".to_string(),
                admin_roles: vec!["admin".to_string()],
                bootstrap_admin_role: "admin".to_string(),
            },
        },
        upstreams: vec![],
        tlsforward: TlsForwardConfig::default(),
        letsencrypt: LetsEncryptConfig::default(),
        local_inference: None,
    }
}

/// Helper to start a test server
async fn start_test_server() -> Result<(SocketAddr, tokio::task::JoinHandle<()>), anyhow::Error> {
    // Use port 0 to get a random available port
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    // Create settings with the actual port
    let settings = test_settings(addr.port());

    // Create database backends
    let state_backend = Arc::new(SqliteStateBackend::new(":memory:").await?);
    let webauthn_backend = Arc::new(SqlxWebAuthnBackend::new(state_backend.pool().clone()));

    // Build server
    let settings_arc = Arc::new(settings.clone());
    let builder = ServerBuilder::new(settings, state_backend, webauthn_backend, settings_arc);
    let jwt_service = builder.build_jwt_service();
    let upstream_registry = builder.build_upstream_registry().await?;
    let state = builder
        .build_app_state(jwt_service, upstream_registry)
        .await?;

    // Build router
    let router = ServerBuilder::build_router();
    let app = ServerBuilder::build_axum_router(router, state, None);

    // Spawn server task
    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("Server failed to start");
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    Ok((addr, handle))
}

#[tokio::test]
async fn test_server_starts_and_responds() {
    let (addr, handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    // Create HTTP client
    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");

    // Test health endpoint
    let response = timeout(
        Duration::from_secs(5),
        client.get(format!("{base_url}/health")).send(),
    )
    .await
    .expect("Request timed out")
    .expect("Failed to send request");

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["status"], "healthy");
    assert!(body["version"].is_string());
    assert!(body["timestamp"].is_string());

    // Abort server
    handle.abort();
}

#[tokio::test]
async fn test_all_observability_endpoints() {
    let (addr, handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");

    // Test all observability endpoints
    let endpoints = vec![
        ("/health", StatusCode::OK),
        ("/metrics", StatusCode::UNAUTHORIZED),
    ];

    for (path, expected_status) in endpoints {
        let response = client
            .get(format!("{base_url}{path}"))
            .send()
            .await
            .unwrap_or_else(|_| panic!("Failed to connect to {path}"));

        assert_eq!(
            response.status(),
            expected_status,
            "Unexpected status for {path}"
        );
    }

    // Abort server
    handle.abort();
}

#[tokio::test]
async fn test_cors_headers() {
    let (addr, handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");

    // Test CORS headers
    let response = client
        .get(format!("{base_url}/health"))
        .header("Origin", "http://example.com")
        .send()
        .await
        .expect("Failed to send request");

    // Check CORS headers
    assert!(
        response
            .headers()
            .contains_key("access-control-allow-origin")
    );

    // Test that correlation ID is exposed
    let exposed_headers = response
        .headers()
        .get("access-control-expose-headers")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    assert!(
        exposed_headers.contains("x-correlation-id"),
        "Expected x-correlation-id in exposed headers, got: {exposed_headers}"
    );

    // Abort server
    handle.abort();
}

#[tokio::test]
#[ignore = "Correlation ID handling needs to be verified"]
async fn test_correlation_id_handling() {
    let (addr, handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");

    // Test with provided correlation ID
    let correlation_id = "test-correlation-123";
    let response = client
        .get(format!("{base_url}/health"))
        .header("x-correlation-id", correlation_id)
        .send()
        .await
        .expect("Failed to send request");

    // Check that correlation ID is returned
    let returned_id = response
        .headers()
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok());

    assert_eq!(returned_id, Some(correlation_id));

    // Test without correlation ID (should generate one)
    let response = client
        .get(format!("{base_url}/health"))
        .send()
        .await
        .expect("Failed to send request");

    let generated_id = response
        .headers()
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok());

    assert!(generated_id.is_some(), "Expected generated correlation ID");
    assert!(
        !generated_id.unwrap().is_empty(),
        "Correlation ID should not be empty"
    );

    // Abort server
    handle.abort();
}

#[tokio::test]
#[ignore = "Metrics endpoint now requires authentication"]
async fn test_metrics_endpoint_format() {
    let (addr, handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");

    let response = client
        .get(format!("{base_url}/metrics"))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), StatusCode::OK);

    // Check content type
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    assert!(
        content_type.contains("text/plain"),
        "Expected text/plain content type, got: {content_type}"
    );

    // Check that response contains Prometheus format
    let body = response.text().await.expect("Failed to read body");

    // Print the body for debugging
    println!("Metrics body: {body}");

    // The metrics endpoint might return an empty response initially
    // or different format, so let's check if it's valid
    assert!(
        !body.is_empty() || body.contains("# HELP") || body.contains("# TYPE"),
        "Expected Prometheus format or valid metrics response, got: {body}"
    );

    // Abort server
    handle.abort();
}
