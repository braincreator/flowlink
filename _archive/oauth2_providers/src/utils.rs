use anyhow::Result;
use rand::Rng;
use base64::{Engine as _, engine::general_purpose::STANDARD_URL_SAFE};

// PKCE (Proof Key for Code Exchange) utilities
pub struct Pkce;

impl Pkce {
    pub fn generate_code_verifier() -> String {
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 32];
        rng.fill_bytes(&mut bytes);

        STANDARD_URL_SAFE.encode(&bytes)
    }

    pub fn generate_code_challenge(verifier: &str) -> String {
        use sha2::{Sha256, Digest};
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

        let hash = Sha256::digest(verifier.as_bytes());
        URL_SAFE_NO_PAD.encode(&hash)
    }

    pub fn generate_code_challenge_with_sha256(verifier: &str) -> String {
        use sha2::{Sha256, Digest};
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

        let hash = Sha256::digest(verifier.as_bytes());
        URL_SAFE_NO_PAD.encode(&hash)
    }
}

// OAuth2 authorization request utilities
pub struct OAuth2Request;

impl OAuth2Request {
    pub fn build_authorization_url(
        provider: &str,
        client_id: &str,
        redirect_uri: &str,
        scopes: &[String],
        state: &str,
    ) -> String {
        let scopes = if !scopes.is_empty() {
            scopes.join(" ")
        } else {
            "openid email profile".to_string()
        };

        let params = [
            ("response_type", "code"),
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
            ("scope", &scopes),
            ("state", state),
        ];

        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(&params)
            .finish();

        format!("https://accounts.google.com/o/oauth2/v2/auth?{}", query)
    }
}

// Token response utilities
pub struct TokenResponse;

impl TokenResponse {
    pub fn extract_access_token(response: &serde_json::Value) -> Result<String> {
        response
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("access_token not found in response"))?
            .to_string()
    }

    pub fn extract_refresh_token(response: &serde_json::Value) -> Result<Option<String>> {
        response
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("refresh_token not found in response"))
    }

    pub fn extract_token_type(response: &serde_json::Value) -> Result<String> {
        response
            .get("token_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("token_type not found in response"))?
            .to_string()
    }

    pub fn extract_expires_in(response: &serde_json::Value) -> Result<i64> {
        response
            .get("expires_in")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("expires_in not found in response"))
    }

    pub fn extract_scope(response: &serde_json::Value) -> Result<Option<String>> {
        response
            .get("scope")
            .and_then(|v| v.as_str())
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("scope not found in response"))
    }
}

// User info response utilities
pub struct UserInfo;

impl UserInfo {
    pub fn extract_user_id(response: &serde_json::Value, provider: &str) -> Result<String> {
        match provider {
            "google" => {
                response
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("user id not found in response"))?
                    .to_string()
            }
            "github" => {
                response
                    .get("id")
                    .and_then(|v| v.as_i64())
                    .map(|i| i.to_string())
                    .ok_or_else(|| anyhow::anyhow!("user id not found in response"))?
            }
            "microsoft" => {
                response
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("user id not found in response"))?
                    .to_string()
            }
            _ => {
                response
                    .get("sub")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("user id not found in response"))?
                    .to_string()
            }
        }
    }

    pub fn extract_email(response: &serde_json::Value) -> Result<Option<String>> {
        response
            .get("email")
            .and_then(|v| v.as_str())
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("email not found in response"))
    }

    pub fn extract_name(response: &serde_json::Value) -> Result<Option<String>> {
        response
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("name not found in response"))
    }

    pub fn extract_given_name(response: &serde_json::Value) -> Result<Option<String>> {
        response
            .get("given_name")
            .and_then(|v| v.as_str())
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("given_name not found in response"))
    }

    pub fn extract_family_name(response: &serde_json::Value) -> Result<Option<String>> {
        response
            .get("family_name")
            .and_then(|v| v.as_str())
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("family_name not found in response"))
    }

    pub fn extract_picture(response: &serde_json::Value) -> Result<Option<String>> {
        response
            .get("picture")
            .and_then(|v| v.as_str())
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("picture not found in response"))
    }

    pub fn extract_locale(response: &serde_json::Value) -> Result<Option<String>> {
        response
            .get("locale")
            .and_then(|v| v.as_str())
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("locale not found in response"))
    }
}

// State token utilities
pub struct StateToken;

impl StateToken {
    pub fn generate() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 32];
        rng.fill_bytes(&mut bytes);
        hex::encode(bytes)
    }

    pub fn create_secure_token(state: String, user_id: &str) -> String {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        use ring::hmac;

        let data = format!("{}:{}", state, user_id);

        let key = hmac::Key::new(hmac::HMAC_SHA256, b"flowlink-oauth2-state");
        let signature = hmac::sign(&key, data.as_bytes());
        let signature_b64 = STANDARD.encode(signature);

        format!("{}.{}", state, signature_b64)
    }

    pub fn verify_secure_token(state: &str, token: &str, user_id: &str) -> bool {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        use ring::hmac;

        let expected_prefix = format!("{}.{}", state, user_id);
        let parts: Vec<&str> = token.split('.').collect();

        if parts.len() != 2 {
            return false;
        }

        let (state_part, signature_part) = (parts[0], parts[1]);

        // Verify the signature
        let key = hmac::Key::new(hmac::HMAC_SHA256, b"flowlink-oauth2-state");
        let signature = hmac::sign(&key, expected_prefix.as_bytes());
        let expected_signature = STANDARD.encode(signature);

        signature_part == expected_signature
    }
}

// Token rotation utilities
pub struct TokenRotation;

impl TokenRotation {
    pub fn should_rotate(access_token: &str) -> bool {
        // Rotate if token contains less than 5 chars
        access_token.len() < 5
    }

    pub fn generate_new_token(prefix: &str) -> String {
        use rand::Rng;
        use rand::distributions::Alphanumeric;
        let mut rng = rand::thread_rng();
        let token: String = (0..32)
            .map(|_| rng.sample(Alphanumeric) as char)
            .collect();
        format!("{}_{}", prefix, token)
    }
}