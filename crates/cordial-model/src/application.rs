//! Applications, teams, install settings, and role connection metadata.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::guild::Guild;
use crate::id::{ApplicationId, ChannelId, GuildId, SkuId, TeamId, UserId};
use crate::locale::Localizations;
use crate::oauth2::Scope;
use crate::user::User;
use crate::{ImageHash, Permissions};

/// An application (an "app"). Its bot user acts for it.
///
/// Partial forms of this object (for example in invites) leave many fields empty.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Application {
    /// The application ID.
    pub id: ApplicationId,
    /// The name.
    #[serde(default)]
    pub name: String,
    /// The icon hash.
    #[serde(default)]
    pub icon: Option<ImageHash>,
    /// The description.
    #[serde(default)]
    pub description: String,
    /// The RPC origin URLs, if RPC is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpc_origins: Option<Vec<String>>,
    /// Tells if users other than the owner can add the bot to guilds.
    #[serde(default)]
    pub bot_public: bool,
    /// Tells if the bot joins only after the full OAuth2 code grant.
    #[serde(default)]
    pub bot_require_code_grant: bool,
    /// The bot user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot: Option<User>,
    /// The URL of the terms of service.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terms_of_service_url: Option<String>,
    /// The URL of the privacy policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_policy_url: Option<String>,
    /// The owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<User>,
    /// The hex Ed25519 public key that verifies interactions and webhook events.
    #[serde(default)]
    pub verify_key: String,
    /// The team, if a team owns the application.
    #[serde(default)]
    pub team: Option<Team>,
    /// The guild of the application, for example a support server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The partial guild of the application.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild: Option<Box<Guild>>,
    /// The game SKU, for a game sold on Discord.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_sku_id: Option<SkuId>,
    /// The URL slug of the store page, for a game sold on Discord.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// The cover image hash for rich presence invites.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_image: Option<ImageHash>,
    /// The legacy flags field. It never grows past 31 bits. Use [`Application::flags`].
    #[serde(default, rename = "flags", skip_serializing_if = "Option::is_none")]
    pub legacy_flags: Option<ApplicationFlags>,
    /// The flags as a string, with all bits. Use [`Application::flags`].
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_util::option_string_integer"
    )]
    pub flags_new: Option<u64>,
    /// The approximate number of guilds that added the application.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approximate_guild_count: Option<u64>,
    /// The approximate number of users that installed the application.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approximate_user_install_count: Option<u64>,
    /// The approximate number of users with an OAuth2 authorization for the application.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approximate_user_authorization_count: Option<u64>,
    /// The OAuth2 redirect URIs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect_uris: Option<Vec<String>>,
    /// The URL that receives interactions over HTTP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interactions_endpoint_url: Option<String>,
    /// The URL for role connection verification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_connections_verification_url: Option<String>,
    /// The URL that receives webhook events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_webhooks_url: Option<String>,
    /// Tells if webhook events are on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_webhooks_status: Option<EventWebhookStatus>,
    /// The webhook event types that the application subscribes to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_webhooks_types: Option<Vec<String>>,
    /// Tags that describe the application, up to 5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// The settings of the default install link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_params: Option<InstallParams>,
    /// The default install settings for each installation context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration_types_config:
        Option<HashMap<ApplicationIntegrationType, IntegrationTypeConfig>>,
    /// The custom install URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_install_url: Option<String>,
}

impl Application {
    /// Gives the application flags, with the bits above 30 when Discord sends them.
    #[must_use]
    pub fn flags(&self) -> ApplicationFlags {
        match (self.flags_new, self.legacy_flags) {
            (Some(bits), _) => ApplicationFlags::from_bits_retain(bits),
            (None, Some(flags)) => flags,
            (None, None) => ApplicationFlags::empty(),
        }
    }
}

/// The partial application in the `READY` event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct PartialApplication {
    /// The application ID.
    pub id: ApplicationId,
    /// The legacy flags field.
    #[serde(default, rename = "flags")]
    pub legacy_flags: ApplicationFlags,
    /// The flags as a string, with all bits.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_util::option_string_integer"
    )]
    pub flags_new: Option<u64>,
}

impl PartialApplication {
    /// Gives the application flags, with the bits above 30 when Discord sends them.
    #[must_use]
    pub fn flags(&self) -> ApplicationFlags {
        self.flags_new
            .map_or(self.legacy_flags, ApplicationFlags::from_bits_retain)
    }
}

crate::discord_flags! {
    /// Public flags of an application.
    pub struct ApplicationFlags: u64 as integer {
        /// The application uses the auto moderation API.
        const AUTO_MODERATION_RULE_CREATE_BADGE = 1 << 6;
        /// Presence intent, for bots in 100 or more guilds.
        const GATEWAY_PRESENCE = 1 << 12;
        /// Presence intent, for bots in fewer than 100 guilds.
        const GATEWAY_PRESENCE_LIMITED = 1 << 13;
        /// Guild members intent, for bots in 100 or more guilds.
        const GATEWAY_GUILD_MEMBERS = 1 << 14;
        /// Guild members intent, for bots in fewer than 100 guilds.
        const GATEWAY_GUILD_MEMBERS_LIMITED = 1 << 15;
        /// Unusual growth stops the verification of the application.
        const VERIFICATION_PENDING_GUILD_LIMIT = 1 << 16;
        /// The application is embedded in the Discord client.
        const EMBEDDED = 1 << 17;
        /// Message content intent, for bots in 100 or more guilds.
        const GATEWAY_MESSAGE_CONTENT = 1 << 18;
        /// Message content intent, for bots in fewer than 100 guilds.
        const GATEWAY_MESSAGE_CONTENT_LIMITED = 1 << 19;
        /// The application has global commands.
        const APPLICATION_COMMAND_BADGE = 1 << 23;
    }
}

crate::discord_enum! {
    /// Where an application can be installed (an installation context).
    pub enum ApplicationIntegrationType: u8 {
        /// The application can be installed to guilds.
        GuildInstall = 0,
        /// The application can be installed to users.
        UserInstall = 1,
    }
}

impl std::fmt::Display for ApplicationIntegrationType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.value())
    }
}

impl std::str::FromStr for ApplicationIntegrationType {
    type Err = std::num::ParseIntError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        text.parse::<u8>().map(Self::from_value)
    }
}

/// The installation contexts that authorized an interaction.
///
/// For a guild installation, the value is the guild ID, or `"0"` in the DM with the bot.
/// For a user installation, the value is the user ID.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct AuthorizingIntegrationOwners(pub HashMap<ApplicationIntegrationType, String>);

impl AuthorizingIntegrationOwners {
    /// Gives the guild of a guild installation. `None` also in the DM with the bot.
    #[must_use]
    pub fn guild_id(&self) -> Option<GuildId> {
        self.0
            .get(&ApplicationIntegrationType::GuildInstall)
            .and_then(|value| value.parse().ok())
    }

    /// Tells if a guild installation authorized the interaction.
    #[must_use]
    pub fn is_guild_install(&self) -> bool {
        self.0
            .contains_key(&ApplicationIntegrationType::GuildInstall)
    }

    /// Gives the user of a user installation.
    #[must_use]
    pub fn user_id(&self) -> Option<UserId> {
        self.0
            .get(&ApplicationIntegrationType::UserInstall)
            .and_then(|value| value.parse().ok())
    }

    /// Tells if only a user installation authorized the interaction.
    #[must_use]
    pub fn is_user_install_only(&self) -> bool {
        self.0.len() == 1
            && self
                .0
                .contains_key(&ApplicationIntegrationType::UserInstall)
    }
}

crate::discord_enum! {
    /// Tells if webhook events are on for an application.
    pub enum EventWebhookStatus: u8 {
        /// The developer turned webhook events off.
        Disabled = 1,
        /// The developer turned webhook events on.
        Enabled = 2,
        /// Discord turned webhook events off, usually because of inactivity.
        DisabledByDiscord = 3,
    }
}

/// The scopes and permissions of an install link.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct InstallParams {
    /// The OAuth2 scopes.
    #[serde(default)]
    pub scopes: Vec<Scope>,
    /// The permissions for the bot role.
    #[serde(default)]
    pub permissions: Permissions,
}

/// The default install settings for one installation context.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct IntegrationTypeConfig {
    /// The install settings of the default install link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth2_install_params: Option<InstallParams>,
}

/// A team of developers that owns applications.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Team {
    /// The icon hash.
    #[serde(default)]
    pub icon: Option<ImageHash>,
    /// The team ID.
    pub id: TeamId,
    /// The members.
    #[serde(default)]
    pub members: Vec<TeamMember>,
    /// The name.
    pub name: String,
    /// The user ID of the owner.
    pub owner_user_id: UserId,
}

/// A member of a team.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct TeamMember {
    /// The membership state.
    pub membership_state: MembershipState,
    /// The team ID.
    pub team_id: TeamId,
    /// The user. Only some fields are set.
    pub user: User,
    /// The role in the team. Old payloads do not have it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<TeamMemberRole>,
}

crate::discord_enum! {
    /// The membership state of a team member.
    pub enum MembershipState: u8 {
        /// The user is invited.
        Invited = 1,
        /// The user accepted the invite.
        Accepted = 2,
    }
}

crate::discord_string_enum! {
    /// The role of a team member. The owner has the role `Admin` and is the team owner.
    pub enum TeamMemberRole {
        /// An admin. The owner also has this role.
        Admin = "admin",
        /// A developer.
        Developer = "developer",
        /// A member with read-only access.
        ReadOnly = "read_only",
    }
}

/// A metadata field of an application for role connections.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct RoleConnectionMetadata {
    /// The comparison type.
    #[serde(rename = "type")]
    pub kind: RoleConnectionMetadataType,
    /// The key: 1 to 50 characters of `a-z`, `0-9`, and `_`.
    pub key: String,
    /// The name, 1 to 100 characters.
    pub name: String,
    /// The localized names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_localizations: Option<Localizations>,
    /// The description, 1 to 200 characters.
    pub description: String,
    /// The localized descriptions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_localizations: Option<Localizations>,
}

crate::discord_enum! {
    /// How a role connection metadata value compares with the value that a guild sets.
    pub enum RoleConnectionMetadataType: u8 {
        /// The integer value is less than or equal to the guild value.
        IntegerLessThanOrEqual = 1,
        /// The integer value is greater than or equal to the guild value.
        IntegerGreaterThanOrEqual = 2,
        /// The integer value is equal to the guild value.
        IntegerEqual = 3,
        /// The integer value is not equal to the guild value.
        IntegerNotEqual = 4,
        /// The date is at or before the guild value (days before now).
        DatetimeLessThanOrEqual = 5,
        /// The date is at or after the guild value (days before now).
        DatetimeGreaterThanOrEqual = 6,
        /// The boolean value is equal to the guild value.
        BooleanEqual = 7,
        /// The boolean value is not equal to the guild value.
        BooleanNotEqual = 8,
    }
}

/// An instance of an activity (an embedded application).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ActivityInstance {
    /// The application ID.
    pub application_id: ApplicationId,
    /// The instance ID.
    pub instance_id: String,
    /// The ID of the launch.
    pub launch_id: String,
    /// Where the activity runs.
    pub location: ActivityLocation,
    /// The IDs of the users in the activity.
    pub users: Vec<UserId>,
}

/// Where an activity instance runs.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ActivityLocation {
    /// The location ID.
    pub id: String,
    /// The kind of location.
    pub kind: ActivityLocationKind,
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The guild ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
}

crate::discord_string_enum! {
    /// The kind of location of an activity instance.
    pub enum ActivityLocationKind {
        /// A guild channel.
        GuildChannel = "gc",
        /// A private channel (a DM or a group DM).
        PrivateChannel = "pc",
    }
}
