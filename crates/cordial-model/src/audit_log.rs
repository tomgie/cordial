//! Audit logs.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::auto_moderation::AutoModerationRule;
use crate::channel::Channel;
use crate::command::Command;
use crate::guild::Integration;
use crate::id::{ApplicationId, AuditLogEntryId, ChannelId, GenericId, MessageId, RoleId, UserId};
use crate::scheduled_event::ScheduledEvent;
use crate::user::User;
use crate::webhook::Webhook;

/// The audit log of a guild, with the objects that its entries refer to.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AuditLog {
    /// The application commands that the entries refer to.
    #[serde(default)]
    pub application_commands: Vec<Command>,
    /// The entries, from most to least recent.
    #[serde(default)]
    pub audit_log_entries: Vec<AuditLogEntry>,
    /// The auto moderation rules that the entries refer to.
    #[serde(default)]
    pub auto_moderation_rules: Vec<AutoModerationRule>,
    /// The scheduled events that the entries refer to.
    #[serde(default)]
    pub guild_scheduled_events: Vec<ScheduledEvent>,
    /// The partial integrations that the entries refer to.
    #[serde(default)]
    pub integrations: Vec<Integration>,
    /// The threads that the entries refer to.
    #[serde(default)]
    pub threads: Vec<Channel>,
    /// The users that the entries refer to.
    #[serde(default)]
    pub users: Vec<User>,
    /// The webhooks that the entries refer to.
    #[serde(default)]
    pub webhooks: Vec<Webhook>,
}

/// An entry of an audit log.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AuditLogEntry {
    /// The ID of the affected object.
    #[serde(default)]
    pub target_id: Option<String>,
    /// The changes to the object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<Vec<AuditLogChange>>,
    /// The user or application that made the change.
    #[serde(default)]
    pub user_id: Option<UserId>,
    /// The entry ID.
    pub id: AuditLogEntryId,
    /// The action.
    pub action_type: AuditLogEvent,
    /// More data for some actions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<AuditLogEntryInfo>,
    /// The reason, 1 to 512 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl AuditLogEntry {
    /// Gives the target ID as an ID of a kind, when it is a snowflake.
    #[must_use]
    pub fn target<T>(&self) -> Option<crate::Id<T>> {
        self.target_id.as_deref()?.parse().ok()
    }
}

crate::discord_enum! {
    /// The action of an audit log entry.
    pub enum AuditLogEvent: u16 {
        /// The guild settings changed.
        GuildUpdate = 1,
        /// A channel was made.
        ChannelCreate = 10,
        /// A channel changed.
        ChannelUpdate = 11,
        /// A channel was deleted.
        ChannelDelete = 12,
        /// A permission overwrite was added.
        ChannelOverwriteCreate = 13,
        /// A permission overwrite changed.
        ChannelOverwriteUpdate = 14,
        /// A permission overwrite was deleted.
        ChannelOverwriteDelete = 15,
        /// Channels were sorted again.
        ChannelPositionUpdate = 16,
        /// A member was kicked.
        MemberKick = 20,
        /// Members were pruned.
        MemberPrune = 21,
        /// A member was banned.
        MemberBanAdd = 22,
        /// A ban was removed.
        MemberBanRemove = 23,
        /// A member changed.
        MemberUpdate = 24,
        /// Roles of a member changed.
        MemberRoleUpdate = 25,
        /// A member was moved to another voice channel.
        MemberMove = 26,
        /// A member was disconnected from voice.
        MemberDisconnect = 27,
        /// A bot was added.
        BotAdd = 28,
        /// A role was made.
        RoleCreate = 30,
        /// A role changed.
        RoleUpdate = 31,
        /// A role was deleted.
        RoleDelete = 32,
        /// Roles were sorted again.
        RolePositionUpdate = 33,
        /// An invite was made.
        InviteCreate = 40,
        /// An invite changed.
        InviteUpdate = 41,
        /// An invite was deleted.
        InviteDelete = 42,
        /// A webhook was made.
        WebhookCreate = 50,
        /// A webhook changed.
        WebhookUpdate = 51,
        /// A webhook was deleted.
        WebhookDelete = 52,
        /// An emoji was made.
        EmojiCreate = 60,
        /// An emoji changed.
        EmojiUpdate = 61,
        /// An emoji was deleted.
        EmojiDelete = 62,
        /// A message was deleted.
        MessageDelete = 72,
        /// Messages were deleted in bulk.
        MessageBulkDelete = 73,
        /// A message was pinned.
        MessagePin = 74,
        /// A message was unpinned.
        MessageUnpin = 75,
        /// An application was added.
        IntegrationCreate = 80,
        /// An application changed.
        IntegrationUpdate = 81,
        /// An application was removed.
        IntegrationDelete = 82,
        /// A stage became live.
        StageInstanceCreate = 83,
        /// A stage changed.
        StageInstanceUpdate = 84,
        /// A stage ended.
        StageInstanceDelete = 85,
        /// A sticker was made.
        StickerCreate = 90,
        /// A sticker changed.
        StickerUpdate = 91,
        /// A sticker was deleted.
        StickerDelete = 92,
        /// A scheduled event was made.
        GuildScheduledEventCreate = 100,
        /// A scheduled event changed.
        GuildScheduledEventUpdate = 101,
        /// A scheduled event was canceled.
        GuildScheduledEventDelete = 102,
        /// A thread was made.
        ThreadCreate = 110,
        /// A thread changed.
        ThreadUpdate = 111,
        /// A thread was deleted.
        ThreadDelete = 112,
        /// The permissions of a command changed.
        ApplicationCommandPermissionUpdate = 121,
        /// A soundboard sound was made.
        SoundboardSoundCreate = 130,
        /// A soundboard sound changed.
        SoundboardSoundUpdate = 131,
        /// A soundboard sound was deleted.
        SoundboardSoundDelete = 132,
        /// An auto moderation rule was made.
        AutoModerationRuleCreate = 140,
        /// An auto moderation rule changed.
        AutoModerationRuleUpdate = 141,
        /// An auto moderation rule was deleted.
        AutoModerationRuleDelete = 142,
        /// Auto moderation blocked a message.
        AutoModerationBlockMessage = 143,
        /// Auto moderation flagged a message to a channel.
        AutoModerationFlagToChannel = 144,
        /// Auto moderation timed out a member.
        AutoModerationUserCommunicationDisabled = 145,
        /// Auto moderation quarantined a member.
        AutoModerationQuarantineUser = 146,
        /// A creator monetization request was made.
        CreatorMonetizationRequestCreated = 150,
        /// The creator monetization terms were accepted.
        CreatorMonetizationTermsAccepted = 151,
        /// An onboarding question was made.
        OnboardingPromptCreate = 163,
        /// An onboarding question changed.
        OnboardingPromptUpdate = 164,
        /// An onboarding question was deleted.
        OnboardingPromptDelete = 165,
        /// Onboarding was made.
        OnboardingCreate = 166,
        /// Onboarding changed.
        OnboardingUpdate = 167,
        /// The Server Guide was made.
        HomeSettingsCreate = 190,
        /// The Server Guide changed.
        HomeSettingsUpdate = 191,
        /// A user set a voice channel status.
        VoiceChannelStatusCreate = 192,
        /// A user deleted a voice channel status.
        VoiceChannelStatusDelete = 193,
    }
}

/// More data of an audit log entry for some actions. Discord sends the numbers as strings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AuditLogEntryInfo {
    /// The application whose command permissions changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
    /// The name of the auto moderation rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_moderation_rule_name: Option<String>,
    /// The trigger type of the auto moderation rule, as a string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_moderation_rule_trigger_type: Option<String>,
    /// The channel of the action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<ChannelId>,
    /// The number of targets, or of position updates.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_util::option_string_integer"
    )]
    pub count: Option<u64>,
    /// The number of days of inactivity for a prune.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_util::option_string_integer"
    )]
    pub delete_member_days: Option<u64>,
    /// The overwritten role or member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<GenericId>,
    /// The number of members that a prune removed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_util::option_string_integer"
    )]
    pub members_removed: Option<u64>,
    /// The pinned or unpinned message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<MessageId>,
    /// The name of the overwritten role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_name: Option<String>,
    /// The overwritten kind: `"0"` role, `"1"` member.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// The type of the integration that made the action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration_type: Option<String>,
    /// The new voice channel status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// A change in an audit log entry.
///
/// The value types depend on the key. Use [`AuditLogChange::old_as`] and [`AuditLogChange::new_as`]
/// to decode them.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AuditLogChange {
    /// The name of the changed field. `$add` and `$remove` are role changes of a member.
    pub key: String,
    /// The new value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_value: Option<Value>,
    /// The old value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_value: Option<Value>,
}

impl AuditLogChange {
    /// Decodes the old value as `T`.
    ///
    /// # Errors
    ///
    /// Gives an error when the value does not match `T`.
    pub fn old_as<T: DeserializeOwned>(&self) -> Result<Option<T>, serde_json::Error> {
        self.old_value
            .clone()
            .map(serde_json::from_value)
            .transpose()
    }

    /// Decodes the new value as `T`.
    ///
    /// # Errors
    ///
    /// Gives an error when the value does not match `T`.
    pub fn new_as<T: DeserializeOwned>(&self) -> Result<Option<T>, serde_json::Error> {
        self.new_value
            .clone()
            .map(serde_json::from_value)
            .transpose()
    }

    /// For `$add` and `$remove`: gives the roles that were added or removed.
    #[must_use]
    pub fn roles(&self) -> Vec<AuditLogRole> {
        self.new_as::<Vec<AuditLogRole>>()
            .ok()
            .flatten()
            .unwrap_or_default()
    }
}

/// A role in a `$add` or `$remove` change.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AuditLogRole {
    /// The role ID.
    pub id: RoleId,
    /// The role name.
    pub name: String,
}
