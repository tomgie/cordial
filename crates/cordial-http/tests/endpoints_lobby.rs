//! Tests of the lobby, application identity, and partner SDK endpoints.

mod common;

use std::collections::HashMap;

use common::{
    assert_json, assert_no_body, assert_request, bearer_client, bot_client, only_request, respond,
    respond_empty, tokenless_client, user_json,
};
use cordial_http::Error;
use cordial_http::endpoints::lobby::LobbyMemberUpdate;
use cordial_http::endpoints::partner_sdk::ExternalAuthType;
use cordial_model::id::{ApplicationId, ChannelId, LobbyId, MessageId, UserId};
use cordial_model::lobby::{LobbyMemberFlags, PrimaryProfileData, ProfileData};
use cordial_model::message::MessageFlags;
use serde_json::json;

const LOBBY: LobbyId = LobbyId::new_const(96_008_815_106_887_111);
const USER: UserId = UserId::new_const(41_771_983_429_993_000);
const APPLICATION: ApplicationId = ApplicationId::new_const(41_771_983_429_993_937);

fn lobby_json() -> serde_json::Value {
    json!({
        "id": "96008815106887111",
        "application_id": "41771983429993937",
        "metadata": {"topic": "we need more redstone"},
        "members": [{
            "id": "41771983429993000",
            "metadata": null,
            "flags": 1,
            "additional_name": "Twindly Shimmerleaf"
        }]
    })
}

fn lobby_message_json() -> serde_json::Value {
    json!({
        "id": "1234",
        "type": 0,
        "content": "gg",
        "lobby_id": "96008815106887111",
        "channel_id": "96008815106887111",
        "author": user_json(),
        "lobby_member": {"additional_name": "Twindly Shimmerleaf"},
        "metadata": null,
        "flags": 0,
        "application_id": "41771983429993937"
    })
}

fn metadata(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

fn profile_json() -> serde_json::Value {
    json!({
        "username": "johndoe123",
        "metadata": null,
        "data": {
            "primary": {
                "season": "Season 3",
                "rank_name": "Silver",
                "rank_image": {"url": "https://example.com/assets/rank-images/silver.png"},
                "playtime_hours": 69.41,
                "total_wins": 57
            },
            "dynamic": [
                {"type": 1, "name": "my_string", "value": "hello"},
                {"type": 2, "name": "my_number", "value": 123.45},
                {"type": 3, "name": "my_media", "value": {"url": "https://example.com/some-media.png"}}
            ]
        }
    })
}

#[tokio::test]
async fn creates_lobby() {
    let transport = respond(201, &lobby_json());

    let lobby = bot_client(&transport)
        .create_lobby()
        .metadata(metadata(&[("topic", "we need more redstone")]))
        .members([LobbyMemberUpdate::new(USER)
            .flags(LobbyMemberFlags::CAN_LINK_LOBBY)
            .additional_name("Twindly Shimmerleaf")])
        .idle_timeout_seconds(300)
        .await
        .unwrap();

    assert_eq!(lobby.id, LOBBY);
    assert_eq!(
        lobby.members[0].additional_name.as_deref(),
        Some("Twindly Shimmerleaf")
    );
    let request = only_request(&transport);
    assert_request(&request, "POST", "/lobbies", Some("Bot abc"));
    assert_json(
        &request,
        &json!({
            "metadata": {"topic": "we need more redstone"},
            "members": [{"id": "41771983429993000", "flags": 1, "additional_name": "Twindly Shimmerleaf"}],
            "idle_timeout_seconds": 300
        }),
    );
}

#[tokio::test]
async fn rejects_bad_idle_timeout() {
    let transport = respond_empty();

    let error = bot_client(&transport)
        .create_lobby()
        .idle_timeout_seconds(4)
        .await
        .unwrap_err();

    assert!(matches!(error, Error::Validation(_)));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn creates_or_joins_lobby_with_bearer_token() {
    let transport = respond(200, &lobby_json());

    bearer_client(&transport)
        .create_or_join_lobby("secret-1")
        .lobby_metadata(metadata(&[("mode", "duo")]))
        .member_metadata(metadata(&[("team", "red")]))
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_request(&request, "PUT", "/lobbies", Some("Bearer user-token"));
    assert_json(
        &request,
        &json!({"secret": "secret-1", "lobby_metadata": {"mode": "duo"}, "member_metadata": {"team": "red"}}),
    );
}

#[tokio::test]
async fn gets_lobby() {
    let transport = respond(200, &lobby_json());

    let lobby = bot_client(&transport).lobby(LOBBY).await.unwrap();

    assert_eq!(
        lobby.members[0].flags,
        Some(LobbyMemberFlags::CAN_LINK_LOBBY)
    );
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/lobbies/96008815106887111",
        Some("Bot abc"),
    );
    assert_no_body(&request);
}

#[tokio::test]
async fn edits_lobby() {
    let transport = respond(200, &lobby_json());

    bot_client(&transport)
        .edit_lobby(LOBBY)
        .clear_metadata()
        .members([LobbyMemberUpdate::new(USER).clear_additional_name()])
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_request(
        &request,
        "PATCH",
        "/lobbies/96008815106887111",
        Some("Bot abc"),
    );
    assert_json(
        &request,
        &json!({"metadata": null, "members": [{"id": "41771983429993000", "additional_name": null}]}),
    );
}

#[tokio::test]
async fn deletes_lobby() {
    let transport = respond_empty();

    bot_client(&transport).delete_lobby(LOBBY).await.unwrap();

    let request = only_request(&transport);
    assert_request(
        &request,
        "DELETE",
        "/lobbies/96008815106887111",
        Some("Bot abc"),
    );
}

#[tokio::test]
async fn adds_lobby_member() {
    let transport = respond(
        200,
        &json!({"id": "41771983429993000", "metadata": {"rank": "gold"}, "flags": 0}),
    );

    let member = bot_client(&transport)
        .add_lobby_member(LOBBY, USER)
        .metadata(metadata(&[("rank", "gold")]))
        .await
        .unwrap();

    assert_eq!(member.id, USER);
    let request = only_request(&transport);
    assert_request(
        &request,
        "PUT",
        "/lobbies/96008815106887111/members/41771983429993000",
        Some("Bot abc"),
    );
    assert_json(&request, &json!({"metadata": {"rank": "gold"}}));
}

#[tokio::test]
async fn bulk_updates_lobby_members() {
    let transport = respond(
        200,
        &json!([{"id": "41771983429993000", "metadata": null, "flags": 1}]),
    );

    let members = bot_client(&transport)
        .bulk_update_lobby_members(
            LOBBY,
            [
                LobbyMemberUpdate::new(USER).flags(LobbyMemberFlags::CAN_LINK_LOBBY),
                LobbyMemberUpdate::new(UserId::new_const(7)).remove(),
            ],
        )
        .await
        .unwrap();

    assert_eq!(members.len(), 1);
    let request = only_request(&transport);
    assert_request(
        &request,
        "POST",
        "/lobbies/96008815106887111/members/bulk",
        Some("Bot abc"),
    );
    assert_json(
        &request,
        &json!([
            {"id": "41771983429993000", "flags": 1},
            {"id": "7", "remove_member": true}
        ]),
    );
}

#[tokio::test]
async fn rejects_empty_bulk_update() {
    let transport = respond_empty();

    let error = bot_client(&transport)
        .bulk_update_lobby_members(LOBBY, [])
        .await
        .unwrap_err();

    assert!(matches!(error, Error::Validation(_)));
}

#[tokio::test]
async fn removes_lobby_member() {
    let transport = respond_empty();

    bot_client(&transport)
        .remove_lobby_member(LOBBY, USER)
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_request(
        &request,
        "DELETE",
        "/lobbies/96008815106887111/members/41771983429993000",
        Some("Bot abc"),
    );
}

#[tokio::test]
async fn leaves_lobby_with_bearer_token() {
    let transport = respond_empty();

    bearer_client(&transport).leave_lobby(LOBBY).await.unwrap();

    let request = only_request(&transport);
    assert_request(
        &request,
        "DELETE",
        "/lobbies/96008815106887111/members/@me",
        Some("Bearer user-token"),
    );
}

#[tokio::test]
async fn links_and_unlinks_lobby_channel() {
    let mut linked = lobby_json();
    linked["linked_channel"] =
        json!({"id": "555", "type": 0, "guild_id": "1", "name": "lobby-chat", "position": 0});
    let transport = respond(200, &linked);
    transport.push(cordial_http::testing::FakeResponse::json(
        200,
        &lobby_json(),
    ));
    let client = bearer_client(&transport);

    let lobby = client
        .link_lobby_channel(LOBBY, ChannelId::new_const(555))
        .await
        .unwrap();
    client.unlink_lobby_channel(LOBBY).await.unwrap();

    assert_eq!(lobby.linked_channel.unwrap().id.get(), 555);
    let requests = transport.requests();
    assert_request(
        &requests[0],
        "PATCH",
        "/lobbies/96008815106887111/channel-linking",
        Some("Bearer user-token"),
    );
    assert_json(&requests[0], &json!({"channel_id": "555"}));
    assert_json(&requests[1], &json!({}));
}

#[tokio::test]
async fn sends_lobby_message() {
    let transport = respond(200, &lobby_message_json());

    let message = bearer_client(&transport)
        .create_lobby_message(LOBBY, "gg")
        .metadata(metadata(&[("emote", "wave")]))
        .flags(MessageFlags::empty())
        .await
        .unwrap();

    assert_eq!(message.lobby_id, LOBBY);
    assert_eq!(
        message.lobby_member.unwrap().additional_name.as_deref(),
        Some("Twindly Shimmerleaf")
    );
    let request = only_request(&transport);
    assert_request(
        &request,
        "POST",
        "/lobbies/96008815106887111/messages",
        Some("Bearer user-token"),
    );
    assert_json(
        &request,
        &json!({"content": "gg", "metadata": {"emote": "wave"}, "flags": 0}),
    );
}

#[tokio::test]
async fn rejects_empty_lobby_message() {
    let transport = respond_empty();

    let error = bearer_client(&transport)
        .create_lobby_message(LOBBY, "")
        .await
        .unwrap_err();

    assert!(matches!(error, Error::Validation(_)));
}

#[tokio::test]
async fn gets_lobby_messages() {
    let transport = respond(200, &json!([lobby_message_json()]));

    let messages = bearer_client(&transport)
        .lobby_messages(LOBBY)
        .limit(250)
        .await
        .unwrap();

    assert_eq!(messages[0].content, "gg");
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/lobbies/96008815106887111/messages?limit=200",
        Some("Bearer user-token"),
    );
}

#[tokio::test]
async fn updates_lobby_message_moderation_metadata() {
    let transport = respond_empty();

    bot_client(&transport)
        .update_lobby_message_moderation_metadata(
            LOBBY,
            MessageId::new_const(1234),
            metadata(&[("action", "hide"), ("reason", "toxicity")]),
        )
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_request(
        &request,
        "PUT",
        "/lobbies/96008815106887111/messages/1234/moderation-metadata",
        Some("Bot abc"),
    );
    assert_json(&request, &json!({"action": "hide", "reason": "toxicity"}));
}

#[tokio::test]
async fn rejects_too_many_moderation_keys() {
    let transport = respond_empty();
    let pairs: Vec<(String, String)> = (0..6).map(|n| (n.to_string(), "x".to_owned())).collect();

    let error = bot_client(&transport)
        .update_lobby_message_moderation_metadata(
            LOBBY,
            MessageId::new_const(1),
            pairs.into_iter().collect(),
        )
        .await
        .unwrap_err();

    assert!(matches!(error, Error::Validation(_)));
}

#[tokio::test]
async fn creates_lobby_invites() {
    let transport = respond(200, &json!({"code": "abc"}));
    transport.push(cordial_http::testing::FakeResponse::json(
        200,
        &json!({"code": "def"}),
    ));

    let own = bearer_client(&transport)
        .create_lobby_invite(LOBBY)
        .await
        .unwrap();
    let other = bot_client(&transport)
        .create_lobby_invite_for_user(LOBBY, USER)
        .await
        .unwrap();

    assert_eq!(own.code, "abc");
    assert_eq!(other.code, "def");
    let requests = transport.requests();
    assert_request(
        &requests[0],
        "POST",
        "/lobbies/96008815106887111/members/@me/invites",
        Some("Bearer user-token"),
    );
    assert_request(
        &requests[1],
        "POST",
        "/lobbies/96008815106887111/members/41771983429993000/invites",
        Some("Bot abc"),
    );
}

#[tokio::test]
async fn edits_application_identity_profile() {
    let transport = respond(201, &profile_json());
    let mut data = ProfileData::default();
    let mut primary = PrimaryProfileData::default();
    primary.season = Some("Season 3".to_owned());
    data.primary = Some(primary);

    let profile = bot_client(&transport)
        .edit_application_identity_profile(APPLICATION, USER, "player/42")
        .username("johndoe123")
        .data(data)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(profile.username.as_deref(), Some("johndoe123"));
    let request = only_request(&transport);
    assert_request(
        &request,
        "PATCH",
        "/applications/41771983429993937/users/41771983429993000/identities/player%2F42/profile",
        Some("Bot abc"),
    );
    assert_json(
        &request,
        &json!({"username": "johndoe123", "data": {"primary": {"season": "Season 3"}}}),
    );
}

#[tokio::test]
async fn edits_application_identity_profile_with_no_content() {
    let transport = respond_empty();

    let profile = bot_client(&transport)
        .edit_application_identity_profile(APPLICATION, USER, "42")
        .username("johndoe123")
        .await
        .unwrap();

    assert_eq!(profile, None);
}

#[tokio::test]
async fn gets_application_identity_profile() {
    let transport = respond(200, &profile_json());

    let profile = bot_client(&transport)
        .application_identity_profile(APPLICATION, USER, "42")
        .await
        .unwrap();

    let data = profile.data.unwrap();
    assert_eq!(data.dynamic.unwrap().len(), 3);
    assert_eq!(data.primary.unwrap().total_wins, Some(57));
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/applications/41771983429993937/users/41771983429993000/identities/42/profile",
        Some("Bot abc"),
    );
}

#[tokio::test]
async fn gets_user_application_identities() {
    let transport = respond(
        200,
        &json!({"identities": [{
            "user_id": "41771983429993000",
            "provider_type": "STEAM",
            "provider_id": "app-1",
            "provider_issued_user_id": "76561198000000000"
        }]}),
    );

    let identities = bot_client(&transport)
        .user_application_identities(USER, APPLICATION)
        .await
        .unwrap();

    let first = &identities.identities[0];
    assert_eq!(first.user_id, Some(USER));
    assert_eq!(first.provider_type, "STEAM");
    assert_eq!(first.provider_id.as_deref(), Some("app-1"));
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/users/41771983429993000/application-identities/41771983429993937",
        Some("Bot abc"),
    );
}

#[tokio::test]
async fn gets_application_identities_by_external_id() {
    let transport = respond(200, &json!({"identities": []}));

    let identities = bot_client(&transport)
        .application_identities_by_external_id(APPLICATION, "OIDC", "user 1")
        .provider_id("issuer")
        .await
        .unwrap();

    assert!(identities.identities.is_empty());
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/applications/41771983429993937/application-identities/OIDC/user%201?provider_id=issuer",
        Some("Bot abc"),
    );
}

#[tokio::test]
async fn deletes_application_identity() {
    let transport = respond_empty();
    transport.push(cordial_http::testing::FakeResponse::empty(204));
    let client = bot_client(&transport);

    client
        .delete_application_identity(USER, APPLICATION, "STEAM", "765")
        .await
        .unwrap();
    client
        .delete_application_identity(USER, APPLICATION, "STEAM", "765")
        .provider_id("app-1")
        .await
        .unwrap();

    let requests = transport.requests();
    assert_request(
        &requests[0],
        "POST",
        "/users/41771983429993000/application-identities/41771983429993937/STEAM/765/delete",
        Some("Bot abc"),
    );
    assert_no_body(&requests[0]);
    assert_json(&requests[1], &json!({"provider_id": "app-1"}));
}

fn provisional_token_json() -> serde_json::Value {
    json!({
        "token_type": "Bearer",
        "access_token": "provisional-access",
        "expires_in": 3600,
        "scope": "sdk.social_layer openid",
        "id_token": "eyJ.x.y",
        "refresh_token": "provisional-refresh",
        "scopes": ["sdk.social_layer", "openid"],
        "expires_at_s": 1_800_000_000
    })
}

#[tokio::test]
async fn creates_provisional_token_without_authorization() {
    let transport = respond(200, &provisional_token_json());

    let token = tokenless_client(&transport)
        .create_provisional_token(
            APPLICATION,
            ExternalAuthType::STEAM_SESSION_TICKET,
            "ticket",
        )
        .client_secret("secret")
        .await
        .unwrap();

    assert_eq!(token.expires_in, 3600);
    assert_eq!(token.bearer_token().secret(), "provisional-access");
    assert!(!format!("{token:?}").contains("provisional-access"));
    let request = only_request(&transport);
    assert_request(&request, "POST", "/partner-sdk/token", None);
    assert_json(
        &request,
        &json!({
            "client_id": "41771983429993937",
            "client_secret": "secret",
            "external_auth_type": "STEAM_SESSION_TICKET",
            "external_auth_token": "ticket"
        }),
    );
}

#[tokio::test]
async fn creates_bot_provisional_token() {
    let transport = respond(200, &provisional_token_json());

    bot_client(&transport)
        .create_bot_provisional_token("player-42")
        .preferred_global_name("Player")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_request(&request, "POST", "/partner-sdk/token/bot", Some("Bot abc"));
    assert_json(
        &request,
        &json!({"external_user_id": "player-42", "preferred_global_name": "Player"}),
    );
}

#[tokio::test]
async fn unmerges_provisional_accounts() {
    let transport = respond_empty();
    transport.push(cordial_http::testing::FakeResponse::empty(204));

    tokenless_client(&transport)
        .unmerge_provisional_account(APPLICATION, ExternalAuthType::new("CUSTOM"), "token")
        .await
        .unwrap();
    bot_client(&transport)
        .unmerge_bot_provisional_account("player-42")
        .await
        .unwrap();

    let requests = transport.requests();
    assert_request(
        &requests[0],
        "POST",
        "/partner-sdk/provisional-accounts/unmerge",
        None,
    );
    assert_json(
        &requests[0],
        &json!({"client_id": "41771983429993937", "external_auth_type": "CUSTOM", "external_auth_token": "token"}),
    );
    assert_request(
        &requests[1],
        "POST",
        "/partner-sdk/provisional-accounts/unmerge/bot",
        Some("Bot abc"),
    );
    assert_json(&requests[1], &json!({"external_user_id": "player-42"}));
}

#[tokio::test]
async fn updates_dm_message_moderation_metadata() {
    let transport = respond_empty();

    bot_client(&transport)
        .update_dm_message_moderation_metadata(
            UserId::new_const(1),
            UserId::new_const(2),
            MessageId::new_const(3),
            metadata(&[("action", "censor")]),
        )
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_request(
        &request,
        "PUT",
        "/partner-sdk/dms/1/2/messages/3/moderation-metadata",
        Some("Bot abc"),
    );
    assert_json(&request, &json!({"action": "censor"}));
}
