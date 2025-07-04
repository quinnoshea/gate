//! Role-based access control middleware

use crate::error::HttpError;
use crate::middleware::auth::AuthenticatedUser;
use std::collections::HashSet;

/// Extension to hold required roles for an endpoint
#[derive(Clone)]
pub struct RequiredRoles(pub HashSet<String>);

/// Helper function to check if user has required roles
pub fn check_roles(user: &AuthenticatedUser, required_roles: &[String]) -> Result<(), HttpError> {
    let user_roles: HashSet<String> = user.roles.iter().cloned().collect();
    let has_required_role = required_roles.iter().any(|role| user_roles.contains(role));

    if !has_required_role {
        return Err(HttpError::AuthorizationFailed(format!(
            "Access denied. Required roles: {:?}, user roles: {:?}",
            required_roles, user.roles
        )));
    }

    Ok(())
}

/// Helper function to check if user has admin role
pub fn check_admin_role(user: &AuthenticatedUser, admin_roles: &[String]) -> Result<(), HttpError> {
    let is_admin = admin_roles
        .iter()
        .any(|admin_role| user.roles.contains(admin_role));

    if !is_admin {
        return Err(HttpError::AuthorizationFailed(
            "Admin access required".to_string(),
        ));
    }

    Ok(())
}
