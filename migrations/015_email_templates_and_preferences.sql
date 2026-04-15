-- Add email templates and preferences tables
CREATE TABLE IF NOT EXISTS notification_preferences (
    account_id TEXT PRIMARY KEY,
    push_enabled BOOLEAN DEFAULT TRUE,
    push_channels TEXT[] DEFAULT '[]',
    email_frequency TEXT DEFAULT 'immediate',
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS account_preferences (
    account_id TEXT PRIMARY KEY,
    language TEXT DEFAULT 'ru',
    theme TEXT DEFAULT 'light',
    timezone TEXT DEFAULT 'Europe/Moscow'
);

-- Add index for faster lookups
CREATE INDEX IF NOT EXISTS idx_notification_prefs_account ON notification_preferences(account_id);
CREATE INDEX IF NOT EXISTS idx_account_prefs_account ON account_preferences(account_id);
