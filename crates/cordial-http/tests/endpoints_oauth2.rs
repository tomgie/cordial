//! Tests of the OAuth2 endpoints and URL builders.

mod common;

use common::{
    assert_no_body, assert_request, bearer_client, bot_client, form, only_request, respond,
    respond_empty, tokenless_client,
};
use cordial_http::endpoints::oauth2::{
    AuthorizationUrl, BotAuthorizationUrl, Prompt, TokenTypeHint, is_discord_issuer,
};
use cordial_http::{Error, TokenKind};
use cordial_model::Permissions;
use cordial_model::application::ApplicationIntegrationType;
use cordial_model::id::{ApplicationId, GuildId};
use cordial_model::oauth2::Scope;
use serde_json::json;

const CLIENT_ID: ApplicationId = ApplicationId::new_const(332_269_999_912_132_097);

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

#[test]
fn builds_authorization_url() {
    let url = AuthorizationUrl::new(ApplicationId::new_const(157_730_590_492_196_864))
        .scopes([Scope::Identify, Scope::GuildsJoin])
        .state("15773059ghq9183habn")
        .redirect_uri("https://nicememe.website")
        .prompt(Prompt::Consent)
        .integration_type(ApplicationIntegrationType::GuildInstall)
        .build();

    assert_eq!(
        url,
        "https://discord.com/oauth2/authorize?response_type=code&client_id=157730590492196864&scope=identify%20guilds.join&state=15773059ghq9183habn&redirect_uri=https%3A%2F%2Fnicememe.website&prompt=consent&integration_type=0"
    );
}

#[test]
fn builds_authorization_url_with_pkce() {
    let url = AuthorizationUrl::new(ApplicationId::new_const(1))
        .scopes([Scope::Identify])
        .prompt(Prompt::None)
        .code_challenge("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM")
        .to_string();

    assert_eq!(
        url,
        "https://discord.com/oauth2/authorize?response_type=code&client_id=1&scope=identify&prompt=none&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256"
    );
}

#[test]
fn builds_bot_authorization_url() {
    let url = BotAuthorizationUrl::new(ApplicationId::new_const(157_730_590_492_196_864))
        .scopes([Scope::Bot, Scope::ApplicationsCommands])
        .permissions(Permissions::from_bits_retain(8))
        .guild(GuildId::new_const(41))
        .disable_guild_select(true)
        .integration_type(ApplicationIntegrationType::GuildInstall)
        .build();

    assert_eq!(
        url,
        "https://discord.com/oauth2/authorize?client_id=157730590492196864&scope=bot%20applications.commands&permissions=8&guild_id=41&disable_guild_select=true&integration_type=0"
    );
}

#[test]
fn bot_authorization_url_without_scopes_uses_default_install_settings() {
    let url = BotAuthorizationUrl::new(ApplicationId::new_const(5))
        .scopes([])
        .build();

    assert_eq!(url, "https://discord.com/oauth2/authorize?client_id=5");
}

#[test]
fn checks_issuer() {
    assert!(is_discord_issuer("https://discord.com"));
    assert!(!is_discord_issuer("https://discord.com/"));
    assert!(!is_discord_issuer("https://evil.example"));
}

#[tokio::test]
async fn exchanges_code_with_pkce() {
    let transport = respond(
        200,
        &json!({
            "access_token": "6qrZcUqja7812RVdnEKjpzOL4CvHBFG",
            "token_type": "Bearer",
            "expires_in": 604800,
            "refresh_token": "D43f5y0ahjqew82jZ4NViEr2YafMKhue",
            "scope": "identify"
        }),
    );

    let token = tokenless_client(&transport)
        .exchange_code(
            CLIENT_ID,
            "937it3ow87i4ery69876wqire",
            "NhhvTDYsFcdgNLnnLijcl7Ku7bEEeee",
            "https://nicememe.website",
        )
        .code_verifier("verifier-1")
        .await
        .unwrap();

    assert_eq!(token.expires_in, 604_800);
    assert_eq!(token.scopes().collect::<Vec<_>>(), vec![Scope::Identify]);
    assert_eq!(token.bearer_token().kind(), TokenKind::Bearer);
    let debug = format!("{token:?}");
    assert!(!debug.contains("6qrZ") && !debug.contains("D43f"));
    let request = only_request(&transport);
    assert_request(&request, "POST", "/oauth2/token", None);
    assert_eq!(
        form(&request),
        pairs(&[
            ("grant_type", "authorization_code"),
            ("code", "NhhvTDYsFcdgNLnnLijcl7Ku7bEEeee"),
            ("redirect_uri", "https://nicememe.website"),
            ("code_verifier", "verifier-1"),
            ("client_id", "332269999912132097"),
            ("client_secret", "937it3ow87i4ery69876wqire"),
        ])
    );
}

#[tokio::test]
async fn exchange_works_on_a_bot_client_without_its_token() {
    let transport = respond(
        200,
        &json!({"access_token": "a", "token_type": "Bearer", "expires_in": 1, "scope": "identify"}),
    );

    bot_client(&transport)
        .exchange_code(CLIENT_ID, "secret", "code", "https://example.com")
        .await
        .unwrap();

    assert_eq!(only_request(&transport).header("authorization"), None);
}

#[tokio::test]
async fn refreshes_token() {
    let transport = respond(
        200,
        &json!({
            "access_token": "new",
            "token_type": "Bearer",
            "expires_in": 604800,
            "refresh_token": "new-refresh",
            "scope": "identify guilds"
        }),
    );

    let token = tokenless_client(&transport)
        .refresh_access_token(CLIENT_ID, "secret", "old-refresh")
        .await
        .unwrap();

    assert_eq!(token.refresh_token.as_deref(), Some("new-refresh"));
    let request = only_request(&transport);
    assert_request(&request, "POST", "/oauth2/token", None);
    assert_eq!(
        form(&request),
        pairs(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", "old-refresh"),
            ("client_id", "332269999912132097"),
            ("client_secret", "secret"),
        ])
    );
}

#[tokio::test]
async fn exchanges_client_credentials() {
    let transport = respond(
        200,
        &json!({
            "access_token": "6qrZcUqja7812RVdnEKjpzOL4CvHBFG",
            "token_type": "Bearer",
            "expires_in": 604800,
            "scope": "identify connections"
        }),
    );

    let token = tokenless_client(&transport)
        .exchange_client_credentials(CLIENT_ID, "secret")
        .scopes([Scope::Identify, Scope::Connections])
        .await
        .unwrap();

    assert_eq!(token.refresh_token, None);
    let request = only_request(&transport);
    assert_eq!(
        form(&request),
        pairs(&[
            ("grant_type", "client_credentials"),
            ("scope", "identify connections"),
            ("client_id", "332269999912132097"),
            ("client_secret", "secret"),
        ])
    );
}

#[tokio::test]
async fn decodes_bot_and_webhook_token_responses() {
    let transport = respond(
        200,
        &json!({
            "token_type": "Bearer",
            "guild": {
                "mfa_level": 0,
                "emojis": [],
                "application_id": null,
                "name": "SomeTest",
                "roles": [{
                    "hoist": false,
                    "name": "@everyone",
                    "mentionable": false,
                    "color": 0,
                    "position": 0,
                    "id": "290926798626357250",
                    "managed": false,
                    "permissions": 49794241,
                    "permissions_new": "49794241"
                }],
                "afk_timeout": 300,
                "system_channel_id": null,
                "widget_channel_id": null,
                "region": "us-east",
                "default_message_notifications": 1,
                "explicit_content_filter": 0,
                "splash": null,
                "features": [],
                "afk_channel_id": null,
                "widget_enabled": false,
                "verification_level": 0,
                "owner_id": "53908232999183680",
                "id": "2909267986347357250",
                "icon": null,
                "description": null,
                "public_updates_channel_id": null,
                "safety_alerts_channel_id": null,
                "rules_channel_id": null,
                "max_members": 100000,
                "vanity_url_code": null,
                "premium_subscription_count": 0,
                "premium_tier": 0,
                "preferred_locale": "en-US",
                "system_channel_flags": 0,
                "banner": null,
                "max_presences": null,
                "discovery_splash": null,
                "max_video_channel_users": 25
            },
            "access_token": "zMndOe7jFLXGawdlxMOdNvXjjOce5X",
            "scope": "bot",
            "expires_in": 604800,
            "refresh_token": "mgp8qnvBwJcmadwgCYKyYD5CAzGAX4"
        }),
    );
    transport.push(cordial_http::testing::FakeResponse::json(
        200,
        &json!({
            "token_type": "Bearer",
            "access_token": "GNaVzEtATqdh173tNHEXY9ZYAuhiYxvy",
            "scope": "webhook.incoming",
            "expires_in": 604800,
            "refresh_token": "PvPL7ELyMDc1836457XCDh1Y8jPbRm",
            "webhook": {
                "application_id": "310954232226357250",
                "name": "testwebhook",
                "url": "https://discord.com/api/webhooks/347114750880120863/kKDdjXa1g9tKNs0-_yOwLyALC9gydEWP6gr9sHabuK1vuofjhQDDnlOclJeRIvYK-pj_",
                "channel_id": "345626669224982402",
                "token": "kKDdjXa1g9tKNs0-_yOwLyALC9gydEWP6gr9sHabuK1vuofjhQDDnlOclJeRIvYK-pj_",
                "type": 1,
                "avatar": null,
                "guild_id": "290926792226357250",
                "id": "347114750880120863"
            }
        }),
    ));
    let client = tokenless_client(&transport);

    let bot = client
        .exchange_code(CLIENT_ID, "secret", "code", "https://example.com")
        .await
        .unwrap();
    let webhook = client
        .exchange_code(CLIENT_ID, "secret", "code", "https://example.com")
        .await
        .unwrap();

    assert_eq!(bot.guild.unwrap().name, "SomeTest");
    let webhook = webhook.webhook.unwrap();
    assert_eq!(webhook.id.get(), 347_114_750_880_120_863);
    assert!(webhook.token.is_some());
}

#[tokio::test]
async fn revokes_token() {
    let transport = respond_empty();

    tokenless_client(&transport)
        .revoke_token(CLIENT_ID, "secret", "access")
        .token_type_hint(TokenTypeHint::AccessToken)
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_request(&request, "POST", "/oauth2/token/revoke", None);
    assert_eq!(
        form(&request),
        pairs(&[
            ("token", "access"),
            ("token_type_hint", "access_token"),
            ("client_id", "332269999912132097"),
            ("client_secret", "secret"),
        ])
    );
}

#[tokio::test]
async fn token_error_does_not_mark_the_client_token_invalid() {
    let transport = cordial_http::testing::FakeTransport::new();
    transport.push(cordial_http::testing::FakeResponse::json(
        401,
        &json!({"error": "invalid_client"}),
    ));
    transport.push(cordial_http::testing::FakeResponse::json(
        200,
        &json!({"id": "1", "name": "app", "description": "", "verify_key": "k"}),
    ));
    let client = bot_client(&transport);

    let error = client
        .revoke_token(CLIENT_ID, "wrong", "access")
        .await
        .unwrap_err();
    client.current_bot_application().await.unwrap();

    assert!(matches!(error, Error::Api(_)));
}

#[tokio::test]
async fn gets_current_bot_application() {
    let transport = respond(
        200,
        &json!({
            "id": "159799960412356608",
            "name": "AIRHORN SOLUTIONS",
            "icon": "f03590d3eb764081d154a66340ea7d6d",
            "description": "",
            "bot_public": true,
            "bot_require_code_grant": false,
            "verify_key": "c8cde6a3c8c6e49d86af3191287b3ce255872be1fff6dc285bdb420c06a2c3c8",
            "flags": 0,
            "redirect_uris": [],
            "interactions_endpoint_url": null,
            "role_connections_verification_url": null
        }),
    );

    let application = bot_client(&transport)
        .current_bot_application()
        .await
        .unwrap();

    assert_eq!(application.name, "AIRHORN SOLUTIONS");
    let request = only_request(&transport);
    assert_request(&request, "GET", "/oauth2/applications/@me", Some("Bot abc"));
    assert_no_body(&request);
}

#[tokio::test]
async fn gets_current_authorization() {
    let transport = respond(
        200,
        &json!({
            "application": {
                "id": "159799960412356608",
                "name": "AIRHORN SOLUTIONS",
                "icon": "f03590d3eb764081d154a66340ea7d6d",
                "description": "",
                "hook": true,
                "bot_public": true,
                "bot_require_code_grant": false,
                "verify_key": "c8cde6a3c8c6e49d86af3191287b3ce255872be1fff6dc285bdb420c06a2c3c8"
            },
            "scopes": ["guilds.join", "identify"],
            "expires": "2021-01-23T02:33:17.017000+00:00",
            "user": {
                "id": "268473310986240001",
                "username": "discord",
                "avatar": "f749bb0cbeeb26ef21eca719337d20f1",
                "discriminator": "0",
                "global_name": "Discord",
                "public_flags": 131072
            }
        }),
    );

    let information = bearer_client(&transport)
        .current_authorization()
        .await
        .unwrap();

    assert_eq!(information.scopes, vec![Scope::GuildsJoin, Scope::Identify]);
    assert_eq!(information.user.unwrap().username, "discord");
    let request = only_request(&transport);
    assert_request(&request, "GET", "/oauth2/@me", Some("Bearer user-token"));
}

#[tokio::test]
async fn gets_oauth2_keys_without_authorization() {
    let transport = respond(
        200,
        &json!({"keys": [{
            "kty": "RSA",
            "use": "sig",
            "kid": "key-1",
            "n": "sXchDaQebHnPiGvyDOAT4saGEUetSyo9MKLOoWFsueri23bOdgWp4Dy1WlUzewbgBHod5pcM9H95GQRV3JDXboIRROSBigeC5yjU1hGzHHyXss8UDprecbAYxknTcQkhslANGRUZmdTOQ5qTRsLAt6BTYuyvVRdhS8exSZEy_c4gs_7svlJJQ4H9_NxsiIoLwAEk7-Q3UXERGYw_75IDrGA84-lA_-Ct4eTlXHBIY2EaV7t7LjJaynVJCpkv4LKjTTAumiGUIuQhrNhZLuF_RJLqHpM2kgWFLU7-VTdL1VbC2tejvcI2BlMkEpk1BzBZI0KQB0GaDWFLN-aEAw3vRw",
            "e": "AQAB",
            "alg": "RS256"
        }]}),
    );

    let key_set = tokenless_client(&transport).oauth2_keys().await.unwrap();

    assert_eq!(key_set.keys[0].key_use, "sig");
    let request = only_request(&transport);
    assert_request(&request, "GET", "/oauth2/keys", None);
}

#[tokio::test]
async fn gets_oauth2_user_info() {
    let transport = respond(
        200,
        &json!({
            "sub": "268473310986240001",
            "email": null,
            "preferred_username": "discord",
            "nickname": "Discord",
            "picture": "https://cdn.discordapp.com/avatars/268473310986240001/f749bb0cbeeb26ef21eca719337d20f1.png",
            "locale": "en-US"
        }),
    );

    let info = bearer_client(&transport).oauth2_user_info().await.unwrap();

    assert_eq!(info.sub, "268473310986240001");
    assert_eq!(info.email, None);
    let request = only_request(&transport);
    assert_request(
        &request,
        "GET",
        "/oauth2/userinfo",
        Some("Bearer user-token"),
    );
}
