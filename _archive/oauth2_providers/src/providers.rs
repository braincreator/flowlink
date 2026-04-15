use anyhow::Result;
use chrono::{Duration, Utc};
use std::sync::Arc;
use urlencoding::encode;

use super::*;
use super::error::OAuthError;

// Google OAuth2 Provider
pub struct GoogleOAuthProvider {
    pub config: OAuthProviderConfig,
}

impl GoogleOAuthProvider {
    pub fn new(config: OAuthProviderConfig) -> Result<Self> {
        if config.client_id.is_empty() {
            return Err(OAuthError::ConfigurationError("Google client_id is required".to_string()));
        }

        Ok(Self { config })
    }

    pub fn get_authorization_url(&self, _config: &OAuthProviderConfig) -> Result<String> {
        let scopes = if !self.config.scope.is_empty() {
            self.config.scope.join(" ")
        } else {
            "openid email profile".to_string()
        };

        let params = [
            ("response_type", "code"),
            ("client_id", &self.config.client_id),
            ("redirect_uri", &self.config.redirect_uri),
            ("scope", &scopes),
            ("access_type", "offline"),
            ("include_granted_scopes", "true"),
            ("prompt", "select_account"),
        ];

        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(&params)
            .finish();

        Ok(format!("{}?{}", self.google_config().authorization_endpoint, query))
    }

    pub async fn exchange_code_for_token(
        &self,
        _config: &OAuthProviderConfig,
        code: &str,
    ) -> Result<OAuthTokenResponse> {
        let client = reqwest::Client::new();

        let response = client
            .post(self.google_config().token_endpoint)
            .form(&[
                ("code", code),
                ("client_id", &self.config.client_id),
                ("client_secret", &self.config.client_secret),
                ("redirect_uri", &self.config.redirect_uri),
                ("grant_type", "authorization_code"),
                ("access_type", "offline"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(OAuthError::OAuth2Error(anyhow::anyhow!(
                "Failed to exchange code for token: {}",
                error_text
            )));
        }

        let google_token: GoogleTokenResponse = response.json().await?;

        Ok(OAuthTokenResponse {
            access_token: google_token.access_token,
            token_type: google_token.token_type,
            expires_in: google_token.expires_in,
            refresh_token: google_token.refresh_token,
            scope: Some(google_token.scope),
            id_token: google_token.id_token,
        })
    }

    pub async fn refresh_token(
        &self,
        _config: &OAuthProviderConfig,
        refresh_token: &str,
    ) -> Result<OAuthTokenResponse> {
        let client = reqwest::Client::new();

        let response = client
            .post(self.google_config().token_endpoint)
            .form(&[
                ("refresh_token", refresh_token),
                ("client_id", &self.config.client_id),
                ("client_secret", &self.config.client_secret),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(OAuthError::OAuth2Error(anyhow::anyhow!(
                "Failed to refresh token: {}",
                error_text
            )));
        }

        let google_token: GoogleTokenResponse = response.json().await?;

        Ok(OAuthTokenResponse {
            access_token: google_token.access_token,
            token_type: google_token.token_type,
            expires_in: google_token.expires_in,
            refresh_token: google_token.refresh_token,
            scope: Some(google_token.scope),
            id_token: google_token.id_token,
        })
    }

    pub async fn revoke_token(&self, _config: &OAuthProviderConfig, token: &str) -> Result<()> {
        let client = reqwest::Client::new();

        let response = client
            .post("https://oauth2.googleapis.com/revoke")
            .form(&[("token", token)])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to revoke token"));
        }

        Ok(())
    }

    pub async fn get_user_info(&self, access_token: &str) -> Result<OAuthUserInfo> {
        let client = reqwest::Client::new();

        let response = client
            .get("https://www.googleapis.com/oauth2/v2/userinfo")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(OAuthError::OAuth2Error(anyhow::anyhow!(
                "Failed to get user info: {}",
                error_text
            )));
        }

        let google_user: GoogleUserInfo = response.json().await?;

        Ok(OAuthUserInfo {
            provider: "google".to_string(),
            user_id: google_user.id.clone(),
            email: Some(google_user.email),
            email_verified: Some(google_user.verified_email),
            name: Some(google_user.name),
            given_name: Some(google_user.given_name),
            family_name: Some(google_user.family_name),
            picture: Some(google_user.picture),
            locale: Some(google_user.locale),
            raw_data: serde_json::to_value(google_user)?,
        })
    }

    fn google_config(&self) -> OAuthProviderInfo {
        OAuthProviderInfo {
            name: "Google".to_string(),
            type_: "google".to_string(),
            url: "https://accounts.google.com".to_string(),
            authorization_endpoint: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_endpoint: "https://oauth2.googleapis.com/token".to_string(),
            userinfo_endpoint: Some("https://www.googleapis.com/oauth2/v2/userinfo".to_string()),
            scopes: vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
            ],
        }
    }
}

// GitHub OAuth2 Provider
pub struct GithubOAuthProvider {
    pub config: OAuthProviderConfig,
}

impl GithubOAuthProvider {
    pub fn new(config: OAuthProviderConfig) -> Result<Self> {
        if config.client_id.is_empty() {
            return Err(OAuthError::ConfigurationError("GitHub client_id is required".to_string()));
        }

        Ok(Self { config })
    }

    pub fn get_authorization_url(&self, _config: &OAuthProviderConfig) -> Result<String> {
        let scopes = if !self.config.scope.is_empty() {
            self.config.scope.join(" ")
        } else {
            "user:email".to_string()
        };

        let params = [
            ("client_id", &self.config.client_id),
            ("redirect_uri", &self.config.redirect_uri),
            ("scope", &scopes),
            ("allow_signup", "true"),
            ("state", &self.config.state),
        ];

        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(&params)
            .finish();

        Ok(format!("{}?{}", "https://github.com/login/oauth/authorize", query))
    }

    pub async fn exchange_code_for_token(
        &self,
        _config: &OAuthProviderConfig,
        code: &str,
    ) -> Result<OAuthTokenResponse> {
        let client = reqwest::Client::new();

        let response = client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .json(&serde_json::json!({
                "client_id": &self.config.client_id,
                "client_secret": &self.config.client_secret,
                "code": code,
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(OAuthError::OAuth2Error(anyhow::anyhow!(
                "Failed to exchange code for token: {}",
                error_text
            )));
        }

        let github_token: GithubTokenResponse = response.json().await?;

        Ok(OAuthTokenResponse {
            access_token: github_token.access_token,
            token_type: github_token.token_type,
            expires_in: 28800, // 8 hours for GitHub
            refresh_token: None, // GitHub doesn't support refresh tokens
            scope: Some(github_token.scope),
            id_token: None,
        })
    }

    pub async fn refresh_token(&self, _config: &OAuthProviderConfig, _refresh_token: &str) -> Result<OAuthTokenResponse> {
        Err(OAuthError::InvalidRefreshToken("GitHub does not support refresh tokens".to_string()))
    }

    pub async fn revoke_token(&self, _config: &OAuthProviderConfig, token: &str) -> Result<()> {
        let client = reqwest::Client::new();

        let response = client
            .post("https://github.com/settings/apps/remove_token")
            .header("Authorization", format!("token {}", token))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to revoke token"));
        }

        Ok(())
    }

    pub async fn get_user_info(&self, access_token: &str) -> Result<OAuthUserInfo> {
        let client = reqwest::Client::new();

        let response = client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(OAuthError::OAuth2Error(anyhow::anyhow!(
                "Failed to get user info: {}",
                error_text
            )));
        }

        let github_user: GithubUserInfo = response.json().await?;

        Ok(OAuthUserInfo {
            provider: "github".to_string(),
            user_id: github_user.id.to_string(),
            email: github_user.email,
            email_verified: None, // GitHub doesn't provide email verification status
            name: github_user.name,
            given_name: github_user.name,
            family_name: None,
            picture: Some(github_user.avatar_url),
            locale: None,
            raw_data: serde_json::to_value(github_user)?,
        })
    }

    fn github_config(&self) -> OAuthProviderInfo {
        OAuthProviderInfo {
            name: "GitHub".to_string(),
            type_: "github".to_string(),
            url: "https://github.com".to_string(),
            authorization_endpoint: "https://github.com/login/oauth/authorize".to_string(),
            token_endpoint: "https://github.com/login/oauth/access_token".to_string(),
            userinfo_endpoint: Some("https://api.github.com/user".to_string()),
            scopes: vec![
                "user:email".to_string(),
            ],
        }
    }
}

// Microsoft OAuth2 Provider
pub struct MicrosoftOAuthProvider {
    pub config: OAuthProviderConfig,
}

impl MicrosoftOAuthProvider {
    pub fn new(config: OAuthProviderConfig) -> Result<Self> {
        if config.client_id.is_empty() {
            return Err(OAuthError::ConfigurationError("Microsoft client_id is required".to_string()));
        }

        Ok(Self { config })
    }

    pub fn get_authorization_url(&self, _config: &OAuthProviderConfig) -> Result<String> {
        let scopes = if !self.config.scope.is_empty() {
            self.config.scope.join(" ")
        } else {
            "User.Read openid email profile".to_string()
        };

        let params = [
            ("client_id", &self.config.client_id),
            ("response_type", "code"),
            ("redirect_uri", &self.config.redirect_uri),
            ("scope", &scopes),
            ("response_mode", "query"),
            ("state", &self.config.state),
        ];

        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(&params)
            .finish();

        Ok(format!("{}?{}", self.microsoft_config().authorization_endpoint, query))
    }

    pub async fn exchange_code_for_token(
        &self,
        _config: &OAuthProviderConfig,
        code: &str,
    ) -> Result<OAuthTokenResponse> {
        let client = reqwest::Client::new();

        let response = client
            .post(self.microsoft_config().token_endpoint)
            .form(&[
                ("client_id", &self.config.client_id),
                ("client_secret", &self.config.client_secret),
                ("code", code),
                ("grant_type", "authorization_code"),
                ("redirect_uri", &self.config.redirect_uri),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(OAuthError::OAuth2Error(anyhow::anyhow!(
                "Failed to exchange code for token: {}",
                error_text
            )));
        }

        let microsoft_token: MicrosoftTokenResponse = response.json().await?;

        Ok(OAuthTokenResponse {
            access_token: microsoft_token.access_token,
            token_type: microsoft_token.token_type,
            expires_in: microsoft_token.expires_in,
            refresh_token: microsoft_token.refresh_token,
            scope: Some(microsoft_token.scope),
            id_token: microsoft_token.id_token,
        })
    }

    pub async fn refresh_token(
        &self,
        _config: &OAuthProviderConfig,
        refresh_token: &str,
    ) -> Result<OAuthTokenResponse> {
        let client = reqwest::Client::new();

        let response = client
            .post(self.microsoft_config().token_endpoint)
            .form(&[
                ("client_id", &self.config.client_id),
                ("client_secret", &self.config.client_secret),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(OAuthError::OAuth2Error(anyhow::anyhow!(
                "Failed to refresh token: {}",
                error_text
            )));
        }

        let microsoft_token: MicrosoftTokenResponse = response.json().await?;

        Ok(OAuthTokenResponse {
            access_token: microsoft_token.access_token,
            token_type: microsoft_token.token_type,
            expires_in: microsoft_token.expires_in,
            refresh_token: microsoft_token.refresh_token,
            scope: Some(microsoft_token.scope),
            id_token: microsoft_token.id_token,
        })
    }

    pub async fn revoke_token(&self, _config: &OAuthProviderConfig, token: &str) -> Result<()> {
        let client = reqwest::Client::new();

        let response = client
            .post("https://login.microsoftonline.com/common/oauth2/v2.0/revoke")
            .form(&[("token", token)])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to revoke token"));
        }

        Ok(())
    }

    pub async fn get_user_info(&self, access_token: &str) -> Result<OAuthUserInfo> {
        let client = reqwest::Client::new();

        let response = client
            .get("https://graph.microsoft.com/v1.0/me")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(OAuthError::OAuth2Error(anyhow::anyhow!(
                "Failed to get user info: {}",
                error_text
            )));
        }

        let microsoft_user: MicrosoftUserInfo = response.json().await?;

        Ok(OAuthUserInfo {
            provider: "microsoft".to_string(),
            user_id: microsoft_user.id.clone(),
            email: microsoft_user.email,
            email_verified: Some(microsoft_user.verified),
            name: microsoft_user.name,
            given_name: microsoft_user.given_name,
            family_name: microsoft_user.family_name,
            picture: None, // Microsoft doesn't provide picture in basic profile
            locale: microsoft_user.locale,
            raw_data: serde_json::to_value(microsoft_user)?,
        })
    }

    fn microsoft_config(&self) -> OAuthProviderInfo {
        OAuthProviderInfo {
            name: "Microsoft".to_string(),
            type_: "microsoft".to_string(),
            url: "https://login.microsoftonline.com".to_string(),
            authorization_endpoint: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string(),
            token_endpoint: "https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string(),
            userinfo_endpoint: Some("https://graph.microsoft.com/v1.0/me".to_string()),
            scopes: vec![
                "User.Read".to_string(),
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
            ],
        }
    }
}