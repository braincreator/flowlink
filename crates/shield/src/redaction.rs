//! FlowLink Shield — Output Redaction
//!
//! Filters sensitive data from command output before sending to MCP clients.
//! Catches credentials, API keys, tokens, passwords, private keys, connection strings.
//! Uses regex patterns with context-aware matching (avoids false positives in code).

use once_cell::sync::Lazy;
use regex::Regex;

/// Categories of sensitive data that can be redacted
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RedactionCategory {
    /// AWS Access Key / Secret Key
    AwsKey,
    /// Generic API Key / Token
    ApiKey,
    /// Bearer / JWT Token
    BearerToken,
    /// Password in key=value or connection string
    Password,
    /// Private Key (PEM/OpenSSH)
    PrivateKey,
    /// Connection string (database URLs, etc.)
    ConnectionString,
    /// Generic Secret (config files, env vars)
    Secret,
}

impl std::fmt::Display for RedactionCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AwsKey => write!(f, "aws_key"),
            Self::ApiKey => write!(f, "api_key"),
            Self::BearerToken => write!(f, "bearer_token"),
            Self::Password => write!(f, "password"),
            Self::PrivateKey => write!(f, "private_key"),
            Self::ConnectionString => write!(f, "connection_string"),
            Self::Secret => write!(f, "secret"),
        }
    }
}

/// A single redaction match
#[derive(Debug, Clone)]
pub struct RedactionMatch {
    /// The category of sensitive data
    pub category: RedactionCategory,
    /// The matched text (before redaction)
    pub matched: String,
    /// Start position in original text
    pub start: usize,
    /// End position in original text
    pub end: usize,
}

/// Result of a redaction scan
#[derive(Debug, Clone)]
pub struct RedactionResult {
    /// All matches found
    pub matches: Vec<RedactionMatch>,
    /// Redacted text (matches replaced with placeholder)
    pub redacted: String,
    /// Whether any sensitive data was found
    pub found: bool,
    /// Total number of unique categories
    pub categories: usize,
}

/// Placeholder used to replace redacted content
const REDACT_PLACEHOLDER: &str = "[REDACTED]";

// ── Pattern Definitions ──

/// AWS Access Key ID (AKIA...)
static RE_AWS_ACCESS_KEY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\bAKIA[0-9A-Z]{16}\b").unwrap()
});

/// AWS Secret Access Key (40-char base64-like after key= or secret=)
static RE_AWS_SECRET_KEY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(?:aws_secret_access_key|secret_key|secret)\s*[=:]\s*['"]?([A-Za-z0-9/+=]{40})['"]?"#).unwrap()
});

/// Generic API keys: key=value patterns with hex/base64 tokens (32+ chars)
static RE_API_KEY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(?:api_key|apikey|api_secret|app_key|app_secret|client_secret|token|access_token|auth_token|private_key|secret_key)\s*[=:]\s*['"]?([A-Za-z0-9_\-.]{32,})['"]?"#).unwrap()
});

/// Bearer tokens in HTTP headers
static RE_BEARER_TOKEN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)Bearer\s+[A-Za-z0-9_\-.]{20,}").unwrap()
});

/// JWT tokens (eyJ...)
static RE_JWT_TOKEN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b").unwrap()
});

/// Password in various formats
static RE_PASSWORD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(?:password|passwd|pwd)\s*[=:]\s*['"]?([^\s'\"]{4,})['"]?"#).unwrap()
});

/// Connection strings: postgresql://user:pass@host, mysql://, mongodb://, redis://
static RE_CONNECTION_STRING: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(?:postgresql|mysql|mongodb|redis|postgres|mongo|amqp|mqtt|sqlserver)://[^\s'\"<>]{8,}"#).unwrap()
});

/// Private keys (PEM format)
static RE_PRIVATE_KEY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"-----BEGIN\s+(?:RSA\s+|EC\s+|OPENSSH\s+|DSA\s+)?PRIVATE\s+KEY-----[\s\S]*?-----END\s+(?:RSA\s+|EC\s+|OPENSSH\s+|DSA\s+)?PRIVATE\s+KEY-----").unwrap()
});

/// Generic secret in config files: key = value patterns with sensitive key names
static RE_GENERIC_SECRET: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(?:db_password|database_password|redis_password|smtp_password|mail_password|jwt_secret|signing_key|encryption_key|hmac_secret|webhook_secret|vault_token|sa_key|service_account_key)\s*[=:]\s*['"]?([^\s'\"]{6,})['"]?"#).unwrap()
});

/// GitHub / GitLab personal access tokens (ghp_*, glpat-*)
static RE_GIT_TOKEN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:ghp_|gho_|ghu_|ghs_|glpat-|glptt-|gitlab-ci-token-)[A-Za-z0-9_]{20,}\b").unwrap()
});

/// Slack, Discord, Telegram bot tokens
static RE_CHAT_TOKEN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:xox[bpas]-[A-Za-z0-9-]{20,})\b").unwrap()
});

/// Stripe / payment tokens
static RE_PAYMENT_TOKEN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:sk_live_|sk_test_|pk_live_|pk_test_)[A-Za-z0-9]{20,}\b").unwrap()
});

// ── Scanner ──

/// Redact sensitive data from text output
pub fn redact(text: &str) -> RedactionResult {
    let mut matches: Vec<RedactionMatch> = Vec::new();

    // AWS Access Key
    for cap in RE_AWS_ACCESS_KEY.find_iter(text) {
        matches.push(RedactionMatch {
            category: RedactionCategory::AwsKey,
            matched: cap.as_str().to_string(),
            start: cap.start(),
            end: cap.end(),
        });
    }

    // AWS Secret Key
    for cap in RE_AWS_SECRET_KEY.find_iter(text) {
        matches.push(RedactionMatch {
            category: RedactionCategory::AwsKey,
            matched: cap.as_str().to_string(),
            start: cap.start(),
            end: cap.end(),
        });
    }

    // API Keys
    for cap in RE_API_KEY.find_iter(text) {
        matches.push(RedactionMatch {
            category: RedactionCategory::ApiKey,
            matched: cap.as_str().to_string(),
            start: cap.start(),
            end: cap.end(),
        });
    }

    // Bearer tokens
    for cap in RE_BEARER_TOKEN.find_iter(text) {
        matches.push(RedactionMatch {
            category: RedactionCategory::BearerToken,
            matched: cap.as_str().to_string(),
            start: cap.start(),
            end: cap.end(),
        });
    }

    // JWT tokens
    for cap in RE_JWT_TOKEN.find_iter(text) {
        matches.push(RedactionMatch {
            category: RedactionCategory::BearerToken,
            matched: cap.as_str().to_string(),
            start: cap.start(),
            end: cap.end(),
        });
    }

    // Passwords
    for cap in RE_PASSWORD.find_iter(text) {
        matches.push(RedactionMatch {
            category: RedactionCategory::Password,
            matched: cap.as_str().to_string(),
            start: cap.start(),
            end: cap.end(),
        });
    }

    // Connection strings
    for cap in RE_CONNECTION_STRING.find_iter(text) {
        matches.push(RedactionMatch {
            category: RedactionCategory::ConnectionString,
            matched: cap.as_str().to_string(),
            start: cap.start(),
            end: cap.end(),
        });
    }

    // Private keys
    for cap in RE_PRIVATE_KEY.find_iter(text) {
        matches.push(RedactionMatch {
            category: RedactionCategory::PrivateKey,
            matched: cap.as_str().to_string(),
            start: cap.start(),
            end: cap.end(),
        });
    }

    // Generic secrets
    for cap in RE_GENERIC_SECRET.find_iter(text) {
        matches.push(RedactionMatch {
            category: RedactionCategory::Secret,
            matched: cap.as_str().to_string(),
            start: cap.start(),
            end: cap.end(),
        });
    }

    // Git tokens
    for cap in RE_GIT_TOKEN.find_iter(text) {
        matches.push(RedactionMatch {
            category: RedactionCategory::ApiKey,
            matched: cap.as_str().to_string(),
            start: cap.start(),
            end: cap.end(),
        });
    }

    // Chat tokens
    for cap in RE_CHAT_TOKEN.find_iter(text) {
        matches.push(RedactionMatch {
            category: RedactionCategory::ApiKey,
            matched: cap.as_str().to_string(),
            start: cap.start(),
            end: cap.end(),
        });
    }

    // Payment tokens
    for cap in RE_PAYMENT_TOKEN.find_iter(text) {
        matches.push(RedactionMatch {
            category: RedactionCategory::ApiKey,
            matched: cap.as_str().to_string(),
            start: cap.start(),
            end: cap.end(),
        });
    }

    // Sort by position and merge overlapping ranges
    matches.sort_by_key(|m| m.start);
    let mut merged: Vec<RedactionMatch> = Vec::new();
    for m in &matches {
        if let Some(last) = merged.last_mut() {
            if m.start <= last.end {
                last.end = last.end.max(m.end);
                continue;
            }
        }
        merged.push(m.clone());
    }

    let categories = merged.iter().map(|m| m.category).collect::<std::collections::HashSet<_>>().len();

    // Build redacted string — replace from end to start to preserve offsets
    let bytes = text.as_bytes();
    let mut redacted_bytes: Vec<u8> = Vec::from(bytes);
    for m in merged.iter().rev() {
        let placeholder = REDACT_PLACEHOLDER.as_bytes();
        redacted_bytes.splice(m.start..m.end, placeholder.iter().copied());
    }
    let redacted = String::from_utf8(redacted_bytes).unwrap_or_else(|_| text.to_string());

    let found = !matches.is_empty();

    RedactionResult {
        matches,
        redacted,
        found,
        categories,
    }
}

/// Quick check if text contains any sensitive data (without building full result)
pub fn contains_secrets(text: &str) -> bool {
    RE_AWS_ACCESS_KEY.is_match(text)
        || RE_AWS_SECRET_KEY.is_match(text)
        || RE_API_KEY.is_match(text)
        || RE_BEARER_TOKEN.is_match(text)
        || RE_JWT_TOKEN.is_match(text)
        || RE_PASSWORD.is_match(text)
        || RE_CONNECTION_STRING.is_match(text)
        || RE_PRIVATE_KEY.is_match(text)
        || RE_GENERIC_SECRET.is_match(text)
        || RE_GIT_TOKEN.is_match(text)
        || RE_CHAT_TOKEN.is_match(text)
        || RE_PAYMENT_TOKEN.is_match(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_aws_key() {
        let input = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE";
        let result = redact(input);
        assert!(result.found);
        assert!(result.redacted.contains("[REDACTED]"));
        assert!(!result.redacted.contains("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn test_redact_password() {
        let input = "DB_PASSWORD=super_secret_pass123";
        let result = redact(input);
        assert!(result.found);
        assert!(result.redacted.contains("[REDACTED]"));
        assert!(!result.redacted.contains("super_secret_pass123"));
    }

    #[test]
    fn test_redact_jwt() {
        let input = "token: eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        let result = redact(input);
        assert!(result.found);
    }

    #[test]
    fn test_redact_bearer_token() {
        let input = "Authorization: Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxIn0";
        let result = redact(input);
        assert!(result.found);
    }

    #[test]
    fn test_redact_connection_string() {
        let input = "DATABASE_URL=postgresql://admin:password123@db.example.com:5432/mydb";
        let result = redact(input);
        assert!(result.found);
        assert!(result.redacted.contains("[REDACTED]"));
    }

    #[test]
    fn test_redact_private_key() {
        let input = "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQ\n-----END RSA PRIVATE KEY-----";
        let result = redact(input);
        assert!(result.found);
    }

    #[test]
    fn test_redact_gitlab_token() {
        let input = "CI_JOB_TOKEN=glpat-xxxxxxxxxxxxxxxxxxxx";
        let result = redact(input);
        assert!(result.found);
    }

    #[test]
    fn test_redact_stripe_key() {
        let input = "sk_live_51AbCdEf1234567890abcdefghijklmnop";
        let result = redact(input);
        assert!(result.found);
    }

    #[test]
    fn test_no_false_positive_safe_text() {
        let input = "The server returned status 200 OK. All systems operational. No errors found.";
        let result = redact(input);
        assert!(!result.found);
    }

    #[test]
    fn test_no_false_positive_code() {
        let input = r#"fn main() {
    let config = Config::new();
    let response = client.get("/api/health").send()?;
    println!("status: {}", response.status());
}"#;
        let result = redact(input);
        assert!(!result.found);
    }

    #[test]
    fn test_redact_env_file() {
        let input = r#"DATABASE_URL=postgresql://admin:secret@localhost:5432/prod
AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
JWT_SECRET=my-super-secret-jwt-key-12345
SLACK_BOT_TOKEN=xoxb-1234567890-ABCDEFGHIJKLMNOP
STRIPE_KEY=sk_live_51AbCdEf1234567890abcdefghijklmnop"#;
        let result = redact(input);
        assert!(result.found);
        assert!(result.categories >= 4); // connection string, aws, secret, chat token, payment
        assert!(!result.redacted.contains("secret@localhost"));
        assert!(!result.redacted.contains("wJalrXUtnFEMI"));
    }

    #[test]
    fn test_contains_secrets_quick() {
        assert!(contains_secrets("password=hunter2"));
        assert!(!contains_secrets("hello world"));
        assert!(contains_secrets("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn test_multiple_matches_sorted() {
        let input = "key1=AKIAIOSFODNN7EXAMPLE and key2=super_secret_value";
        let result = redact(input);
        assert!(result.found);
        assert!(result.redacted.contains("[REDACTED]"));
    }
}
