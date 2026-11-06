//! Typed snowflake IDs.
//!
//! A Discord ID is a 64-bit snowflake. The type parameter tells what the ID refers to.
//! Thus a user ID cannot go where a channel ID is necessary.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::str::FromStr;

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize, Serializer};

use crate::time::Timestamp;

/// The Discord epoch: the first millisecond of 2015, in Unix milliseconds.
pub const DISCORD_EPOCH_MILLIS: u64 = 1_420_070_400_000;

/// A Discord snowflake ID that refers to a `T`.
pub struct Id<T> {
    value: NonZeroU64,
    marker: PhantomData<fn(T) -> T>,
}

impl<T> Id<T> {
    /// Makes an ID from a raw value. Gives `None` when the value is 0.
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self::from_non_zero(value)),
            None => None,
        }
    }

    /// Makes an ID from a value that is not zero.
    #[must_use]
    pub const fn from_non_zero(value: NonZeroU64) -> Self {
        Self {
            value,
            marker: PhantomData,
        }
    }

    /// Makes an ID from a raw value.
    ///
    /// # Panics
    ///
    /// Panics when the value is 0. Use it only for constants.
    #[must_use]
    pub const fn new_const(value: u64) -> Self {
        match Self::new(value) {
            Some(id) => id,
            None => panic!("a Discord ID cannot be 0"),
        }
    }

    /// Gives the raw value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.value.get()
    }

    /// Gives the raw value as a `NonZeroU64`.
    #[must_use]
    pub const fn into_non_zero(self) -> NonZeroU64 {
        self.value
    }

    /// Gives the same value as an ID of another kind.
    ///
    /// Discord uses one value for related objects. Example: the `@everyone` role ID is the guild ID.
    #[must_use]
    pub const fn cast<U>(self) -> Id<U> {
        Id::from_non_zero(self.value)
    }

    /// Gives the time when Discord made the ID.
    #[must_use]
    pub fn timestamp(self) -> Timestamp {
        Timestamp::from_unix_millis(self.unix_millis().cast_signed())
    }

    /// Gives the time when Discord made the ID, in Unix milliseconds.
    #[must_use]
    pub const fn unix_millis(self) -> u64 {
        (self.get() >> 22) + DISCORD_EPOCH_MILLIS
    }

    /// Gives the internal worker ID (bits 17 to 21).
    #[must_use]
    pub const fn worker_id(self) -> u8 {
        ((self.get() & 0x003E_0000) >> 17) as u8
    }

    /// Gives the internal process ID (bits 12 to 16).
    #[must_use]
    pub const fn process_id(self) -> u8 {
        ((self.get() & 0x0001_F000) >> 12) as u8
    }

    /// Gives the increment (bits 0 to 11).
    #[must_use]
    pub const fn increment(self) -> u16 {
        (self.get() & 0xFFF) as u16
    }

    /// Makes the lowest ID for a time. Use it as a `before` or `after` cursor for pagination.
    ///
    /// Gives `None` when the time is at or before the Discord epoch.
    #[must_use]
    pub const fn from_unix_millis(unix_millis: u64) -> Option<Self> {
        if unix_millis <= DISCORD_EPOCH_MILLIS {
            return None;
        }
        Self::new((unix_millis - DISCORD_EPOCH_MILLIS) << 22)
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T> Eq for Id<T> {}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Id({})", self.value)
    }
}

impl<T> fmt::Display for Id<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.value, formatter)
    }
}

/// The error when a text is not a valid ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseIdError;

impl fmt::Display for ParseIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the text is not a valid Discord ID (an integer from 1 to 2^64-1)")
    }
}

impl std::error::Error for ParseIdError {}

impl<T> FromStr for Id<T> {
    type Err = ParseIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let value: u64 = text.parse().map_err(|_| ParseIdError)?;
        Self::new(value).ok_or(ParseIdError)
    }
}

impl<T> TryFrom<u64> for Id<T> {
    type Error = ParseIdError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(ParseIdError)
    }
}

impl<T> TryFrom<i64> for Id<T> {
    type Error = ParseIdError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        u64::try_from(value)
            .ok()
            .and_then(Self::new)
            .ok_or(ParseIdError)
    }
}

impl<T> From<NonZeroU64> for Id<T> {
    fn from(value: NonZeroU64) -> Self {
        Self::from_non_zero(value)
    }
}

impl<T> From<Id<T>> for u64 {
    fn from(id: Id<T>) -> Self {
        id.get()
    }
}

impl<T> Serialize for Id<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.value)
    }
}

impl<'de, T> Deserialize<'de> for Id<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(IdVisitor(PhantomData))
    }
}

struct IdVisitor<T>(PhantomData<fn(T) -> T>);

impl<T> Visitor<'_> for IdVisitor<T> {
    type Value = Id<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a Discord ID as a string or an integer")
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Id::new(value).ok_or_else(|| E::custom("a Discord ID cannot be 0"))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Id::try_from(value)
            .map_err(|_| E::custom(format_args!("{value} is not a valid Discord ID")))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        value
            .parse()
            .map_err(|_| E::custom(format_args!("{value:?} is not a valid Discord ID")))
    }
}

/// Marker types for [`Id`].
pub mod marker {
    macro_rules! markers {
        ($( $(#[$meta:meta])* $name:ident, )*) => {
            $(
                $(#[$meta])*
                #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
                #[non_exhaustive]
                pub struct $name;
            )*
        };
    }

    markers! {
        /// An application.
        ApplicationMarker,
        /// A message attachment.
        AttachmentMarker,
        /// An audit log entry.
        AuditLogEntryMarker,
        /// An auto moderation rule.
        AutoModerationRuleMarker,
        /// A channel or a thread.
        ChannelMarker,
        /// An application command.
        CommandMarker,
        /// A version of an application command.
        CommandVersionMarker,
        /// An emoji.
        EmojiMarker,
        /// An entitlement.
        EntitlementMarker,
        /// A forum tag.
        ForumTagMarker,
        /// An ID that can refer to more than one kind of object.
        GenericMarker,
        /// A guild.
        GuildMarker,
        /// A guild integration.
        IntegrationMarker,
        /// An interaction.
        InteractionMarker,
        /// A lobby.
        LobbyMarker,
        /// A message.
        MessageMarker,
        /// An onboarding prompt or prompt option.
        OnboardingPromptMarker,
        /// A role.
        RoleMarker,
        /// A role subscription listing.
        RoleSubscriptionListingMarker,
        /// A guild scheduled event.
        ScheduledEventMarker,
        /// A SKU.
        SkuMarker,
        /// A soundboard sound.
        SoundboardSoundMarker,
        /// A stage instance.
        StageInstanceMarker,
        /// A sticker.
        StickerMarker,
        /// A sticker pack.
        StickerPackMarker,
        /// A subscription.
        SubscriptionMarker,
        /// A team.
        TeamMarker,
        /// A user.
        UserMarker,
        /// A webhook.
        WebhookMarker,
    }
}

use marker::{
    ApplicationMarker, AttachmentMarker, AuditLogEntryMarker, AutoModerationRuleMarker,
    ChannelMarker, CommandMarker, CommandVersionMarker, EmojiMarker, EntitlementMarker,
    ForumTagMarker, GenericMarker, GuildMarker, IntegrationMarker, InteractionMarker, LobbyMarker,
    MessageMarker, OnboardingPromptMarker, RoleMarker, RoleSubscriptionListingMarker,
    ScheduledEventMarker, SkuMarker, SoundboardSoundMarker, StageInstanceMarker, StickerMarker,
    StickerPackMarker, SubscriptionMarker, TeamMarker, UserMarker, WebhookMarker,
};

/// The ID of an application.
pub type ApplicationId = Id<ApplicationMarker>;
/// The ID of a message attachment.
pub type AttachmentId = Id<AttachmentMarker>;
/// The ID of an audit log entry.
pub type AuditLogEntryId = Id<AuditLogEntryMarker>;
/// The ID of an auto moderation rule.
pub type AutoModerationRuleId = Id<AutoModerationRuleMarker>;
/// The ID of a channel or a thread.
pub type ChannelId = Id<ChannelMarker>;
/// The ID of an application command.
pub type CommandId = Id<CommandMarker>;
/// The ID of a version of an application command.
pub type CommandVersionId = Id<CommandVersionMarker>;
/// The ID of an emoji.
pub type EmojiId = Id<EmojiMarker>;
/// The ID of an entitlement.
pub type EntitlementId = Id<EntitlementMarker>;
/// The ID of a forum tag.
pub type ForumTagId = Id<ForumTagMarker>;
/// An ID that can refer to more than one kind of object.
pub type GenericId = Id<GenericMarker>;
/// The ID of a guild.
pub type GuildId = Id<GuildMarker>;
/// The ID of a guild integration.
pub type IntegrationId = Id<IntegrationMarker>;
/// The ID of an interaction.
pub type InteractionId = Id<InteractionMarker>;
/// The ID of a lobby.
pub type LobbyId = Id<LobbyMarker>;
/// The ID of a message.
pub type MessageId = Id<MessageMarker>;
/// The ID of an onboarding prompt or prompt option.
pub type OnboardingPromptId = Id<OnboardingPromptMarker>;
/// The ID of a role.
pub type RoleId = Id<RoleMarker>;
/// The ID of a role subscription listing.
pub type RoleSubscriptionListingId = Id<RoleSubscriptionListingMarker>;
/// The ID of a guild scheduled event.
pub type ScheduledEventId = Id<ScheduledEventMarker>;
/// The ID of a SKU.
pub type SkuId = Id<SkuMarker>;
/// The ID of a soundboard sound.
pub type SoundboardSoundId = Id<SoundboardSoundMarker>;
/// The ID of a stage instance.
pub type StageInstanceId = Id<StageInstanceMarker>;
/// The ID of a sticker.
pub type StickerId = Id<StickerMarker>;
/// The ID of a sticker pack.
pub type StickerPackId = Id<StickerPackMarker>;
/// The ID of a subscription.
pub type SubscriptionId = Id<SubscriptionMarker>;
/// The ID of a team.
pub type TeamId = Id<TeamMarker>;
/// The ID of a user.
pub type UserId = Id<UserMarker>;
/// The ID of a webhook.
pub type WebhookId = Id<WebhookMarker>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_string_and_integer() {
        let from_string: UserId = serde_json::from_str("\"80351110224678912\"").unwrap();
        let from_integer: UserId = serde_json::from_str("80351110224678912").unwrap();

        assert_eq!(from_string, from_integer);
        assert_eq!(from_string.get(), 80_351_110_224_678_912);
    }

    #[test]
    fn encodes_as_string() {
        let id = UserId::new_const(80_351_110_224_678_912);

        assert_eq!(serde_json::to_string(&id).unwrap(), "\"80351110224678912\"");
    }

    #[test]
    fn refuses_zero_and_negative() {
        assert!(serde_json::from_str::<UserId>("\"0\"").is_err());
        assert!(serde_json::from_str::<UserId>("-5").is_err());
        assert!(serde_json::from_str::<UserId>("\"abc\"").is_err());
    }

    #[test]
    fn gives_snowflake_parts() {
        // Example from the Discord reference page.
        let id = UserId::new_const(175_928_847_299_117_063);

        assert_eq!(id.unix_millis(), 1_462_015_105_796);
        assert_eq!(id.worker_id(), 1);
        assert_eq!(id.process_id(), 0);
        assert_eq!(id.increment(), 7);
    }

    #[test]
    fn makes_pagination_cursor() {
        let id = MessageId::from_unix_millis(1_462_015_105_796).unwrap();

        assert_eq!(id.unix_millis(), 1_462_015_105_796);
        assert_eq!(id.increment(), 0);
        assert!(MessageId::from_unix_millis(DISCORD_EPOCH_MILLIS).is_none());
    }

    #[test]
    fn option_has_no_size_cost() {
        assert_eq!(size_of::<Option<UserId>>(), size_of::<UserId>());
    }
}
