use std::fmt;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum OAuthError {
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("Unknown provider type: {0}")]
    UnknownProvider(String),

    #[error("OAuth2 error: {0}")]
    OAuth2Error(#[from] oauth2::reqwest::Error),

    #[error("Invalid authorization code: {0}")]
    InvalidAuthorizationCode(String),

    #[error("Invalid refresh token: {0}")]
    InvalidRefreshToken(String),

    #[error("Access token expired: {0}")]
    AccessTokenExpired(String),

    #[error("State token mismatch: {0}")]
    StateTokenMismatch(String),

    #[error("User authentication failed: {0}")]
    UserAuthenticationFailed(String),

    #[error("Invalid state token")]
    InvalidStateToken,

    #[error("Token expired")]
    TokenExpired,

    #[error("Storage error: {0}")]
    StorageError(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("Token validation failed: {0}")]
    TokenValidationFailed(String),
}

impl fmt::Display for OAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for OAuthError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            OAuthError::OAuth2Error(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_error_display() {
        let error = OAuthError::ConfigurationError("test error".to_string());
        assert_eq!(error.to_string(), "Configuration error: test error");

        let error = OAuthError::ProviderNotFound("github".to_string());
        assert_eq!(error.to_string(), "Provider not found: github");
    }
}