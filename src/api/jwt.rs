//! Session JWT load and `Authorization` helpers for the TinyHumans API.

pub use crate::openhuman::credentials::session_support::get_session_token;
pub use crate::openhuman::credentials::{APP_SESSION_PROVIDER, DEFAULT_AUTH_PROFILE_NAME};

/// Value for `Authorization: Bearer …` (matches backend expectations).
pub fn bearer_authorization_value(token: &str) -> String {
    format!("Bearer {}", token.trim())
}

/// Like [`get_session_token`] but falls back to the `OPENHUMAN_DEV_JWT_TOKEN`
/// env var when the stored session is empty. This lets local dev / offline
/// sessions use voice transcription and other backend-proxied features without
/// going through the full OAuth flow.
pub fn get_session_token_with_dev_fallback(
    config: &crate::openhuman::config::Config,
) -> Result<Option<String>, String> {
    let token = get_session_token(config)?;
    if token.as_ref().map_or(true, |t| t.trim().is_empty()) {
        if let Ok(dev) = std::env::var("OPENHUMAN_DEV_JWT_TOKEN") {
            let trimmed = dev.trim().to_string();
            if !trimmed.is_empty() {
                log::debug!("[jwt] using OPENHUMAN_DEV_JWT_TOKEN fallback");
                return Ok(Some(trimmed));
            }
        }
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bearer_authorization_value() {
        // Standard token
        assert_eq!(bearer_authorization_value("my_token"), "Bearer my_token");

        // Token with leading/trailing spaces
        assert_eq!(
            bearer_authorization_value("  spaced_token  "),
            "Bearer spaced_token"
        );

        // Empty string
        assert_eq!(bearer_authorization_value(""), "Bearer ");

        // Whitespace only string
        assert_eq!(bearer_authorization_value("   "), "Bearer ");

        // Token with internal spaces (should not be trimmed)
        assert_eq!(
            bearer_authorization_value("token with spaces"),
            "Bearer token with spaces"
        );
    }
}
