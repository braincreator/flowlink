-- Add preferences_api module
INSERT INTO crates/relay/src/lib.rs (line 1, column 1):
  pub mod preferences_api;

INSERT INTO crates/relay/src/server.rs (line 850, column 1):
  .route("/api/v1/account/notifications", axum::routing::get(crate::preferences_api::get_notifications))

INSERT INTO crates/relay/src/server.rs (line 850, column 1):
  .route("/api/v1/account/preferences", axum::routing::put(crate::preferences_api::update_notifications_preferences))

INSERT INTO crates/relay/src/server.rs (line 850, column 1):
  .route("/api/v1/account/preferences", axum::routing::put(crate::preferences_api::update_account_preferences))
