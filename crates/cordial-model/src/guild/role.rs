//! Roles.

use serde::{Deserialize, Serialize};

use crate::id::{IntegrationId, RoleId, RoleSubscriptionListingId, UserId};
use crate::{Color, ImageHash, Permissions};

/// A role in a guild.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Role {
    /// The role ID. The `@everyone` role has the ID of the guild.
    pub id: RoleId,
    /// The name.
    pub name: String,
    /// The color. Deprecated: use `colors`. Discord still sends it.
    #[serde(default)]
    pub color: Color,
    /// The colors of the role.
    #[serde(default)]
    pub colors: RoleColors,
    /// Tells if the role shows separately in the member list.
    pub hoist: bool,
    /// The icon hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<ImageHash>,
    /// The Unicode emoji of the role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unicode_emoji: Option<String>,
    /// The position. Roles with the same position sort by ID.
    pub position: i32,
    /// The permissions.
    pub permissions: Permissions,
    /// Tells if an integration manages the role.
    pub managed: bool,
    /// Tells if members can mention the role.
    pub mentionable: bool,
    /// The tags of the role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<RoleTags>,
    /// The role flags.
    #[serde(default)]
    pub flags: RoleFlags,
}

impl Role {
    /// Gives the mention text, for example `<@&41771983423143936>`.
    #[must_use]
    pub fn mention(&self) -> String {
        format!("<@&{}>", self.id)
    }

    /// Compares the position of two roles as Discord does: by position, then by ID.
    ///
    /// A higher role in the hierarchy is "greater".
    #[must_use]
    pub fn cmp_position(&self, other: &Self) -> std::cmp::Ordering {
        // With the same position, the role with the lower ID is higher.
        self.position
            .cmp(&other.position)
            .then_with(|| other.id.cmp(&self.id))
    }
}

/// The colors of a role.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct RoleColors {
    /// The primary color. A value of 0 means "no color".
    pub primary_color: Color,
    /// The secondary color. It makes the role a gradient. Needs `ENHANCED_ROLE_COLORS`.
    #[serde(default)]
    pub secondary_color: Option<Color>,
    /// The tertiary color. It makes the role holographic. Needs `ENHANCED_ROLE_COLORS`.
    #[serde(default)]
    pub tertiary_color: Option<Color>,
}

impl RoleColors {
    /// The fixed colors that Discord sets when a request gives a tertiary color.
    pub const HOLOGRAPHIC: Self = Self {
        primary_color: Color(11_127_295),
        secondary_color: Some(Color(16_759_788)),
        tertiary_color: Some(Color(16_761_760)),
    };

    /// Makes a single color.
    #[must_use]
    pub const fn solid(color: Color) -> Self {
        Self {
            primary_color: color,
            secondary_color: None,
            tertiary_color: None,
        }
    }

    /// Makes a gradient of two colors.
    #[must_use]
    pub const fn gradient(primary: Color, secondary: Color) -> Self {
        Self {
            primary_color: primary,
            secondary_color: Some(secondary),
            tertiary_color: None,
        }
    }
}

/// The tags of a role.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct RoleTags {
    /// The ID of the bot that the role belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<UserId>,
    /// The ID of the integration that the role belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration_id: Option<IntegrationId>,
    /// Tells if the role is the Booster role of the guild.
    #[serde(
        default,
        skip_serializing_if = "crate::serde_util::is_false",
        with = "crate::serde_util::present_is_true"
    )]
    pub premium_subscriber: bool,
    /// The ID of the subscription SKU and listing of the role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_listing_id: Option<RoleSubscriptionListingId>,
    /// Tells if the role is available for purchase.
    #[serde(
        default,
        skip_serializing_if = "crate::serde_util::is_false",
        with = "crate::serde_util::present_is_true"
    )]
    pub available_for_purchase: bool,
    /// Tells if the role is a linked role of the guild.
    #[serde(
        default,
        skip_serializing_if = "crate::serde_util::is_false",
        with = "crate::serde_util::present_is_true"
    )]
    pub guild_connections: bool,
}

crate::discord_flags! {
    /// Flags of a role.
    pub struct RoleFlags: u64 as integer {
        /// Members can select the role in an onboarding prompt.
        const IN_PROMPT = 1 << 0;
    }
}

/// A partial role, as in invites.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct PartialRole {
    /// The role ID.
    pub id: RoleId,
    /// The name.
    pub name: String,
    /// The position.
    pub position: i32,
    /// The color. Deprecated: use `colors`.
    #[serde(default)]
    pub color: Color,
    /// The colors of the role.
    #[serde(default)]
    pub colors: RoleColors,
    /// The icon hash.
    #[serde(default)]
    pub icon: Option<ImageHash>,
    /// The Unicode emoji of the role.
    #[serde(default)]
    pub unicode_emoji: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_tags_mean_true() {
        let tags: RoleTags = serde_json::from_str(r#"{"premium_subscriber":null}"#).unwrap();

        assert!(tags.premium_subscriber);
        assert!(!tags.available_for_purchase);
        assert_eq!(
            serde_json::to_string(&tags).unwrap(),
            r#"{"premium_subscriber":null}"#
        );
    }
}
