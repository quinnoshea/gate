//! User-friendly error message mappings

/// Convert technical errors to user-friendly messages
pub fn get_user_friendly_error(error: &str) -> String {
    let lower = error.to_lowercase();

    if lower.contains("network") || lower.contains("failed to fetch") {
        "Connection error. Please check your internet connection and try again.".to_string()
    } else if lower.contains("unauthorized") || lower.contains("401") {
        "Your session has expired. Please sign in again.".to_string()
    } else if lower.contains("forbidden") || lower.contains("403") {
        "You don't have permission to access this resource.".to_string()
    } else if lower.contains("not found") || lower.contains("404") {
        "The requested resource was not found.".to_string()
    } else if lower.contains("timeout") {
        "The request timed out. Please try again.".to_string()
    } else if lower.contains("canceled")
        || lower.contains("cancelled")
        || lower.contains("user cancel")
    {
        "Authentication was canceled. Please try again when ready.".to_string()
    } else if lower.contains("not supported") {
        "Your browser doesn't support passwordless authentication. Please try a different browser."
            .to_string()
    } else if lower.contains("security") || lower.contains("insecure") {
        "This feature requires a secure connection. Please ensure you're using HTTPS.".to_string()
    } else if lower.contains("already exists") {
        "This device is already registered. Try signing in instead.".to_string()
    } else if lower.contains("credential") && lower.contains("not") && lower.contains("found") {
        "No registered devices found. Please register this device first.".to_string()
    } else if lower.contains("rate limit") {
        "Too many attempts. Please wait a moment and try again.".to_string()
    } else if lower.contains("server") || lower.contains("500") || lower.contains("internal") {
        "Something went wrong on our end. Please try again later.".to_string()
    } else {
        // For unknown errors, return a generic message
        "An unexpected error occurred. Please try again.".to_string()
    }
}
