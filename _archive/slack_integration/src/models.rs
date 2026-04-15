use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use chrono::{DateTime, Utc};

// Slack-specific models and types

// Base Slack models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackChannel {
    pub id: String,
    pub name: String,
    pub name_normalized: String,
    pub is_shared: bool,
    pub is_org_shared: bool,
    pub is_channel: bool,
    pub is_group: bool,
    pub is_im: bool,
    pub is_mpim: bool,
    pub is_private: bool,
    pub is_thread_only: bool,
    pub creator: String,
    pub created: DateTime<Utc>,
    pub is_archived: bool,
    pub is_general: bool,
    pub unlinked_count: i32,
    pub last_read: Option<DateTime<Utc>>,
    pub context_team_id: Option<String>,
    pub context_message_id: Option<String>,
    pub priority: f32,
    pub previous_names: Option<Vec<String>>,
    pub num_members: Option<i32>,
    pub topic: Option<SlackChannelTopic>,
    pub purpose: Option<SlackChannelTopic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackChannelTopic {
    pub value: String,
    pub creator: String,
    pub last_set: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackUser {
    pub id: String,
    pub team_id: String,
    pub name: String,
    pub deleted: bool,
    pub color: String,
    pub real_name: String,
    pub tz: String,
    pub tz_label: String,
    pub tz_offset: i32,
    pub profile: SlackUserProfile,
    pub is_restricted: bool,
    pub is_ultra_restricted: bool,
    pub is_bot: bool,
    pub updated: DateTime<Utc>,
    pub is_admin: bool,
    pub is_owner: bool,
    pub is_primary_owner: bool,
    pub is_restricted: bool,
    pub is_ultra_restricted: bool,
    pub is_bot: bool,
    pub app_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackUserProfile {
    pub avatar_hash: Option<String>,
    pub status_text: Option<String>,
    pub status_emoji: Option<String>,
    pub status_expiration: Option<i64>,
    pub real_name_normalized: String,
    pub display_name_normalized: String,
    pub email: Option<String>,
    pub team: Option<String>,
    pub image_original: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub image_24: Option<String>,
    pub image_48: Option<String>,
    pub image_72: Option<String>,
    pub image_192: Option<String>,
    pub image_512: Option<String>,
    pub image_1024: Option<String>,
    pub title: Option<String>,
    pub phone: Option<String>,
    pub skype: Option<String>,
    pub real_name: String,
    pub display_name: String,
}

// FlowLink-specific models for Slack integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    pub bot: SlackBotConfig,
    pub webhook: SlackWebhookConfig,
    pub flowlink_endpoint: String,
    pub app_id: String,
    pub sign_secret: String,
    pub allowed_channels: Vec<String>,
    pub approval_channel: String,
    pub admin_users: Vec<String>,
}

impl Default for SlackConfig {
    fn default() -> Self {
        Self {
            bot: SlackBotConfig::default(),
            webhook: SlackWebhookConfig::default(),
            flowlink_endpoint: "http://localhost:8080".to_string(),
            app_id: "".to_string(),
            sign_secret: "".to_string(),
            allowed_channels: vec![],
            approval_channel: "#approvals".to_string(),
            admin_users: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackBotConfig {
    pub bot_token: String,
    pub app_token: String,
    pub signing_secret: String,
    pub allowed_channels: Vec<String>,
    pub approval_channel: String,
}

impl Default for SlackBotConfig {
    fn default() -> Self {
        Self {
            bot_token: "".to_string(),
            app_token: "".to_string(),
            signing_secret: "".to_string(),
            allowed_channels: vec![],
            approval_channel: "#approvals".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackWebhookConfig {
    pub port: u16,
    pub signing_secret: String,
    pub verification_token: String,
}

impl Default for SlackWebhookConfig {
    fn default() -> Self {
        Self {
            port: 3001,
            signing_secret: "".to_string(),
            verification_token: "".to_string(),
        }
    }
}

// Slack event models
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SlackEvent {
    #[serde(rename = "url_verification")]
    UrlVerification { challenge: String },
    #[serde(rename = "event_callback")]
    EventCallback(SlackEventCallback),
    #[serde(rename = "event")]
    Message(SlackMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackEventCallback {
    pub token: String,
    pub team_id: String,
    pub api_app_id: String,
    pub event: SlackEventPayload,
    pub type_: String,
    pub authed_users: Option<Vec<String>>,
    pub authorizations: Option<Vec<SlackAuthorization>>,
    pub is_ext_shared_channel: bool,
    pub event_id: String,
    pub event_time: i64,
    pub payload: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackAuthorization {
    pub enterprise_id: Option<String>,
    pub enterprise_name: Option<String>,
    pub team_id: String,
    pub team_name: String,
    pub user_id: String,
    pub is_bot: bool,
    pub bot_id: Option<String>,
    pub bot_user_id: Option<String>,
    pub bot_scopes: Option<Vec<String>>,
    pub bot_access_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SlackEventPayload {
    #[serde(rename = "message")]
    Message(SlackMessage),
    #[serde(rename = "app_mention")]
    AppMention(SlackMessage),
    #[serde(rename = "interactive_message")]
    InteractiveMessage(SlackInteractiveMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackMessage {
    pub bot_id: Option<String>,
    pub type_: String,
    pub text: Option<String>,
    pub user: String,
    pub team: String,
    pub channel: String,
    pub ts: String,
    pub thread_ts: Option<String>,
    pub parent_user_id: Option<String>,
    pub app_id: Option<String>,
    pub bot_profile: Option<SlackBotProfile>,
    pub display_as_bot: bool,
    pub x_original_user: Option<String>,
    pub attachments: Option<Vec<SlackAttachment>>,
    pub files: Option<Vec<SlackFile>>,
    pub pinned_to: Option<Vec<String>>,
    pub edited: Option<SlackMessageEdited>,
    pub reactions: Option<Vec<SlackReaction>>,
    pub last_read: Option<String>,
    pub subscribed: Option<bool>,
    pub unread_count: Option<i32>,
    pub unread_count_display: Option<i32>,
    pub mrkdwn_in: Option<Vec<String>>,
    pub submessages: Option<Vec<SlackMessage>>,
    pub previous_reading: Option<String>,
    pub bot_access_token: Option<String>,
    pub welcome_message: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackBotProfile {
    pub id: String,
    pub app_id: String,
    pub name: String,
    pub icons: SlackBotIcons,
    pub updated: i64,
    pub team_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackBotIcons {
    pub image_36: String,
    pub image_48: String,
    pub image_72: String,
    pub image_192: String,
    pub image_512: String,
    pub image_1024: Option<String>,
    pub image_original: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackAttachment {
    pub id: Option<i64>,
    pub fallback: Option<String>,
    pub color: Option<String>,
    pub pretext: Option<String>,
    pub author_name: Option<String>,
    pub author_link: Option<String>,
    pub author_icon: Option<String>,
    pub title: Option<String>,
    pub title_link: Option<String>,
    pub text: Option<String>,
    pub fields: Option<Vec<SlackAttachmentField>>,
    pub image_url: Option<String>,
    pub image_width: Option<i32>,
    pub image_height: Option<i32>,
    pub thumb_url: Option<String>,
    pub thumb_width: Option<i32>,
    pub thumb_height: Option<i32>,
    pub footer: Option<String>,
    pub footer_icon: Option<String>,
    pub ts: Option<i64>,
    pub mrkdwn_in: Option<Vec<String>>,
    pub actions: Option<Vec<SlackAction>>,
    pub callback_id: Option<String>,
    pub context: Option<SlackMessageContext>,
    pub app_unfurl_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackAttachmentField {
    pub title: Option<String>,
    pub value: String,
    pub short: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackAction {
    pub name: String,
    pub text: String,
    pub type_: String,
    pub value: Option<String>,
    pub style: Option<String>,
    pub url: Option<String>,
    pub confirm: Option<SlackConfirm>,
    pub options: Option<Vec<SlackOption>>,
    pub selected_options: Option<Vec<SlackOption>>,
    pub option_groups: Option<Vec<SlackOptionGroup>>,
    pub data_source: Option<String>,
    pub min_query_length: Option<i32>,
    pub selected_option: Option<SlackOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfirm {
    pub text: String,
    pub title: Option<String>,
    pub ok_text: Option<String>,
    pub dismiss_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackOption {
    pub text: String,
    pub value: String,
    pub description: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackOptionGroup {
    pub label: String,
    pub options: Vec<SlackOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackMessageContext {
    pub message_id: String,
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackFile {
    pub id: String,
    pub created: i64,
    pub timestamp: i64,
    pub name: String,
    pub title: Option<String>,
    pub mimetype: Option<String>,
    pub filetype: Option<String>,
    pub pretty_type: Option<String>,
    pub user: String,
    pub editable: bool,
    pub size: i64,
    pub mode: String,
    pub is_external: bool,
    pub external_type: Option<String>,
    pub has_rich_preview: bool,
    pub is_public: bool,
    pub public_url_shared: bool,
    pub display_as_bot: bool,
    pub username: Option<String>,
    pub url_private: Option<String>,
    pub url_private_download: Option<String>,
    pub permalink: Option<String>,
    pub permalink_public: Option<String>,
    pub edit_link: Option<String>,
    pub preview: Option<String>,
    pub preview_hightlight: Option<String>,
    pub lines: Option<i32>,
    pub lines_more: Option<i32>,
    pub image_exif_rotation: Option<i32>,
    pub thumb_64: Option<String>,
    pub thumb_80: Option<String>,
    pub thumb_160: Option<String>,
    pub thumb_720: Option<String>,
    pub thumb_800: Option<String>,
    pub thumb_960: Option<String>,
    pub thumb_1024: Option<String>,
    pub original_w: Option<i32>,
    pub original_h: Option<i32>,
    pub thumb_w: Option<i32>,
    pub thumb_h: Option<i32>,
    pub video_info: Option<SlackVideoInfo>,
    pub groups: Option<Vec<String>>,
    pub channels: Option<Vec<String>>,
    pub initial_comment: Option<SlackComment>,
    pub comments: Option<Vec<SlackComment>>,
    pub num_stars: Option<i32>,
    pub is_starred: Option<bool>,
    pub shares: Option<HashMap<String, Vec<SlackShare>>>,
    pub channel_actions_ts: Option<String>,
    pub is_sharenable: bool,
    pub has_more_shares: bool,
    pub original_width: Option<i32>,
    pub original_height: Option<i32>,
    pub permalink: Option<String>,
    pub permalink_public: Option<String>,
    pub edit_link: Option<String>,
    pub preview: Option<String>,
    pub preview_hightlight: Option<String>,
    pub lines: Option<i32>,
    pub lines_more: Option<i32>,
    pub image_exif_rotation: Option<i32>,
    pub thumb_64: Option<String>,
    pub thumb_80: Option<String>,
    pub thumb_160: Option<String>,
    pub thumb_720: Option<String>,
    pub thumb_800: Option<String>,
    pub thumb_960: Option<String>,
    pub thumb_1024: Option<String>,
    pub original_w: Option<i32>,
    pub original_h: Option<i32>,
    pub thumb_w: Option<i32>,
    pub thumb_h: Option<i32>,
    pub video_info: Option<SlackVideoInfo>,
    pub groups: Option<Vec<String>>,
    pub channels: Option<Vec<String>>,
    pub initial_comment: Option<SlackComment>,
    pub comments: Option<Vec<SlackComment>>,
    pub num_stars: Option<i32>,
    pub is_starred: Option<bool>,
    pub shares: Option<HashMap<String, Vec<SlackShare>>>,
    pub channel_actions_ts: Option<String>,
    pub is_sharenable: bool,
    pub has_more_shares: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackVideoInfo {
    pub duration: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub url_private: Option<String>,
    pub thumb_mp4: Option<String>,
    pub thumb_jpeg: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackComment {
    pub id: String,
    pub created: i64,
    pub timestamp: i64,
    pub user: String,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackShare {
    pub reply_users: Option<Vec<String>>,
    pub reply_count: Option<i32>,
    pub ts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackMessageEdited {
    pub user: Option<String>,
    pub ts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackReaction {
    pub type_: String,
    pub count: i32,
    pub users: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackInteractiveMessage {
    pub id: String,
    pub callback_id: String,
    pub type_: String,
    pub actions: Vec<SlackInteractiveAction>,
    pub original_message: SlackMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackInteractiveAction {
    pub name: String,
    pub type_: String,
    pub value: Option<String>,
    pub selected_options: Option<Vec<SlackOption>>,
    pub options: Option<Vec<SlackOption>>,
    pub confirm: Option<SlackConfirm>,
}

// FlowLink-specific models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackNotification {
    pub title: String,
    pub message: String,
    pub channel: String,
    pub color: String,
    pub timestamp: DateTime<Utc>,
    pub priority: NotificationPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub request_id: String,
    pub agent_id: String,
    pub command: String,
    pub user_id: String,
    pub team_id: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub agent_id: String,
    pub command: String,
    pub output: String,
    pub success: bool,
    pub exit_code: i32,
    pub channel: String,
    pub user: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackIntegrationMetrics {
    pub total_commands: i64,
    pub successful_commands: i64,
    pub failed_commands: i64,
    pub approvals_requested: i64,
    pub approvals_given: i64,
    pub approvals_rejected: i64,
    pub commands_by_agent: HashMap<String, i64>,
    pub commands_by_user: HashMap<String, i64>,
    pub uptime_days: i64,
    pub last_restart: DateTime<Utc>,
}