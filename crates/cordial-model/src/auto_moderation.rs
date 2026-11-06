//! Auto moderation rules.

use serde::{Deserialize, Serialize};

use crate::id::{AutoModerationRuleId, ChannelId, GuildId, RoleId, UserId};

/// An auto moderation rule.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AutoModerationRule {
    /// The rule ID.
    pub id: AutoModerationRuleId,
    /// The guild ID.
    pub guild_id: GuildId,
    /// The name.
    pub name: String,
    /// The user that made the rule.
    pub creator_id: UserId,
    /// When the rule runs.
    pub event_type: AutoModerationEventType,
    /// What the rule checks.
    pub trigger_type: AutoModerationTriggerType,
    /// The data of the trigger.
    #[serde(default)]
    pub trigger_metadata: AutoModerationTriggerMetadata,
    /// The actions when the rule triggers.
    #[serde(default)]
    pub actions: Vec<AutoModerationAction>,
    /// Tells if the rule is on.
    #[serde(default)]
    pub enabled: bool,
    /// The roles that the rule does not affect, up to 20.
    #[serde(default)]
    pub exempt_roles: Vec<RoleId>,
    /// The channels that the rule does not affect, up to 50.
    #[serde(default)]
    pub exempt_channels: Vec<ChannelId>,
}

crate::discord_enum! {
    /// What an auto moderation rule checks.
    pub enum AutoModerationTriggerType: u8 {
        /// Words from a list of keywords. Up to 6 for each guild.
        Keyword = 1,
        /// Generic spam. Up to 1 for each guild.
        Spam = 3,
        /// Words from lists that Discord defines. Up to 1 for each guild.
        KeywordPreset = 4,
        /// Too many unique mentions. Up to 1 for each guild.
        MentionSpam = 5,
        /// Words from a list of keywords in member profiles. Up to 1 for each guild.
        MemberProfile = 6,
    }
}

crate::discord_enum! {
    /// When an auto moderation rule runs.
    pub enum AutoModerationEventType: u8 {
        /// A member sends or edits a message.
        MessageSend = 1,
        /// A member edits the profile.
        MemberUpdate = 2,
    }
}

/// The data of an auto moderation trigger. The fields that apply depend on the trigger type.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AutoModerationTriggerMetadata {
    /// Keyword and member profile: substrings to find, up to 1000 of up to 60 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyword_filter: Option<Vec<String>>,
    /// Keyword and member profile: Rust regular expressions, up to 10 of up to 260 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regex_patterns: Option<Vec<String>>,
    /// Keyword preset: the word lists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presets: Option<Vec<KeywordPresetType>>,
    /// Substrings that do not trigger the rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_list: Option<Vec<String>>,
    /// Mention spam: the number of unique mentions allowed in a message, up to 50.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_total_limit: Option<u8>,
    /// Mention spam: tells if mention raids are detected automatically.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_raid_protection_enabled: Option<bool>,
}

crate::discord_enum! {
    /// A word list that Discord defines.
    pub enum KeywordPresetType: u8 {
        /// Swearing or cursing.
        Profanity = 1,
        /// Sexually explicit words.
        SexualContent = 2,
        /// Insults and hate speech.
        Slurs = 3,
    }
}

/// An action of an auto moderation rule.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AutoModerationAction {
    /// The action type.
    #[serde(rename = "type")]
    pub kind: AutoModerationActionType,
    /// The data for the action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<AutoModerationActionMetadata>,
}

impl AutoModerationAction {
    /// Blocks the message, with an optional explanation of up to 150 characters.
    #[must_use]
    pub fn block_message(custom_message: Option<String>) -> Self {
        Self {
            kind: AutoModerationActionType::BlockMessage,
            metadata: custom_message.map(|message| AutoModerationActionMetadata {
                custom_message: Some(message),
                ..AutoModerationActionMetadata::default()
            }),
        }
    }

    /// Sends the content to an alert channel.
    #[must_use]
    pub fn send_alert(channel_id: ChannelId) -> Self {
        Self {
            kind: AutoModerationActionType::SendAlertMessage,
            metadata: Some(AutoModerationActionMetadata {
                channel_id: Some(channel_id),
                ..AutoModerationActionMetadata::default()
            }),
        }
    }

    /// Times out the member, for up to 2419200 seconds (4 weeks).
    #[must_use]
    pub fn timeout(duration_seconds: u32) -> Self {
        Self {
            kind: AutoModerationActionType::Timeout,
            metadata: Some(AutoModerationActionMetadata {
                duration_seconds: Some(duration_seconds),
                ..AutoModerationActionMetadata::default()
            }),
        }
    }
}

crate::discord_enum! {
    /// The type of an auto moderation action.
    pub enum AutoModerationActionType: u8 {
        /// Block the message.
        BlockMessage = 1,
        /// Send the content to a channel.
        SendAlertMessage = 2,
        /// Time out the member. Needs `MODERATE_MEMBERS`.
        Timeout = 3,
        /// Block the member from text, voice, and other interactions.
        BlockMemberInteraction = 4,
    }
}

/// The data of an auto moderation action.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AutoModerationActionMetadata {
    /// Send alert: the channel for the content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<ChannelId>,
    /// Timeout: the duration in seconds, up to 2419200.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u32>,
    /// Block message: the explanation, up to 150 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_message: Option<String>,
}
