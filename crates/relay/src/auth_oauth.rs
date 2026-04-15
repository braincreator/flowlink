//! Auth API endpoints — OAuth callbacks, JWT tokens, user info
//!
//! Provides authentication endpoints for FlowLink:
//! - `/api/auth/callback/{provider}` — OAuth callbacks (VK, Yandex, GitHub)
//! - `/api/auth/me` — Get current authenticated user info
//! - `/api/auth/token` — Exchange OAuth code for JWT
//! - `/api/auth/refresh` — Refresh access token
//!
//! **Flow:**
//! 1. User clicks "Login via VK/Yandex/GitHub" → Redirects to OAuth provider
//! 2. Provider redirects back to `https://flowlink.flow-masters.ru/api/auth/callback/{provider}?code=...&state=...`
//! 3. Backend exchanges OAuth code for JWT token
//! 4. JWT token stored in localStorage → User logged in

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Json,
};
use crate::config::{self as Config};
use crate::server::AppState;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use super::auth::OAuthUser;
use super::auth::urlencoding;

// --------------------------------------------------------------------------- //
// OAuth Callback Query Params
// --------------------------------------------------------------------------- //

#[derive(Debug, Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: String,
    pub state: Option<String>,
    pub provider: Option<String>,
}

// --------------------------------------------------------------------------- //
// Refresh Token Request
// --------------------------------------------------------------------------- //

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

// --------------------------------------------------------------------------- //
// Refresh Token Response
// --------------------------------------------------------------------------- //

#[derive(Debug, Serialize)]
pub struct RefreshTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub token_type: String,
}

// --------------------------------------------------------------------------- //
// Response Types
// --------------------------------------------------------------------------- //

#[derive(Debug, Serialize)]
pub struct AuthMeResponse {
    pub account_id: String,
    pub email: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub tg_id: Option<String>,
    pub created_at: i64,
    pub plan: Option<String>,
    pub active: bool,
    pub servers_count: i32,
}

// --------------------------------------------------------------------------- //
// Helper: Exchange OAuth code for JWT
// --------------------------------------------------------------------------- //

async fn exchange_oauth_code(
    provider: &str,
    code: &str,
    db: &flowlink_db::DbPool,
    config: &Config,
) -> Result<(String, String), AuthError> {
    info!("Exchanging OAuth code for provider: {}", provider);

    // OAuth 1.0 exchange (POST with code to /oauth/token endpoint)
    // For now, we'll create a user if one doesn't exist
    let (access_token, refresh_token) = match provider {
        "vk" => {
            // VK OAuth
            let resp = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .post(&format!(
                    "{}/oauth/token",
                    config.vk_oauth_endpoint()
                ))
                .header("Authorization", &format!("Bearer {}", config.vk_service_token()))
                .form(&[
                    ("code", code),
                    ("client_id", &config.vk_app_id()),
                    ("client_secret", &config.vk_app_secret()),
                    ("redirect_uri", "https://flowlink.flow-masters.ru/api/auth/callback/vk"),
                    ("grant_type", "authorization_code"),
                ])
                .send()
                .await
                .map_err(|e| AuthError::Provider(e.to_string()))?;

            // Parse response
            let json = resp.json().map_err(|e| AuthError::Provider(e.to_string()))?;

            let access_token = json["access_token"]
                .as_str()
                .ok_or_else(|| return Err(AuthError::Provider("No access_token in VK response".to_string()))?
                .to_string();

            let refresh_token = json["refresh_token"]
                .as_str()
                .ok_or_else(|| return Err(AuthError::Provider("No refresh_token in VK response".to_string()))?
                .to_string();

            (access_token, refresh_token)
        }
        _ => return Err(AuthError::Provider(format!("Unsupported provider: {}", provider))),
    };

    // Check/create user account
    let (account_id, email) = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT account_id, email FROM accounts WHERE email = (SELECT email FROM oauth_providers WHERE provider = $1 AND code = $2 LIMIT 1)"
    )
    .bind(provider, code)
    .fetch_optional(db.pool())
    .await
    .map_err(|e| AuthError::Database(e.to_string()))?
    .ok_or_else(|| {
        // Create new account
        let new_account_id = uuid::Uuid::new_v4();
        
        sqlx::query::<_, _>(
            "INSERT INTO accounts (account_id, email) VALUES ($1, 'oauth_user_${new_account_id}')"
        )
        .bind(new_account_id)
        .execute(db.pool())
        .await
        .map_err(|e| AuthError::Database(e.to_string()))?;

        info!("Created new account {} for OAuth provider {}", new_account_id, provider);

        (new_account_id.clone(), format!("oauth_user_{}", new_account_id))
    })?;

    Ok((access_token, email.clone()))
}
