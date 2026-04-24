use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPreferences {
    #[serde(default)]
    push_enabled: bool,
    #[serde(default)]
    push_channels: Vec<String>, // "telegram", "email", "sms"
    #[serde(default)]
    email_frequency: String, // "immediate", "daily", "weekly", "never"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountPreferences {
    #[serde(default)]
    language: String, // "ru", "en", "auto"
    #[serde(default)]
    theme: String, // "light", "dark", "auto"
    #[serde(default)]
    timezone: String, // "Europe/Moscow", "Europe/London", "auto"
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── NotificationPreferences ─────────────────────────────────────

    #[test]
    fn notification_preferences_default_construction() {
        let prefs: NotificationPreferences = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(!prefs.push_enabled);
        assert!(prefs.push_channels.is_empty());
        assert!(prefs.email_frequency.is_empty());
    }

    #[test]
    fn notification_preferences_with_all_fields() {
        let prefs: NotificationPreferences = serde_json::from_value(serde_json::json!({
            "push_enabled": true,
            "push_channels": ["telegram", "email"],
            "email_frequency": "daily"
        })).unwrap();
        assert!(prefs.push_enabled);
        assert_eq!(prefs.push_channels, vec!["telegram", "email"]);
        assert_eq!(prefs.email_frequency, "daily");
    }

    #[test]
    fn notification_preferences_serde_roundtrip() {
        let prefs = NotificationPreferences {
            push_enabled: true,
            push_channels: vec!["telegram".to_string(), "sms".to_string()],
            email_frequency: "immediate".to_string(),
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let deserialized: NotificationPreferences = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.push_enabled, prefs.push_enabled);
        assert_eq!(deserialized.push_channels, prefs.push_channels);
        assert_eq!(deserialized.email_frequency, prefs.email_frequency);
    }

    #[test]
    fn notification_preferences_clone() {
        let prefs = NotificationPreferences {
            push_enabled: true,
            push_channels: vec!["email".to_string()],
            email_frequency: "weekly".to_string(),
        };
        let cloned = prefs.clone();
        assert_eq!(cloned.push_enabled, prefs.push_enabled);
        assert_eq!(cloned.push_channels, prefs.push_channels);
        assert_eq!(cloned.email_frequency, prefs.email_frequency);
    }

    #[test]
    fn notification_preferences_empty_push_channels() {
        let prefs: NotificationPreferences = serde_json::from_value(serde_json::json!({
            "push_enabled": false,
            "push_channels": [],
            "email_frequency": "never"
        })).unwrap();
        assert!(prefs.push_channels.is_empty());
        assert_eq!(prefs.email_frequency, "never");
    }

    // ── AccountPreferences ──────────────────────────────────────────

    #[test]
    fn account_preferences_default_construction() {
        let prefs: AccountPreferences = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(prefs.language.is_empty());
        assert!(prefs.theme.is_empty());
        assert!(prefs.timezone.is_empty());
    }

    #[test]
    fn account_preferences_with_all_fields() {
        let prefs: AccountPreferences = serde_json::from_value(serde_json::json!({
            "language": "en",
            "theme": "dark",
            "timezone": "UTC"
        })).unwrap();
        assert_eq!(prefs.language, "en");
        assert_eq!(prefs.theme, "dark");
        assert_eq!(prefs.timezone, "UTC");
    }

    #[test]
    fn account_preferences_serde_roundtrip() {
        let prefs = AccountPreferences {
            language: "ru".to_string(),
            theme: "light".to_string(),
            timezone: "Europe/Moscow".to_string(),
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let deserialized: AccountPreferences = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.language, prefs.language);
        assert_eq!(deserialized.theme, prefs.theme);
        assert_eq!(deserialized.timezone, prefs.timezone);
    }

    #[test]
    fn account_preferences_clone() {
        let prefs = AccountPreferences {
            language: "auto".to_string(),
            theme: "auto".to_string(),
            timezone: "auto".to_string(),
        };
        let cloned = prefs.clone();
        assert_eq!(cloned.language, prefs.language);
        assert_eq!(cloned.theme, prefs.theme);
        assert_eq!(cloned.timezone, prefs.timezone);
    }

    #[test]
    fn account_preferences_auto_values() {
        let prefs: AccountPreferences = serde_json::from_value(serde_json::json!({
            "language": "auto",
            "theme": "auto",
            "timezone": "auto"
        })).unwrap();
        assert_eq!(prefs.language, "auto");
        assert_eq!(prefs.theme, "auto");
        assert_eq!(prefs.timezone, "auto");
    }
}
