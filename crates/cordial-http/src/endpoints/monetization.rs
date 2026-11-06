//! SKU, entitlement, and subscription endpoints.

use std::fmt::Write as _;

use cordial_model::id::{
    ApplicationId, EntitlementId, GenericId, GuildId, SkuId, SubscriptionId, UserId,
};
use cordial_model::monetization::{Entitlement, EntitlementOwnerType, Sku, Subscription};
use serde::Serialize;

use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, RequestBody};
use crate::route::{MajorParameter, Method, Query, Route};

/// Joins IDs with commas, for a query parameter such as `sku_ids`.
fn join_ids(ids: &[SkuId]) -> String {
    let mut text = String::new();
    for (index, id) in ids.iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        let _ = write!(text, "{id}");
    }
    text
}

/// `GET /applications/{application.id}/entitlements`: the entitlements of an application,
/// active and ended.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetEntitlements {
    client: Client,
    application_id: ApplicationId,
    user_id: Option<UserId>,
    sku_ids: Vec<SkuId>,
    before: Option<EntitlementId>,
    after: Option<EntitlementId>,
    limit: Option<u8>,
    guild_id: Option<GuildId>,
    exclude_ended: Option<bool>,
    exclude_deleted: Option<bool>,
}

impl GetEntitlements {
    /// Gets only the entitlements of a user.
    pub fn user(mut self, user_id: UserId) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Gets only the entitlements for these SKUs.
    pub fn sku_ids(mut self, sku_ids: impl IntoIterator<Item = SkuId>) -> Self {
        self.sku_ids = sku_ids.into_iter().collect();
        self
    }

    /// Gets entitlements before an entitlement ID.
    pub fn before(mut self, entitlement_id: EntitlementId) -> Self {
        self.before = Some(entitlement_id);
        self
    }

    /// Gets entitlements after an entitlement ID.
    pub fn after(mut self, entitlement_id: EntitlementId) -> Self {
        self.after = Some(entitlement_id);
        self
    }

    /// Sets the number of entitlements, 1 to 100. The default is 100.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(1, 100));
        self
    }

    /// Gets only the entitlements of a guild.
    pub fn guild(mut self, guild_id: GuildId) -> Self {
        self.guild_id = Some(guild_id);
        self
    }

    /// Removes ended entitlements from the result. The default is `false`.
    pub fn exclude_ended(mut self, exclude: bool) -> Self {
        self.exclude_ended = Some(exclude);
        self
    }

    /// Removes deleted entitlements from the result. The default is `true`.
    pub fn exclude_deleted(mut self, exclude: bool) -> Self {
        self.exclude_deleted = Some(exclude);
        self
    }
}

impl Endpoint for GetEntitlements {
    type Response = Vec<Entitlement>;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("user_id", self.user_id);
        if !self.sku_ids.is_empty() {
            query.push("sku_ids", join_ids(&self.sku_ids));
        }
        query.push_opt("before", self.before);
        query.push_opt("after", self.after);
        query.push_opt("limit", self.limit);
        query.push_opt("guild_id", self.guild_id);
        query.push_opt("exclude_ended", self.exclude_ended);
        query.push_opt("exclude_deleted", self.exclude_deleted);
        Route::new(
            Method::Get,
            "/applications/{application_id}/entitlements",
            format!(
                "/applications/{}/entitlements{}",
                self.application_id,
                query.finish()
            ),
            MajorParameter::None,
        )
    }
}

/// `GET /applications/{application.id}/entitlements/{entitlement.id}`: one entitlement.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetEntitlement {
    client: Client,
    application_id: ApplicationId,
    entitlement_id: EntitlementId,
}

impl Endpoint for GetEntitlement {
    type Response = Entitlement;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/applications/{application_id}/entitlements/{entitlement_id}",
            format!(
                "/applications/{}/entitlements/{}",
                self.application_id, self.entitlement_id
            ),
            MajorParameter::None,
        )
    }
}

/// `POST /applications/{application.id}/entitlements/{entitlement.id}/consume`: marks an
/// entitlement of a consumable SKU as consumed.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct ConsumeEntitlement {
    client: Client,
    application_id: ApplicationId,
    entitlement_id: EntitlementId,
}

impl Endpoint for ConsumeEntitlement {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/applications/{application_id}/entitlements/{entitlement_id}/consume",
            format!(
                "/applications/{}/entitlements/{}/consume",
                self.application_id, self.entitlement_id
            ),
            MajorParameter::None,
        )
    }

    fn retry_safe(&self) -> bool {
        // A second consume has no other effect.
        true
    }
}

/// The owner of a test entitlement: a guild or a user.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TestEntitlementOwner {
    /// A guild. Use it for a guild subscription.
    Guild(GuildId),
    /// A user. Use it for a user subscription.
    User(UserId),
}

impl TestEntitlementOwner {
    fn id(self) -> GenericId {
        match self {
            Self::Guild(guild_id) => guild_id.cast(),
            Self::User(user_id) => user_id.cast(),
        }
    }

    fn kind(self) -> EntitlementOwnerType {
        match self {
            Self::Guild(_) => EntitlementOwnerType::Guild,
            Self::User(_) => EntitlementOwnerType::User,
        }
    }
}

/// `POST /applications/{application.id}/entitlements`: gives a test entitlement for a SKU
/// to a guild or a user.
///
/// The response is a partial entitlement without `starts_at`, `ends_at`, and
/// `subscription_id`. Reload the Discord client to see the change.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateTestEntitlement {
    client: Client,
    application_id: ApplicationId,
    sku_id: SkuId,
    owner: TestEntitlementOwner,
}

#[derive(Serialize)]
struct CreateTestEntitlementBody {
    sku_id: SkuId,
    owner_id: GenericId,
    owner_type: EntitlementOwnerType,
}

impl Endpoint for CreateTestEntitlement {
    type Response = Entitlement;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/applications/{application_id}/entitlements",
            format!("/applications/{}/entitlements", self.application_id),
            MajorParameter::None,
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&CreateTestEntitlementBody {
            sku_id: self.sku_id,
            owner_id: self.owner.id(),
            owner_type: self.owner.kind(),
        })
    }
}

/// `DELETE /applications/{application.id}/entitlements/{entitlement.id}`: deletes a test
/// entitlement.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteTestEntitlement {
    client: Client,
    application_id: ApplicationId,
    entitlement_id: EntitlementId,
}

impl Endpoint for DeleteTestEntitlement {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/applications/{application_id}/entitlements/{entitlement_id}",
            format!(
                "/applications/{}/entitlements/{}",
                self.application_id, self.entitlement_id
            ),
            MajorParameter::None,
        )
    }
}

/// `GET /users/@me/applications/{application.id}/entitlements`: the entitlements of the
/// current user for an application.
///
/// This endpoint needs a Bearer token. The OpenAPI specification is the only source of
/// this endpoint.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetCurrentUserEntitlements {
    client: Client,
    application_id: ApplicationId,
    sku_ids: Vec<SkuId>,
    exclude_consumed: Option<bool>,
}

impl GetCurrentUserEntitlements {
    /// Gets only the entitlements for these SKUs (up to 100).
    pub fn sku_ids(mut self, sku_ids: impl IntoIterator<Item = SkuId>) -> Self {
        self.sku_ids = sku_ids.into_iter().collect();
        self
    }

    /// Removes consumed entitlements from the result.
    pub fn exclude_consumed(mut self, exclude: bool) -> Self {
        self.exclude_consumed = Some(exclude);
        self
    }
}

impl Endpoint for GetCurrentUserEntitlements {
    type Response = Vec<Entitlement>;

    fn route(&self) -> Route {
        let mut query = Query::new();
        if !self.sku_ids.is_empty() {
            query.push("sku_ids", join_ids(&self.sku_ids));
        }
        query.push_opt("exclude_consumed", self.exclude_consumed);
        Route::new(
            Method::Get,
            "/users/@me/applications/{application_id}/entitlements",
            format!(
                "/users/@me/applications/{}/entitlements{}",
                self.application_id,
                query.finish()
            ),
            MajorParameter::None,
        )
    }
}

/// `GET /applications/{application.id}/skus`: the SKUs of an application.
///
/// A subscription has two SKUs. Use the SKU of type `Subscription` (5) for tests and
/// integrations.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetSkus {
    client: Client,
    application_id: ApplicationId,
}

impl Endpoint for GetSkus {
    type Response = Vec<Sku>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/applications/{application_id}/skus",
            format!("/applications/{}/skus", self.application_id),
            MajorParameter::None,
        )
    }
}

/// `GET /skus/{sku.id}/subscriptions`: the subscriptions that contain a SKU.
///
/// The user filter is necessary, except with an OAuth2 token.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetSkuSubscriptions {
    client: Client,
    sku_id: SkuId,
    before: Option<SubscriptionId>,
    after: Option<SubscriptionId>,
    limit: Option<u8>,
    user_id: Option<UserId>,
}

impl GetSkuSubscriptions {
    /// Gets subscriptions before a subscription ID.
    pub fn before(mut self, subscription_id: SubscriptionId) -> Self {
        self.before = Some(subscription_id);
        self
    }

    /// Gets subscriptions after a subscription ID.
    pub fn after(mut self, subscription_id: SubscriptionId) -> Self {
        self.after = Some(subscription_id);
        self
    }

    /// Sets the number of subscriptions, 1 to 100. The default is 50.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(1, 100));
        self
    }

    /// Gets only the subscriptions of a user.
    pub fn user(mut self, user_id: UserId) -> Self {
        self.user_id = Some(user_id);
        self
    }
}

impl Endpoint for GetSkuSubscriptions {
    type Response = Vec<Subscription>;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("before", self.before);
        query.push_opt("after", self.after);
        query.push_opt("limit", self.limit);
        query.push_opt("user_id", self.user_id);
        Route::new(
            Method::Get,
            "/skus/{sku_id}/subscriptions",
            format!("/skus/{}/subscriptions{}", self.sku_id, query.finish()),
            MajorParameter::None,
        )
    }
}

/// `GET /skus/{sku.id}/subscriptions/{subscription.id}`: one subscription.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetSkuSubscription {
    client: Client,
    sku_id: SkuId,
    subscription_id: SubscriptionId,
}

impl Endpoint for GetSkuSubscription {
    type Response = Subscription;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/skus/{sku_id}/subscriptions/{subscription_id}",
            format!(
                "/skus/{}/subscriptions/{}",
                self.sku_id, self.subscription_id
            ),
            MajorParameter::None,
        )
    }
}

super::into_future!(
    GetEntitlements,
    GetEntitlement,
    ConsumeEntitlement,
    CreateTestEntitlement,
    DeleteTestEntitlement,
    GetCurrentUserEntitlements,
    GetSkus,
    GetSkuSubscriptions,
    GetSkuSubscription,
);

impl Client {
    /// Gets the entitlements of an application.
    pub fn entitlements(&self, application_id: ApplicationId) -> GetEntitlements {
        GetEntitlements {
            client: self.clone(),
            application_id,
            user_id: None,
            sku_ids: Vec::new(),
            before: None,
            after: None,
            limit: None,
            guild_id: None,
            exclude_ended: None,
            exclude_deleted: None,
        }
    }

    /// Gets one entitlement.
    pub fn entitlement(
        &self,
        application_id: ApplicationId,
        entitlement_id: EntitlementId,
    ) -> GetEntitlement {
        GetEntitlement {
            client: self.clone(),
            application_id,
            entitlement_id,
        }
    }

    /// Marks an entitlement of a consumable SKU as consumed.
    pub fn consume_entitlement(
        &self,
        application_id: ApplicationId,
        entitlement_id: EntitlementId,
    ) -> ConsumeEntitlement {
        ConsumeEntitlement {
            client: self.clone(),
            application_id,
            entitlement_id,
        }
    }

    /// Gives a test entitlement for a SKU to a guild or a user.
    pub fn create_test_entitlement(
        &self,
        application_id: ApplicationId,
        sku_id: SkuId,
        owner: TestEntitlementOwner,
    ) -> CreateTestEntitlement {
        CreateTestEntitlement {
            client: self.clone(),
            application_id,
            sku_id,
            owner,
        }
    }

    /// Deletes a test entitlement.
    pub fn delete_test_entitlement(
        &self,
        application_id: ApplicationId,
        entitlement_id: EntitlementId,
    ) -> DeleteTestEntitlement {
        DeleteTestEntitlement {
            client: self.clone(),
            application_id,
            entitlement_id,
        }
    }

    /// Gets the entitlements of the current user for an application. Needs a Bearer token.
    pub fn current_user_entitlements(
        &self,
        application_id: ApplicationId,
    ) -> GetCurrentUserEntitlements {
        GetCurrentUserEntitlements {
            client: self.clone(),
            application_id,
            sku_ids: Vec::new(),
            exclude_consumed: None,
        }
    }

    /// Gets the SKUs of an application.
    pub fn skus(&self, application_id: ApplicationId) -> GetSkus {
        GetSkus {
            client: self.clone(),
            application_id,
        }
    }

    /// Gets the subscriptions that contain a SKU.
    pub fn sku_subscriptions(&self, sku_id: SkuId) -> GetSkuSubscriptions {
        GetSkuSubscriptions {
            client: self.clone(),
            sku_id,
            before: None,
            after: None,
            limit: None,
            user_id: None,
        }
    }

    /// Gets one subscription of a SKU.
    pub fn sku_subscription(
        &self,
        sku_id: SkuId,
        subscription_id: SubscriptionId,
    ) -> GetSkuSubscription {
        GetSkuSubscription {
            client: self.clone(),
            sku_id,
            subscription_id,
        }
    }
}
