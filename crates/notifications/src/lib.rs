//! FlowLink notification system — email, telegram, preferences, notification routing.
//!
//! This crate provides:
//! - `NotificationRouter` — extensible channel-based notification dispatch
//! - `NotificationChannel` trait — for adding new channels (Telegram, Slack, etc.)
//! - `Severity` / `Category` — notification classification
//! - Email service (`EmailService`) — SMTP-based email sending
//! - Email queue (`EmailQueue`) — scheduled email delivery with templates
//! - `api` — REST API handlers for channel management
//! - `preferences` — In-memory notification store and preferences API

pub mod router;
pub mod email;
pub mod email_queue;
pub mod api;
pub mod preferences;

// Re-export key types
pub use router::*;
pub use email::EmailService;
pub use email_queue::EmailQueue;
pub use api::{NotificationState, error_response};
pub use preferences::{NotificationStore, Notification as UserNotification};
