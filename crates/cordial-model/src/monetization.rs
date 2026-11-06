//! SKUs, entitlements, and subscriptions.

use serde::{Deserialize, Serialize};

use crate::Timestamp;
use crate::id::{ApplicationId, EntitlementId, GenericId, GuildId, SkuId, SubscriptionId, UserId};

/// A premium offering of an application (a stock keeping unit).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Sku {
    /// The SKU ID.
    pub id: SkuId,
    /// The SKU type.
    #[serde(rename = "type")]
    pub kind: SkuType,
    /// The application ID.
    pub application_id: ApplicationId,
    /// The name for customers.
    pub name: String,
    /// The URL slug that Discord makes from the name.
    pub slug: String,
    /// The SKU flags.
    #[serde(default)]
    pub flags: SkuFlags,
}

crate::discord_enum! {
    /// The type of a SKU.
    pub enum SkuType: u8 {
        /// A durable one-time purchase.
        Durable = 2,
        /// A consumable one-time purchase.
        Consumable = 3,
        /// A recurring subscription.
        Subscription = 5,
        /// A group that Discord makes for each subscription SKU.
        SubscriptionGroup = 6,
    }
}

crate::discord_flags! {
    /// Flags of a SKU.
    pub struct SkuFlags: u64 as integer {
        /// The SKU is available for purchase.
        const AVAILABLE = 1 << 2;
        /// A subscription for one guild. All users in the guild get access.
        const GUILD_SUBSCRIPTION = 1 << 7;
        /// A subscription for one user. The user gets access in all guilds.
        const USER_SUBSCRIPTION = 1 << 8;
    }
}

/// The access of a user or a guild to a SKU.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Entitlement {
    /// The entitlement ID.
    pub id: EntitlementId,
    /// The SKU ID.
    pub sku_id: SkuId,
    /// The application ID.
    pub application_id: ApplicationId,
    /// The user that has access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<UserId>,
    /// The entitlement type.
    #[serde(rename = "type")]
    pub kind: EntitlementType,
    /// Tells if the entitlement was deleted.
    #[serde(default)]
    pub deleted: bool,
    /// When the entitlement starts to be valid.
    #[serde(default)]
    pub starts_at: Option<Timestamp>,
    /// When the entitlement stops to be valid. `None` for an active subscription.
    #[serde(default)]
    pub ends_at: Option<Timestamp>,
    /// The guild that has access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// For a consumable item: tells if it was consumed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumed: Option<bool>,
    /// The subscription of the entitlement. The docs show it only in examples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_id: Option<SubscriptionId>,
    /// The promotion of the entitlement. The docs show it only in examples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion_id: Option<GenericId>,
    /// The gift code flags. The docs show it only in examples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gift_code_flags: Option<u64>,
}

impl Entitlement {
    /// Tells if the entitlement gives access at the time `now`.
    #[must_use]
    pub fn is_active_at(&self, now: Timestamp) -> bool {
        let started = self.starts_at.is_none_or(|start| start <= now);
        let not_ended = self.ends_at.is_none_or(|end| end > now);
        !self.deleted && self.consumed != Some(true) && started && not_ended
    }
}

crate::discord_enum! {
    /// The type of an entitlement.
    pub enum EntitlementType: u8 {
        /// The user bought the SKU.
        Purchase = 1,
        /// A Discord Nitro subscription.
        PremiumSubscription = 2,
        /// The developer gave it as a gift.
        DeveloperGift = 3,
        /// A developer bought it in application test mode.
        TestModePurchase = 4,
        /// Given when the SKU was free.
        FreePurchase = 5,
        /// Another user gave it as a gift.
        UserGift = 6,
        /// A Nitro subscriber claimed it for free.
        PremiumPurchase = 7,
        /// Bought as an application subscription.
        ApplicationSubscription = 8,
    }
}

/// The owner kind of a test entitlement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EntitlementOwnerType {
    /// A guild.
    Guild,
    /// A user.
    User,
}

impl EntitlementOwnerType {
    /// Gives the raw value that Discord uses.
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            Self::Guild => 1,
            Self::User => 2,
        }
    }
}

impl Serialize for EntitlementOwnerType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.value())
    }
}

/// A recurring payment of a user for a subscription SKU.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Subscription {
    /// The subscription ID.
    pub id: SubscriptionId,
    /// The user that pays.
    pub user_id: UserId,
    /// The SKUs that the user subscribes to.
    pub sku_ids: Vec<SkuId>,
    /// The entitlements that the subscription gives.
    pub entitlement_ids: Vec<EntitlementId>,
    /// The SKUs that the user will subscribe to at renewal.
    #[serde(default)]
    pub renewal_sku_ids: Option<Vec<SkuId>>,
    /// The start of the current period.
    pub current_period_start: Timestamp,
    /// The end of the current period.
    pub current_period_end: Timestamp,
    /// The status. Use entitlements, not the status, to give access.
    pub status: SubscriptionStatus,
    /// When the subscription was canceled.
    #[serde(default)]
    pub canceled_at: Option<Timestamp>,
    /// The country code. Only with a private OAuth scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

crate::discord_enum! {
    /// The status of a subscription.
    pub enum SubscriptionStatus: u8 {
        /// Active and it will renew.
        Active = 0,
        /// Not active and not charged.
        Inactive = 1,
        /// Active, but it will not renew.
        Ending = 2,
    }
}
