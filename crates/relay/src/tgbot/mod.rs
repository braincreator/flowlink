//! Telegram bot module for FlowLink relay.
//!
//! Integrated Telegram bot that runs alongside the relay server.

#[cfg(feature = "tgbot")]
pub mod commands;
#[cfg(feature = "tgbot")]
pub mod bot;
#[cfg(feature = "tgbot")]
pub mod notifications;

#[cfg(feature = "tgbot")]
pub use bot::start_tgbot;
