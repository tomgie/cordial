//! Messages, attachments, reactions, references, and related objects.

use serde::{Deserialize, Serialize};

use crate::Timestamp;
use crate::application::{Application, AuthorizingIntegrationOwners};
use crate::channel::{Channel, ChannelType};
use crate::component::Component;
use crate::embed::Embed;
use crate::emoji::PartialEmoji;
use crate::guild::PartialMember;
use crate::id::{
    ApplicationId, AttachmentId, ChannelId, GuildId, InteractionId, MessageId, RoleId,
    RoleSubscriptionListingId, UserId, WebhookId,
};
use crate::interaction::{InteractionType, Resolved};
use crate::poll::Poll;
use crate::sticker::{Sticker, StickerItem};
use crate::user::User;

/// The maximum number of characters in the content of a message.
pub const CONTENT_LIMIT: usize = 2000;
/// The maximum number of embeds in a message.
pub const EMBED_COUNT_LIMIT: usize = 10;
/// The maximum number of stickers in a message.
pub const STICKER_COUNT_LIMIT: usize = 3;

/// A message in a channel.
///
/// Without the `MESSAGE_CONTENT` intent, `content`, `embeds`, `attachments`, and `components`
/// are empty and `poll` is absent, except in some cases (DMs, mentions, own messages).
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Message {
    /// The message ID.
    pub id: MessageId,
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The author. For a webhook message, a pseudo-user with the webhook ID and name.
    pub author: User,
    /// The text content.
    #[serde(default)]
    pub content: String,
    /// When the message was sent.
    pub timestamp: Timestamp,
    /// When the message was last edited.
    #[serde(default)]
    pub edited_timestamp: Option<Timestamp>,
    /// Tells if the message is text-to-speech.
    #[serde(default)]
    pub tts: bool,
    /// Tells if the message mentions `@everyone` or `@here`.
    #[serde(default)]
    pub mention_everyone: bool,
    /// The mentioned users. In gateway events, each can have a partial member.
    #[serde(default)]
    pub mentions: Vec<MentionedUser>,
    /// The mentioned roles.
    #[serde(default)]
    pub mention_roles: Vec<RoleId>,
    /// The mentioned channels. Only crossposted messages have them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_channels: Option<Vec<ChannelMention>>,
    /// The attachments that no embed or component refers to.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// The embeds.
    #[serde(default)]
    pub embeds: Vec<Embed>,
    /// The reactions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactions: Option<Vec<Reaction>>,
    /// The nonce that the sender gave, to check that the message was sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<Nonce>,
    /// Tells if the message is pinned.
    #[serde(default)]
    pub pinned: bool,
    /// The webhook that made the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook_id: Option<WebhookId>,
    /// The message type.
    #[serde(rename = "type", default)]
    pub kind: MessageType,
    /// The rich presence activity of the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<MessageActivity>,
    /// The partial application of a rich presence message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application: Option<Box<Application>>,
    /// The application of an interaction response or an application webhook.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
    /// The message flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<MessageFlags>,
    /// The source of a crosspost, a channel follow, a pin, a reply, or a forward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_reference: Option<MessageReference>,
    /// The forwarded messages. Discord gives at most one now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_snapshots: Option<Vec<MessageSnapshot>>,
    /// The referenced message, for replies, thread starters, and context menu commands.
    ///
    /// Absent means that Discord did not try to get it. `Null` means that it was deleted.
    #[serde(default, skip_serializing_if = "crate::Nullable::is_absent")]
    pub referenced_message: crate::Nullable<Box<Message>>,
    /// Data about the interaction that made the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction_metadata: Option<Box<MessageInteractionMetadata>>,
    /// Deprecated: use `interaction_metadata`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction: Option<Box<MessageInteraction>>,
    /// The thread that was started from this message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<Box<Channel>>,
    /// The components.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<Component>,
    /// The stickers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticker_items: Option<Vec<StickerItem>>,
    /// Deprecated: use `sticker_items`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stickers: Option<Vec<Sticker>>,
    /// The approximate position of the message in a thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<u64>,
    /// The data of a role subscription purchase message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_subscription_data: Option<RoleSubscriptionData>,
    /// The users, members, channels, and roles that the message refers to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<Box<Resolved>>,
    /// The poll.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll: Option<Box<Poll>>,
    /// The call in a private channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call: Option<MessageCall>,
    /// A client theme that the message shares.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_client_theme: Option<SharedClientTheme>,
    /// The guild ID. Only in gateway events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The partial member of the author. Only in gateway events, for guild messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Box<PartialMember>>,
    /// The channel type. Only in gateway events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_type: Option<ChannelType>,
}

impl Message {
    /// Gives the message flags, or no flags when the field is absent.
    #[must_use]
    pub fn flags(&self) -> MessageFlags {
        self.flags.unwrap_or_default()
    }

    /// Tells if the message is a forward.
    #[must_use]
    pub fn is_forward(&self) -> bool {
        self.message_reference
            .as_ref()
            .is_some_and(|reference| reference.kind == MessageReferenceType::Forward)
    }

    /// Gives the URL of the message.
    #[must_use]
    pub fn link(&self) -> String {
        let guild = self
            .guild_id
            .map_or_else(|| "@me".to_owned(), |guild_id| guild_id.to_string());
        format!(
            "https://discord.com/channels/{guild}/{}/{}",
            self.channel_id, self.id
        )
    }
}

/// A mentioned user, with the partial member in gateway events.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MentionedUser {
    /// The user.
    #[serde(flatten)]
    pub user: User,
    /// The partial member. Only in gateway events, for guild messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<PartialMember>,
}

/// A nonce: an integer or a string of up to 25 characters.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum Nonce {
    /// An integer nonce.
    Integer(i64),
    /// A string nonce.
    String(String),
}

crate::discord_enum! {
    /// The type of a message.
    pub enum MessageType: u8 {
        /// A normal message.
        Default = 0,
        /// A user was added to a group DM.
        RecipientAdd = 1,
        /// A user was removed from a group DM.
        RecipientRemove = 2,
        /// A call.
        Call = 3,
        /// The channel name changed.
        ChannelNameChange = 4,
        /// The channel icon changed.
        ChannelIconChange = 5,
        /// A message was pinned.
        ChannelPinnedMessage = 6,
        /// A member joined.
        UserJoin = 7,
        /// A member boosted the guild.
        GuildBoost = 8,
        /// The guild reached boost level 1.
        GuildBoostTier1 = 9,
        /// The guild reached boost level 2.
        GuildBoostTier2 = 10,
        /// The guild reached boost level 3.
        GuildBoostTier3 = 11,
        /// An announcement channel was followed.
        ChannelFollowAdd = 12,
        /// The guild lost discovery.
        GuildDiscoveryDisqualified = 14,
        /// The guild got discovery again.
        GuildDiscoveryRequalified = 15,
        /// First discovery grace period warning.
        GuildDiscoveryGracePeriodInitialWarning = 16,
        /// Final discovery grace period warning.
        GuildDiscoveryGracePeriodFinalWarning = 17,
        /// A thread was made.
        ThreadCreated = 18,
        /// A reply.
        Reply = 19,
        /// A response to a slash command.
        ChatInputCommand = 20,
        /// The first message of a thread made from a message.
        ThreadStarterMessage = 21,
        /// A reminder to invite users.
        GuildInviteReminder = 22,
        /// A response to a context menu command.
        ContextMenuCommand = 23,
        /// An auto moderation action.
        AutoModerationAction = 24,
        /// A role subscription purchase.
        RoleSubscriptionPurchase = 25,
        /// A premium upsell of an interaction.
        InteractionPremiumUpsell = 26,
        /// A stage started.
        StageStart = 27,
        /// A stage ended.
        StageEnd = 28,
        /// A user became a stage speaker.
        StageSpeaker = 29,
        /// The stage topic changed.
        StageTopic = 31,
        /// A guild application premium subscription.
        GuildApplicationPremiumSubscription = 32,
        /// Incident alert mode was turned on.
        GuildIncidentAlertModeEnabled = 36,
        /// Incident alert mode was turned off.
        GuildIncidentAlertModeDisabled = 37,
        /// A raid was reported.
        GuildIncidentReportRaid = 38,
        /// A raid report was a false alarm.
        GuildIncidentReportFalseAlarm = 39,
        /// A purchase notification.
        PurchaseNotification = 44,
        /// The result of a poll.
        PollResult = 46,
    }
}

impl MessageType {
    /// Tells if a bot can delete messages of this type. Type 24 also needs `MANAGE_MESSAGES`.
    #[must_use]
    pub const fn is_deletable(self) -> bool {
        !matches!(
            self,
            Self::RecipientAdd
                | Self::RecipientRemove
                | Self::Call
                | Self::ChannelNameChange
                | Self::ChannelIconChange
                | Self::ThreadStarterMessage
        )
    }
}

crate::discord_flags! {
    /// Flags of a message.
    pub struct MessageFlags: u64 as integer {
        /// The message was published to following channels.
        const CROSSPOSTED = 1 << 0;
        /// The message came from a followed channel.
        const IS_CROSSPOST = 1 << 1;
        /// The message shows no embeds.
        const SUPPRESS_EMBEDS = 1 << 2;
        /// The source of the crosspost was deleted.
        const SOURCE_MESSAGE_DELETED = 1 << 3;
        /// The message came from the urgent message system.
        const URGENT = 1 << 4;
        /// The message has a thread with the same ID.
        const HAS_THREAD = 1 << 5;
        /// Only the user of the interaction can see the message.
        const EPHEMERAL = 1 << 6;
        /// The bot is "thinking" (a deferred interaction response).
        const LOADING = 1 << 7;
        /// The message could not mention some roles and add their members to the thread.
        const FAILED_TO_MENTION_SOME_ROLES_IN_THREAD = 1 << 8;
        /// The message triggers no push or desktop notifications.
        const SUPPRESS_NOTIFICATIONS = 1 << 12;
        /// The message is a voice message.
        const IS_VOICE_MESSAGE = 1 << 13;
        /// The message has a snapshot (a forward).
        const HAS_SNAPSHOT = 1 << 14;
        /// The message uses Components V2. It cannot be removed after it is set.
        const IS_COMPONENTS_V2 = 1 << 15;
    }
}

/// The rich presence activity of a message.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessageActivity {
    /// The activity type.
    #[serde(rename = "type")]
    pub kind: MessageActivityType,
    /// The party ID of a rich presence event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub party_id: Option<String>,
}

crate::discord_enum! {
    /// The type of a message activity.
    pub enum MessageActivityType: u8 {
        /// Join.
        Join = 1,
        /// Spectate.
        Spectate = 2,
        /// Listen.
        Listen = 3,
        /// Join request.
        JoinRequest = 5,
        /// Stream request.
        StreamRequest = 6,
    }
}

/// Data about the interaction that made a message.
///
/// The docs define three variants by `kind`. This type holds the fields of all three.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessageInteractionMetadata {
    /// The interaction ID.
    pub id: InteractionId,
    /// The interaction type.
    #[serde(rename = "type")]
    pub kind: InteractionType,
    /// The user that triggered the interaction.
    pub user: User,
    /// The installation contexts of the interaction.
    #[serde(default)]
    pub authorizing_integration_owners: AuthorizingIntegrationOwners,
    /// The original response. Only on follow-up messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_response_message_id: Option<MessageId>,
    /// Command variant: the target user of a user command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_user: Option<User>,
    /// Command variant: the target message of a message command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_message_id: Option<MessageId>,
    /// Component variant: the message that has the component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interacted_message_id: Option<MessageId>,
    /// Modal submit variant: the interaction that opened the modal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triggering_interaction_metadata: Option<Box<MessageInteractionMetadata>>,
}

/// Deprecated data about the interaction of a message. Use [`MessageInteractionMetadata`].
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessageInteraction {
    /// The interaction ID.
    pub id: InteractionId,
    /// The interaction type.
    #[serde(rename = "type")]
    pub kind: InteractionType,
    /// The command name, with subcommands.
    pub name: String,
    /// The user that triggered the interaction.
    pub user: User,
    /// The partial member of the user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<PartialMember>,
}

/// The call in a private channel.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessageCall {
    /// The users that joined the call.
    pub participants: Vec<UserId>,
    /// When the call ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_timestamp: Option<Timestamp>,
}

/// A reference to another message.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessageReference {
    /// The reference type. Absent means `Default`.
    #[serde(
        rename = "type",
        default,
        skip_serializing_if = "crate::serde_util::is_default"
    )]
    pub kind: MessageReferenceType,
    /// The referenced message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<MessageId>,
    /// The channel of the referenced message. Required for forwards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<ChannelId>,
    /// The guild of the referenced message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// When sending: return an error if the message does not exist. The default is `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fail_if_not_exists: Option<bool>,
}

impl MessageReference {
    /// Makes a reply reference.
    #[must_use]
    pub fn reply(message_id: MessageId) -> Self {
        Self {
            message_id: Some(message_id),
            ..Self::default()
        }
    }

    /// Makes a forward reference.
    #[must_use]
    pub fn forward(channel_id: ChannelId, message_id: MessageId) -> Self {
        Self {
            kind: MessageReferenceType::Forward,
            message_id: Some(message_id),
            channel_id: Some(channel_id),
            ..Self::default()
        }
    }

    /// Sets whether an error comes when the message does not exist.
    #[must_use]
    pub fn fail_if_not_exists(mut self, fail: bool) -> Self {
        self.fail_if_not_exists = Some(fail);
        self
    }
}

crate::discord_enum! {
    /// The type of a message reference.
    pub enum MessageReferenceType: u8 {
        /// A normal reference, as replies use. The message is in `referenced_message`.
        Default = 0,
        /// A forward. The message is in `message_snapshots`.
        Forward = 1,
    }
}

/// A forwarded message.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessageSnapshot {
    /// A partial copy of the message.
    pub message: SnapshotMessage,
}

/// The partial message of a snapshot. It has no author.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct SnapshotMessage {
    /// The message type.
    #[serde(rename = "type", default)]
    pub kind: MessageType,
    /// The text content.
    #[serde(default)]
    pub content: String,
    /// The embeds.
    #[serde(default)]
    pub embeds: Vec<Embed>,
    /// The attachments.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// When the message was sent.
    pub timestamp: Timestamp,
    /// When the message was last edited.
    #[serde(default)]
    pub edited_timestamp: Option<Timestamp>,
    /// The message flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<MessageFlags>,
    /// The mentioned users.
    #[serde(default)]
    pub mentions: Vec<User>,
    /// The mentioned roles.
    #[serde(default)]
    pub mention_roles: Vec<RoleId>,
    /// Deprecated: use `sticker_items`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stickers: Option<Vec<Sticker>>,
    /// The stickers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticker_items: Option<Vec<StickerItem>>,
    /// The components.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<Component>,
}

/// A reaction to a message.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Reaction {
    /// The number of reactions, with super reactions.
    pub count: u64,
    /// The numbers of normal and super reactions.
    #[serde(default)]
    pub count_details: ReactionCountDetails,
    /// Tells if the current user reacted.
    #[serde(default)]
    pub me: bool,
    /// Tells if the current user super-reacted.
    #[serde(default)]
    pub me_burst: bool,
    /// The emoji.
    pub emoji: PartialEmoji,
    /// The hex colors of the super reaction.
    #[serde(default)]
    pub burst_colors: Vec<String>,
}

/// The numbers of normal and super reactions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ReactionCountDetails {
    /// The number of super reactions.
    pub burst: u64,
    /// The number of normal reactions.
    pub normal: u64,
}

crate::discord_enum! {
    /// The type of a reaction.
    pub enum ReactionKind: u8 {
        /// A normal reaction.
        Normal = 0,
        /// A super reaction.
        Burst = 1,
    }
}

/// A file attached to a message.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Attachment {
    /// The attachment ID.
    pub id: AttachmentId,
    /// The file name.
    pub filename: String,
    /// The title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The alt text, up to 1024 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The media type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// The size in bytes.
    #[serde(default)]
    pub size: u64,
    /// The source URL.
    pub url: String,
    /// The proxied URL.
    #[serde(default)]
    pub proxy_url: String,
    /// The height, for images and videos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// The width, for images and videos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// The thumbhash placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// The version of the placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder_version: Option<u32>,
    /// Tells if the attachment is ephemeral. Discord removes it after some time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ephemeral: Option<bool>,
    /// The duration of an audio or video file, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f64>,
    /// The base64 waveform of a voice message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waveform: Option<String>,
    /// The attachment flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<AttachmentFlags>,
    /// For a clip: the users in the stream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_participants: Option<Vec<User>>,
    /// For a clip: when it was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_created_at: Option<Timestamp>,
    /// For a clip: the application in the stream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application: Option<Box<Application>>,
}

impl Attachment {
    /// Gives the attachment flags, or no flags when the field is absent.
    #[must_use]
    pub fn flags(&self) -> AttachmentFlags {
        self.flags.unwrap_or_default()
    }
}

crate::discord_flags! {
    /// Flags of an attachment.
    pub struct AttachmentFlags: u64 as integer {
        /// The attachment is a clip from a stream.
        const IS_CLIP = 1 << 0;
        /// The attachment is the thumbnail of a thread in a media channel.
        const IS_THUMBNAIL = 1 << 1;
        /// The attachment was edited with remix. Deprecated.
        const IS_REMIX = 1 << 2;
        /// The attachment is a spoiler.
        const IS_SPOILER = 1 << 3;
        /// Content scanning flagged the attachment as explicit.
        const CONTAINS_EXPLICIT_CONTENT = 1 << 4;
        /// The attachment is an animated image.
        const IS_ANIMATED = 1 << 5;
        /// Content scanning flagged the attachment as gore.
        const CONTAINS_GORE_CONTENT = 1 << 6;
        /// Content scanning flagged the attachment as self-harm.
        const CONTAINS_SELF_HARM_CONTENT = 1 << 7;
    }
}

/// The ID of an attachment in a request: an existing attachment, or the index `n` of `files[n]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum AttachmentRequestId {
    /// An existing attachment, to keep it.
    Existing(AttachmentId),
    /// A new file: the index `n` of the multipart part `files[n]`.
    New(u32),
}

/// An attachment in a create or edit request.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AttachmentRequest {
    /// The existing attachment ID, or the index of the new file.
    pub id: AttachmentRequestId,
    /// The file name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// The title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The alt text, up to 1024 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The duration in seconds. Required for voice messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f64>,
    /// The base64 waveform. Required for voice messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waveform: Option<String>,
    /// Tells if the attachment is a spoiler.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_spoiler: Option<bool>,
}

impl AttachmentRequest {
    /// Makes a request entry that keeps an existing attachment.
    #[must_use]
    pub fn keep(id: AttachmentId) -> Self {
        Self::with_id(AttachmentRequestId::Existing(id))
    }

    /// Makes a request entry for the new file `files[index]`.
    #[must_use]
    pub fn new_file(index: u32, filename: impl Into<String>) -> Self {
        let mut request = Self::with_id(AttachmentRequestId::New(index));
        request.filename = Some(filename.into());
        request
    }

    fn with_id(id: AttachmentRequestId) -> Self {
        Self {
            id,
            filename: None,
            title: None,
            description: None,
            duration_secs: None,
            waveform: None,
            is_spoiler: None,
        }
    }

    /// Sets the alt text.
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets whether the attachment is a spoiler.
    #[must_use]
    pub fn spoiler(mut self, spoiler: bool) -> Self {
        self.is_spoiler = Some(spoiler);
        self
    }
}

/// A channel that a crossposted message mentions.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ChannelMention {
    /// The channel ID.
    pub id: ChannelId,
    /// The guild ID.
    pub guild_id: GuildId,
    /// The channel type.
    #[serde(rename = "type")]
    pub kind: ChannelType,
    /// The channel name.
    pub name: String,
}

/// Which mentions in a message notify users.
///
/// Without this field, regular messages parse all mentions. Interactions and webhooks
/// parse only user mentions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AllowedMentions {
    /// The mention types to parse from the content. Do not combine a type with an
    /// explicit list of the same kind.
    #[serde(default)]
    pub parse: Vec<MentionType>,
    /// The roles that can be mentioned, up to 100.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<RoleId>,
    /// The users that can be mentioned, up to 100.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub users: Vec<UserId>,
    /// For replies: tells if the author of the replied message is mentioned.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub replied_user: bool,
}

impl AllowedMentions {
    /// Allows no mentions.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Allows all mention types.
    #[must_use]
    pub fn all() -> Self {
        Self {
            parse: vec![
                MentionType::Roles,
                MentionType::Users,
                MentionType::Everyone,
            ],
            roles: Vec::new(),
            users: Vec::new(),
            replied_user: true,
        }
    }

    /// Allows only user mentions.
    #[must_use]
    pub fn users() -> Self {
        Self {
            parse: vec![MentionType::Users],
            ..Self::default()
        }
    }
}

crate::discord_string_enum! {
    /// A mention type for [`AllowedMentions::parse`].
    pub enum MentionType {
        /// Role mentions.
        Roles = "roles",
        /// User mentions.
        Users = "users",
        /// `@everyone` and `@here` mentions.
        Everyone = "everyone",
    }
}

/// The data of a role subscription purchase message.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct RoleSubscriptionData {
    /// The SKU and listing.
    pub role_subscription_listing_id: RoleSubscriptionListingId,
    /// The name of the tier.
    pub tier_name: String,
    /// The total number of months of the subscription.
    pub total_months_subscribed: u32,
    /// Tells if the message is for a renewal.
    pub is_renewal: bool,
}

/// A pinned message.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessagePin {
    /// When the message was pinned.
    pub pinned_at: Timestamp,
    /// The message.
    pub message: Message,
}

/// A page of pinned messages.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessagePins {
    /// The pins.
    pub items: Vec<MessagePin>,
    /// Tells if more pins are available.
    #[serde(default)]
    pub has_more: bool,
}

/// A client theme that a message shares.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct SharedClientTheme {
    /// The hex colors of the theme, up to 5.
    #[serde(default)]
    pub colors: Vec<String>,
    /// The direction of the colors, up to 360.
    #[serde(default)]
    pub gradient_angle: u16,
    /// The intensity of the colors, up to 100.
    #[serde(default)]
    pub base_mix: u8,
    /// The mode of the theme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_theme: Option<BaseTheme>,
}

crate::discord_enum! {
    /// The mode of a shared client theme.
    pub enum BaseTheme: u8 {
        /// Not set. Equal to `Dark`.
        Unset = 0,
        /// Dark.
        Dark = 1,
        /// Light.
        Light = 2,
        /// Darker.
        Darker = 3,
        /// Midnight.
        Midnight = 4,
    }
}
