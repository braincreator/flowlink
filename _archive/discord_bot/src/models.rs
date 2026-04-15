use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Discord-specific models and types

// User roles and permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordRole {
    pub id: String,
    pub name: String,
    pub color: u32,
    pub permissions: String,
    pub position: i32,
    pub hoist: bool,
    pub managed: bool,
    pub mentionable: bool,
}

// Discord member with roles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordMember {
    pub user: DiscordUser,
    pub nick: Option<String>,
    pub roles: Vec<String>,
    pub joined_at: String,
    pub premium_since: Option<String>,
    pub deaf: bool,
    pub mute: bool,
    pub flags: i32,
    pub communication_disabled_until: Option<String>,
}

// Discord reaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordReaction {
    pub count: i32,
    pub me: bool,
    pub emoji: DiscordEmoji,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordEmoji {
    pub id: Option<String>,
    pub name: String,
    pub roles: Option<Vec<String>>,
    pub user: Option<DiscordUser>,
    pub require_colons: Option<bool>,
    pub managed: Option<bool>,
    pub animated: Option<bool>,
    pub available: Option<bool>,
}

// Discord components for interactive messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DiscordComponent {
    #[serde(rename = "ACTION_ROW")]
    ActionRow { components: Vec<DiscordComponent> },
    #[serde(rename = "BUTTON")]
    Button {
        style: ButtonStyle,
        label: Option<String>,
        emoji: Option<DiscordEmoji>,
        custom_id: String,
        url: Option<String>,
        disabled: Option<bool>,
    },
    #[serde(rename = "SELECT_MENU")]
    SelectMenu {
        custom_id: String,
        options: Vec<SelectOption>,
        placeholder: Option<String>,
        min_values: Option<u8>,
        max_values: Option<u8>,
        disabled: Option<bool>,
    },
    #[serde(rename = "TEXT_INPUT")]
    TextInput {
        custom_id: String,
        style: TextInputStyle,
        label: String,
        placeholder: Option<String>,
        value: Option<String>,
        required: Option<bool>,
        min_length: Option<u8>,
        max_length: Option<u8>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ButtonStyle {
    Primary = 1,
    Secondary = 2,
    Success = 3,
    Danger = 4,
    Link = 5,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextInputStyle {
    Short = 1,
    Paragraph = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    pub label: String,
    pub value: String,
    pub description: Option<String>,
    pub emoji: Option<DiscordEmoji>,
    pub default: Option<bool>,
}

// Discord audit log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordAuditLog {
    pub webhooks: Vec<DiscordWebhook>,
    pub users: Vec<DiscordUser>,
    pub audit_log_entries: Vec<AuditLogEntry>,
    pub integrations: Vec<Integration>,
    pub threads: Vec<DiscordChannel>,
    pub guild_scheduled_events: Vec<GuildScheduledEvent>,
    pub auto_moderation_rules: Vec<AutoModerationRule>,
    pub guild_hashes: GuildHashes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: String,
    pub user_id: Option<String>,
    pub target_id: Option<String>,
    pub changes: Vec<AuditLogChange>,
    pub user_id: Option<String>,
    pub action_type: AuditLogEventType,
    pub options: Option<HashMap<String, serde_json::Value>>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogChange {
    pub key: String,
    pub new_value: Option<serde_json::Value>,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditLogEventType {
    GuildUpdate,
    ChannelCreate,
    ChannelUpdate,
    ChannelDelete,
    ChannelOverwriteCreate,
    ChannelOverwriteUpdate,
    ChannelOverwriteDelete,
    MemberKick,
    MemberPrune,
    MemberBanAdd,
    MemberBanRemove,
    MemberUpdate,
    MemberRoleUpdate,
    MemberMove,
    MemberDisconnect,
    BotAdd,
    RoleCreate,
    RoleUpdate,
    RoleDelete,
    InviteCreate,
    InviteUpdate,
    InviteDelete,
    WebhookCreate,
    WebhookUpdate,
    WebhookDelete,
    EmojiCreate,
    EmojiUpdate,
    EmojiDelete,
    MessageDelete,
    MessageBulkDelete,
    MessagePin,
    MessageUnpin,
    IntegrationCreate,
    IntegrationUpdate,
    IntegrationDelete,
    StageInstanceCreate,
    StageInstanceUpdate,
    StageInstanceDelete,
    StickerCreate,
    StickerUpdate,
    StickerDelete,
    ThreadCreate,
    ThreadUpdate,
    ThreadDelete,
    ApplicationCommandPermissionUpdate,
    AutoModerationRuleCreate,
    AutoModerationRuleUpdate,
    AutoModerationRuleDelete,
    AutoModerationBlockMessage,
    AutoModerationFlagToContent,
    AutoModerationUserCommunicationDisabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordWebhook {
    pub id: String,
    pub guild_id: Option<String>,
    pub channel_id: String,
    pub user: Option<DiscordUser>,
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub token: Option<String>,
    pub application_id: Option<String>,
    pub source_guild: Option<SourceGuild>,
    pub source_channel: Option<SourceChannel>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub splash: Option<String>,
    pub discovery_splash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceChannel {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Integration {
    pub id: String,
    pub name: String,
    pub type_: String,
    pub enabled: bool,
    pub syncing: bool,
    pub role_id: Option<String>,
    pub enable_emoticons: Option<bool>,
    pub expire_behavior: Option<i32>,
    pub expire_grace_period: Option<i32>,
    pub user: DiscordUser,
    pub account: IntegrationAccount,
    pub synced_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationAccount {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildScheduledEvent {
    pub id: String,
    pub guild_id: String,
    pub channel_id: Option<String>,
    pub creator_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub scheduled_start_time: String,
    pub scheduled_end_time: Option<String>,
    pub privacy_level: i32,
    pub status: i32,
    pub entity_type: i32,
    pub entity_id: Option<String>,
    pub entity_metadata: Option<EntityMetadata>,
    pub creator: Option<DiscordUser>,
    pub user_count: Option<i32>,
    pub image_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMetadata {
    pub location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoModerationRule {
    pub id: String,
    pub guild_id: String,
    pub name: String,
    pub creator_id: String,
    pub event_type: AutoModerationEventType,
    pub trigger_type: AutoModerationTriggerType,
    pub trigger_metadata: TriggerMetadata,
    pub actions: Vec<AutoModerationAction>,
    pub enabled: bool,
    pub exempt_roles: Vec<String>,
    pub exempt_channels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoModerationEventType {
    MessageSend,
    MemberUpdate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoModerationTriggerType {
    Keyword,
    HarmfulLink,
    Spam,
    KeywordPreset,
    MentionSpam,
    MemberProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerMetadata {
    pub keyword_filter: Option<Vec<String>>,
    pub regex_patterns: Option<Vec<String>>,
    pub presets: Option<Vec<AutoModerationPreset>>,
    pub allow_list: Option<Vec<String>>,
    pub mention_total_limit: Option<i32>,
    pub mention_raid_protection_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoModerationPreset {
    Profanity,
    SexualContent,
    Slurs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoModerationAction {
    pub type_: AutoModerationActionType,
    pub metadata: ActionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoModerationActionType {
    BlockMessage,
    SendAlertMessage,
    Timeout,
    BlockInteraction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionMetadata {
    pub channel_id: Option<String>,
    pub duration_seconds: Option<i32>,
    pub custom_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildHashes {
    pub kick: Option<String>,
    pub prune: Option<String>,
    pub ban_add: Option<String>,
    pub ban_remove: Option<String>,
    pub update_channel: Option<String>,
    pub thread_create: Option<String>,
    pub thread_update: Option<String>,
    pub thread_delete: Option<String>,
}