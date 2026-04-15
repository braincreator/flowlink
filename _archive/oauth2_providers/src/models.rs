use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

// OAuth2 configuration models
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OAuthProviderConfig {
    pub provider_type: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scope: Vec<String>,
    pub state: String,
    pub code_verifier: String,
    pub auto_approve: bool,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    pub id_token: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OAuthUserInfo {
    pub provider: String,
    pub user_id: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub picture: Option<String>,
    pub locale: Option<String>,
    pub raw_data: serde_json::Value,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OAuthProviderInfo {
    pub name: String,
    pub type_: String,
    pub url: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: Option<String>,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct StateToken {
    pub state: String,
    pub provider: String,
    pub user_id: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub used: bool,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OAuthSession {
    pub id: String,
    pub user_id: String,
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub expires_at: DateTime<Utc>,
    pub scope: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OAuthStats {
    pub registered_providers: usize,
    pub providers: Vec<(String, OAuthProviderInfo)>,
}

// OAuth2 request models
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AuthorizationRequest {
    pub provider: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<Vec<String>>,
    pub state: Option<String>,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TokenExchangeRequest {
    pub provider: String,
    pub code: String,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RefreshTokenRequest {
    pub provider: String,
    pub refresh_token: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RevokeTokenRequest {
    pub provider: String,
    pub token: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct UserInfoRequest {
    pub provider: String,
    pub access_token: String,
}

// Response models
#[derive(Debug, Clone, Serialize)]
pub struct AuthResponse {
    pub success: bool,
    pub message: String,
    pub token: Option<String>,
    pub user: Option<OAuthUserInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthError {
    pub error: String,
    pub error_description: Option<String>,
    pub error_uri: Option<String>,
}

// Token rotation models
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TokenRotation {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
}

// ID Token claims (OpenID Connect)
#[derive(Debug, Clone, serde::Deserialize)]
pub struct IdTokenClaims {
    pub iss: String,
    pub sub: String,
    pub aud: Vec<String>,
    #[serde(with = "serde_aux::field_attributes::from_option")]
    pub exp: i64,
    #[serde(with = "serde_aux::field_attributes::from_option")]
    pub iat: i64,
    #[serde(with = "serde_aux::field_attributes::from_option")]
    pub nonce: Option<String>,
    pub auth_time: Option<i64>,
    #[serde(with = "serde_aux::field_attributes::from_option")]
    pub acr: Option<String>,
    #[serde(with = "serde_aux::field_attributes::from_option")]
    pub amr: Option<Vec<String>>,
}

// Provider-specific token models
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GoogleTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: String,
    pub id_token: Option<String>,
    pub user_id: String,
    pub granted_scopes: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GithubTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub scope: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MicrosoftTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: String,
    pub id_token: Option<String>,
    pub user_id: String,
}

// Provider-specific user info models
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GoogleUserInfo {
    pub id: String,
    pub email: String,
    pub verified_email: bool,
    pub name: String,
    pub given_name: String,
    pub family_name: String,
    pub picture: String,
    pub locale: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GithubUserInfo {
    pub login: String,
    pub id: i64,
    pub node_id: String,
    pub avatar_url: String,
    pub gravatar_id: String,
    pub url: String,
    pub html_url: String,
    pub followers_url: String,
    pub following_url: String,
    pub gists_url: String,
    pub starred_url: String,
    pub subscriptions_url: String,
    pub organizations_url: String,
    pub repos_url: String,
    pub events_url: String,
    pub received_events_url: String,
    pub type: String,
    pub site_admin: bool,
    pub name: Option<String>,
    pub company: Option<String>,
    pub blog: String,
    pub location: Option<String>,
    pub email: Option<String>,
    pub hireable: Option<bool>,
    pub bio: Option<String>,
    pub public_repos: i32,
    pub public_gists: i32,
    pub followers: i32,
    pub following: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MicrosoftUserInfo {
    pub id: String,
    pub oid: Option<String>,
    pub preferred_username: Option<String>,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub email: Option<String>,
    pub_verified: bool,
    pub locale: Option<String>,
    pub phone_number: Option<String>,
    pub businessPhones: Option<Vec<String>>,
    pub streetAddress: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
    pub postalCode: Option<String>,
}