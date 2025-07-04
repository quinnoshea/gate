//! Tests for role-based access control functionality

use gate_http::error::HttpError;
use gate_http::middleware::auth::AuthenticatedUser;
use gate_http::middleware::rbac::{check_admin_role, check_roles};
use serde_json::json;

#[test]
fn test_check_roles_with_matching_role() {
    let user = AuthenticatedUser {
        id: "user123".to_string(),
        name: Some("Test User".to_string()),
        email: Some("user@example.com".to_string()),
        roles: vec!["user".to_string(), "editor".to_string()],
        metadata: json!({}),
    };

    // Should succeed when user has one of the required roles
    let result = check_roles(&user, &["editor".to_string(), "admin".to_string()]);
    assert!(result.is_ok());
}

#[test]
fn test_check_roles_without_matching_role() {
    let user = AuthenticatedUser {
        id: "user123".to_string(),
        name: Some("Test User".to_string()),
        email: Some("user@example.com".to_string()),
        roles: vec!["user".to_string()],
        metadata: json!({}),
    };

    // Should fail when user doesn't have any required roles
    let result = check_roles(&user, &["editor".to_string(), "admin".to_string()]);
    assert!(result.is_err());

    match result {
        Err(HttpError::AuthorizationFailed(msg)) => {
            assert!(msg.contains("Access denied"));
        }
        _ => panic!("Expected AuthorizationFailed error"),
    }
}

#[test]
fn test_check_roles_with_empty_required_roles() {
    let user = AuthenticatedUser {
        id: "user123".to_string(),
        name: Some("Test User".to_string()),
        email: Some("user@example.com".to_string()),
        roles: vec!["user".to_string()],
        metadata: json!({}),
    };

    // Should fail when no roles are required (edge case)
    let result = check_roles(&user, &[]);
    assert!(result.is_err());
}

#[test]
fn test_check_admin_role_with_admin() {
    let user = AuthenticatedUser {
        id: "admin123".to_string(),
        name: Some("Test Admin".to_string()),
        email: Some("admin@example.com".to_string()),
        roles: vec!["admin".to_string()],
        metadata: json!({}),
    };

    let admin_roles = vec!["admin".to_string(), "superadmin".to_string()];
    let result = check_admin_role(&user, &admin_roles);
    assert!(result.is_ok());
}

#[test]
fn test_check_admin_role_with_superadmin() {
    let user = AuthenticatedUser {
        id: "superadmin123".to_string(),
        name: Some("Test Superadmin".to_string()),
        email: Some("superadmin@example.com".to_string()),
        roles: vec!["superadmin".to_string()],
        metadata: json!({}),
    };

    let admin_roles = vec!["admin".to_string(), "superadmin".to_string()];
    let result = check_admin_role(&user, &admin_roles);
    assert!(result.is_ok());
}

#[test]
fn test_check_admin_role_without_admin() {
    let user = AuthenticatedUser {
        id: "user123".to_string(),
        name: Some("Test User".to_string()),
        email: Some("user@example.com".to_string()),
        roles: vec!["user".to_string(), "editor".to_string()],
        metadata: json!({}),
    };

    let admin_roles = vec!["admin".to_string(), "superadmin".to_string()];
    let result = check_admin_role(&user, &admin_roles);
    assert!(result.is_err());

    match result {
        Err(HttpError::AuthorizationFailed(msg)) => {
            assert_eq!(msg, "Admin access required");
        }
        _ => panic!("Expected AuthorizationFailed error"),
    }
}

#[test]
fn test_check_admin_role_with_multiple_roles() {
    let user = AuthenticatedUser {
        id: "multiuser123".to_string(),
        name: Some("Multi Role User".to_string()),
        email: Some("multiuser@example.com".to_string()),
        roles: vec![
            "user".to_string(),
            "editor".to_string(),
            "admin".to_string(),
        ],
        metadata: json!({}),
    };

    let admin_roles = vec!["admin".to_string()];
    let result = check_admin_role(&user, &admin_roles);
    assert!(result.is_ok());
}

#[test]
fn test_role_comparison_case_sensitive() {
    let user = AuthenticatedUser {
        id: "user123".to_string(),
        name: Some("Test User".to_string()),
        email: Some("user@example.com".to_string()),
        roles: vec!["Admin".to_string()], // Capital A
        metadata: json!({}),
    };

    let admin_roles = vec!["admin".to_string()]; // lowercase a
    let result = check_admin_role(&user, &admin_roles);
    // Should fail because roles are case-sensitive
    assert!(result.is_err());
}
