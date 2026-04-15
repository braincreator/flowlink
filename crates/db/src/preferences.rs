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
