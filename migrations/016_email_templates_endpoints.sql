-- Add endpoints for email templates and preferences

-- Email Templates
INSERT INTO crates/relay/src/email_templates.rs (line 1, column 1):
  (pub async fn send_welcome_email()
   (pub async fn send_payment_success_email()

INSERT INTO crates/relay/src/server.rs (line 850, column 1):
  .route("/api/v1/account/notifications", axum::routing::get(crate::email_templates::get_notifications))

INSERT INTO crates/relay/src/server.rs (line 850, column 1):
  .route("/api/v1/account/preferences", axum::routing::put(crate::email_templates::update_preferences))

-- Preferences
INSERT INTO crates/relay/src/db/preferences.rs (line 1, column 1):
  (pub struct NotificationPreferences
  (pub struct AccountPreferences
