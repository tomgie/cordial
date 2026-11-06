//! Tests of the user, entitlement, SKU, and subscription endpoints.

mod common;

use common::{
    assert_json, assert_no_body, assert_request, bearer_client, bot_client, only_request, respond,
    respond_empty, user_json,
};
use cordial_http::endpoints::monetization::TestEntitlementOwner;
use cordial_http::{Error, ImageData};
use cordial_model::id::{ApplicationId, EntitlementId, GuildId, SkuId, SubscriptionId, UserId};
use cordial_model::monetization::{EntitlementType, SkuType, SubscriptionStatus};
use cordial_model::user::ConnectionVisibility;
use serde_json::json;

const APPLICATION: ApplicationId = ApplicationId::new_const(1_019_370_614_521_200_640);
const GUILD: GuildId = GuildId::new_const(80_351_110_224_678_912);

fn entitlement_json() -> serde_json::Value {
    json!({
        "id": "1019653849998299136",
        "sku_id": "1019475255913222144",
        "application_id": "1019370614521200640",
        "user_id": "771129655544643584",
        "promotion_id": null,
        "type": 8,
        "deleted": false,
        "gift_code_flags": 0,
        "consumed": false,
        "starts_at": "2022-09-14T17:00:18.704163+00:00",
        "ends_at": "2022-10-14T17:00:18.704163+00:00",
        "guild_id": "1015034326372454400",
        "subscription_id": "1019653835926409216"
    })
}

fn subscription_json() -> serde_json::Value {
    json!({
        "id": "1278078770116427839",
        "user_id": "1088605110638227537",
        "sku_ids": ["1158857122189168803"],
        "entitlement_ids": [],
        "renewal_sku_ids": null,
        "current_period_start": "2024-08-27T19:48:44.406602+00:00",
        "current_period_end": "2024-09-27T19:48:44.406602+00:00",
        "status": 0,
        "canceled_at": null
    })
}

#[tokio::test]
async fn edits_current_user() {
    let transport = respond(200, &user_json());

    let user = bot_client(&transport)
        .edit_current_user()
        .username("Nelly")
        .avatar(ImageData::new("image/png", b"PNG"))
        .clear_banner()
        .await
        .unwrap();

    assert_eq!(user.username, "Nelly");
    let request = only_request(&transport);
    assert_request(&request, "PATCH", "/users/@me", Some("Bot abc"));
    assert_json(
        &request,
        &json!({"username": "Nelly", "avatar": "data:image/png;base64,UE5H", "banner": null}),
    );
}

#[tokio::test]
async fn gets_current_user_guilds_with_query() {
    let transport = respond(
        200,
        &json!([{
            "id": "80351110224678912",
            "name": "1337 Krew",
            "icon": "8342729096ea3675442027381ff50dfe",
            "banner": "bb42bdc37653b7cf58c4c8cc622e76cb",
            "owner": true,
            "permissions": "36953089",
            "features": ["COMMUNITY", "NEWS", "ANIMATED_ICON", "INVITE_SPLASH", "BANNER", "ROLE_ICONS"],
            "approximate_member_count": 3268,
            "approximate_presence_count": 784
        }]),
    );

    let guilds = bot_client(&transport)
        .current_user_guilds()
        .after(GuildId::new_const(5))
        .limit(250)
        .shard(2)
        .with_counts(true)
        .await
        .unwrap();

    assert_eq!(guilds[0].id, GUILD);
    assert_eq!(guilds[0].approximate_member_count, Some(3268));
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/users/@me/guilds?after=5&limit=200&shard=2&with_counts=true",
        Some("Bot abc"),
    );
    assert_no_body(&request);
}

#[tokio::test]
async fn gets_current_user_guild_member_with_bearer_token() {
    let transport = respond(
        200,
        &json!({
            "user": user_json(),
            "nick": "NOT API SUPPORT",
            "avatar": null,
            "roles": [],
            "joined_at": "2015-04-26T06:26:56.936000+00:00",
            "deaf": false,
            "mute": false,
            "flags": 0
        }),
    );

    let member = bearer_client(&transport)
        .current_user_guild_member(GUILD)
        .await
        .unwrap();

    assert_eq!(member.nick.as_deref(), Some("NOT API SUPPORT"));
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/users/@me/guilds/80351110224678912/member",
        Some("Bearer user-token"),
    );
}

#[tokio::test]
async fn leaves_guild() {
    let transport = respond_empty();

    bot_client(&transport).leave_guild(GUILD).await.unwrap();

    let request = only_request(&transport);
    assert_request(
        &request,
        "DELETE",
        "/users/@me/guilds/80351110224678912",
        Some("Bot abc"),
    );
    assert_no_body(&request);
}

#[tokio::test]
async fn creates_dm() {
    let transport = respond(
        200,
        &json!({"id": "319674150115610528", "type": 1, "last_message_id": null, "recipients": [user_json()]}),
    );

    let channel = bot_client(&transport)
        .create_dm(UserId::new_const(80_351_110_224_678_912))
        .await
        .unwrap();

    assert_eq!(channel.id.get(), 319_674_150_115_610_528);
    let request = only_request(&transport);
    assert_request(&request, "POST", "/users/@me/channels", Some("Bot abc"));
    assert_json(&request, &json!({"recipient_id": "80351110224678912"}));
}

#[tokio::test]
async fn creates_group_dm() {
    let transport = respond(
        200,
        &json!({"id": "319674150115610529", "type": 3, "last_message_id": null, "recipients": [user_json()]}),
    );

    bot_client(&transport)
        .create_group_dm(["token-a", "token-b"])
        .nick(UserId::new_const(7), "Seven")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_request(&request, "POST", "/users/@me/channels", Some("Bot abc"));
    assert_json(
        &request,
        &json!({"access_tokens": ["token-a", "token-b"], "nicks": {"7": "Seven"}}),
    );
}

#[tokio::test]
async fn gets_current_user_connections() {
    let transport = respond(
        200,
        &json!([{
            "id": "12345",
            "name": "nelly_streams",
            "type": "twitch",
            "verified": true,
            "friend_sync": false,
            "show_activity": true,
            "two_way_link": false,
            "visibility": 1
        }]),
    );

    let connections = bearer_client(&transport)
        .current_user_connections()
        .await
        .unwrap();

    assert_eq!(connections[0].name, "nelly_streams");
    assert_eq!(connections[0].visibility, ConnectionVisibility::Everyone);
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/users/@me/connections",
        Some("Bearer user-token"),
    );
}

#[tokio::test]
async fn gets_role_connection() {
    let transport = respond(
        200,
        &json!({"platform_name": "Game", "platform_username": "nelly", "metadata": {"level": "12"}}),
    );

    let connection = bearer_client(&transport)
        .current_user_role_connection(APPLICATION)
        .await
        .unwrap();

    assert_eq!(connection.metadata["level"], "12");
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/users/@me/applications/1019370614521200640/role-connection",
        Some("Bearer user-token"),
    );
}

#[tokio::test]
async fn updates_role_connection() {
    let transport = respond(
        200,
        &json!({"platform_name": "Game", "platform_username": null, "metadata": {"level": "12"}}),
    );

    let connection = bearer_client(&transport)
        .update_current_user_role_connection(APPLICATION)
        .platform_name("Game")
        .metadata("level", "12")
        .await
        .unwrap();

    assert_eq!(connection.platform_name.as_deref(), Some("Game"));
    let request = only_request(&transport);
    assert_request(
        &request,
        "PUT",
        "/users/@me/applications/1019370614521200640/role-connection",
        Some("Bearer user-token"),
    );
    assert_json(
        &request,
        &json!({"platform_name": "Game", "metadata": {"level": "12"}}),
    );
}

#[tokio::test]
async fn rejects_long_role_connection_name() {
    let transport = respond_empty();

    let error = bearer_client(&transport)
        .update_current_user_role_connection(APPLICATION)
        .platform_name("x".repeat(51))
        .await
        .unwrap_err();

    assert!(matches!(error, Error::Validation(_)));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn deletes_role_connection() {
    let transport = respond_empty();

    bearer_client(&transport)
        .delete_current_user_role_connection(APPLICATION)
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_request(
        &request,
        "DELETE",
        "/users/@me/applications/1019370614521200640/role-connection",
        Some("Bearer user-token"),
    );
}

#[tokio::test]
async fn lists_entitlements_with_query() {
    let transport = respond(200, &json!([entitlement_json()]));

    let entitlements = bot_client(&transport)
        .entitlements(APPLICATION)
        .user(UserId::new_const(771_129_655_544_643_584))
        .sku_ids([SkuId::new_const(1), SkuId::new_const(2)])
        .before(EntitlementId::new_const(9))
        .limit(0)
        .guild(GuildId::new_const(3))
        .exclude_ended(true)
        .exclude_deleted(false)
        .await
        .unwrap();

    assert_eq!(
        entitlements[0].kind,
        EntitlementType::ApplicationSubscription
    );
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/applications/1019370614521200640/entitlements?user_id=771129655544643584&sku_ids=1%2C2&before=9&limit=1&guild_id=3&exclude_ended=true&exclude_deleted=false",
        Some("Bot abc"),
    );
}

#[tokio::test]
async fn gets_entitlement() {
    let transport = respond(200, &entitlement_json());

    let entitlement = bot_client(&transport)
        .entitlement(
            APPLICATION,
            EntitlementId::new_const(1_019_653_849_998_299_136),
        )
        .await
        .unwrap();

    assert_eq!(entitlement.consumed, Some(false));
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/applications/1019370614521200640/entitlements/1019653849998299136",
        Some("Bot abc"),
    );
}

#[tokio::test]
async fn consumes_entitlement() {
    let transport = respond_empty();

    bot_client(&transport)
        .consume_entitlement(APPLICATION, EntitlementId::new_const(5))
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_request(
        &request,
        "POST",
        "/applications/1019370614521200640/entitlements/5/consume",
        Some("Bot abc"),
    );
    assert_no_body(&request);
}

#[tokio::test]
async fn creates_test_entitlement() {
    let transport = respond(
        200,
        &json!({
            "id": "1",
            "sku_id": "999184799365857331",
            "application_id": "1019370614521200640",
            "guild_id": "847184799365857999",
            "type": 4,
            "deleted": false
        }),
    );

    let entitlement = bot_client(&transport)
        .create_test_entitlement(
            APPLICATION,
            SkuId::new_const(999_184_799_365_857_331),
            TestEntitlementOwner::Guild(GuildId::new_const(847_184_799_365_857_999)),
        )
        .await
        .unwrap();

    assert_eq!(entitlement.kind, EntitlementType::TestModePurchase);
    assert_eq!(entitlement.starts_at, None);
    let request = only_request(&transport);
    assert_request(
        &request,
        "POST",
        "/applications/1019370614521200640/entitlements",
        Some("Bot abc"),
    );
    assert_json(
        &request,
        &json!({"sku_id": "999184799365857331", "owner_id": "847184799365857999", "owner_type": 1}),
    );
}

#[tokio::test]
async fn deletes_test_entitlement() {
    let transport = respond_empty();

    bot_client(&transport)
        .delete_test_entitlement(APPLICATION, EntitlementId::new_const(5))
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_request(
        &request,
        "DELETE",
        "/applications/1019370614521200640/entitlements/5",
        Some("Bot abc"),
    );
}

#[tokio::test]
async fn gets_current_user_entitlements() {
    let transport = respond(200, &json!([entitlement_json()]));

    let entitlements = bearer_client(&transport)
        .current_user_entitlements(APPLICATION)
        .sku_ids([SkuId::new_const(1019475255913222144)])
        .exclude_consumed(true)
        .await
        .unwrap();

    assert_eq!(entitlements.len(), 1);
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/users/@me/applications/1019370614521200640/entitlements?sku_ids=1019475255913222144&exclude_consumed=true",
        Some("Bearer user-token"),
    );
}

#[tokio::test]
async fn lists_skus() {
    let transport = respond(
        200,
        &json!([{
            "id": "1088510058284990888",
            "type": 5,
            "dependent_sku_id": null,
            "application_id": "788708323867885999",
            "manifest_labels": null,
            "access_type": 1,
            "name": "Test Premium",
            "features": [],
            "release_date": null,
            "premium": false,
            "slug": "test-premium",
            "flags": 128,
            "show_age_gate": false
        }]),
    );

    let skus = bot_client(&transport)
        .skus(ApplicationId::new_const(788_708_323_867_885_999))
        .await
        .unwrap();

    assert_eq!(skus[0].kind, SkuType::Subscription);
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/applications/788708323867885999/skus",
        Some("Bot abc"),
    );
}

#[tokio::test]
async fn lists_sku_subscriptions() {
    let transport = respond(200, &json!([subscription_json()]));

    let subscriptions = bot_client(&transport)
        .sku_subscriptions(SkuId::new_const(1_158_857_122_189_168_803))
        .user(UserId::new_const(1_088_605_110_638_227_537))
        .after(SubscriptionId::new_const(4))
        .limit(10)
        .await
        .unwrap();

    assert_eq!(subscriptions[0].status, SubscriptionStatus::Active);
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/skus/1158857122189168803/subscriptions?after=4&limit=10&user_id=1088605110638227537",
        Some("Bot abc"),
    );
}

#[tokio::test]
async fn gets_sku_subscription() {
    let transport = respond(200, &subscription_json());

    let subscription = bot_client(&transport)
        .sku_subscription(
            SkuId::new_const(1_158_857_122_189_168_803),
            SubscriptionId::new_const(1_278_078_770_116_427_839),
        )
        .await
        .unwrap();

    assert_eq!(subscription.canceled_at, None);
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/skus/1158857122189168803/subscriptions/1278078770116427839",
        Some("Bot abc"),
    );
}
