//! Tests of the member, ban, and role endpoints.

#[path = "common_guild/mod.rs"]
mod common;

use common::{
    GUILD, assert_reason, member_json, role_json, setup, setup_empty, setup_json, single_request,
    user_json,
};
use cordial_http::endpoints::role::RolePosition;
use cordial_http::testing::FakeResponse;
use cordial_http::{Error, ImageData};
use cordial_model::guild::{MemberFlags, RoleColors};
use cordial_model::id::{ChannelId, RoleId, UserId};
use cordial_model::{Color, Permissions, Timestamp};
use serde_json::json;

const USER: UserId = UserId::new_const(53_908_099_506_183_680);
const ROLE: RoleId = RoleId::new_const(41_771_983_423_143_936);

#[tokio::test]
async fn get_guild_member() {
    let (client, transport) = setup_json(&member_json());

    let member = client.guild_member(GUILD, USER).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/members/53908099506183680");
    assert_eq!(member.nick.as_deref(), Some("NOT API SUPPORT"));
    assert_eq!(member.user_id(), Some(USER));
}

#[tokio::test]
async fn list_guild_members() {
    let (client, transport) = setup_json(&json!([member_json()]));

    let members = client
        .guild_members(GUILD)
        .limit(5000)
        .after(UserId::new_const(9))
        .await
        .unwrap();

    single_request(&transport, "GET", "/guilds/100/members?limit=1000&after=9");
    assert_eq!(members.len(), 1);
}

#[tokio::test]
async fn search_guild_members() {
    let (client, transport) = setup_json(&json!([member_json()]));

    let members = client
        .search_guild_members(GUILD, "Ma son")
        .limit(10)
        .await
        .unwrap();

    single_request(
        &transport,
        "GET",
        "/guilds/100/members/search?query=Ma%20son&limit=10",
    );
    assert_eq!(members[0].roles, vec![ROLE]);
}

#[tokio::test]
async fn get_guild_member_permissions() {
    let (client, transport) = setup_json(&json!({"permissions": "2048"}));

    let computed = client.guild_member_permissions(GUILD, USER).await.unwrap();

    single_request(
        &transport,
        "GET",
        "/guilds/100/permissions/member/53908099506183680",
    );
    assert_eq!(computed.permissions, Permissions::SEND_MESSAGES);
}

#[tokio::test]
async fn add_guild_member() {
    let (client, transport) = setup(FakeResponse::json(201, &member_json()));

    let member = client
        .add_guild_member(GUILD, USER, "oauth-token")
        .nick("Mason")
        .roles(vec![ROLE])
        .mute(false)
        .deaf(true)
        .await
        .unwrap();

    let request = single_request(&transport, "PUT", "/guilds/100/members/53908099506183680");
    assert_eq!(request.header("authorization"), Some("Bot abc"));
    assert_eq!(
        request.json(),
        json!({
            "access_token": "oauth-token", "nick": "Mason", "roles": ["41771983423143936"],
            "mute": false, "deaf": true
        })
    );
    assert!(member.is_some());
}

#[tokio::test]
async fn add_guild_member_gives_none_when_already_member() {
    let (client, _transport) = setup_empty();

    let member = client
        .add_guild_member(GUILD, USER, "oauth-token")
        .await
        .unwrap();

    assert!(member.is_none());
}

#[tokio::test]
async fn add_guild_member_hides_token_in_debug() {
    let (client, _transport) = setup_empty();

    let request = client.add_guild_member(GUILD, USER, "oauth-token");

    assert!(!format!("{request:?}").contains("oauth-token"));
}

#[tokio::test]
async fn edit_guild_member() {
    let (client, transport) = setup_json(&member_json());
    let until = Timestamp::parse("2026-12-01T00:00:00+00:00").unwrap();

    client
        .edit_guild_member(GUILD, USER)
        .clear_nick()
        .roles(vec![ROLE])
        .mute(true)
        .channel_id(ChannelId::new_const(77))
        .communication_disabled_until(until)
        .flags(MemberFlags::BYPASSES_VERIFICATION)
        .reason("moderation")
        .await
        .unwrap();

    let request = single_request(&transport, "PATCH", "/guilds/100/members/53908099506183680");
    assert_reason(&request, "moderation");
    let body = request.json();
    assert_eq!(body["nick"], json!(null));
    assert_eq!(body["roles"], json!(["41771983423143936"]));
    assert_eq!(body["mute"], json!(true));
    assert_eq!(body["channel_id"], json!("77"));
    assert!(
        body["communication_disabled_until"]
            .as_str()
            .unwrap()
            .starts_with("2026-12-01T00:00:00")
    );
    assert_eq!(body["flags"], json!(4));
    assert_eq!(body.as_object().unwrap().len(), 6);
}

#[tokio::test]
async fn edit_guild_member_disconnects_and_ends_timeout() {
    let (client, transport) = setup_json(&member_json());

    client
        .edit_guild_member(GUILD, USER)
        .disconnect()
        .clear_communication_disabled_until()
        .await
        .unwrap();

    let request = single_request(&transport, "PATCH", "/guilds/100/members/53908099506183680");
    assert_eq!(request.header("x-audit-log-reason"), None);
    assert_eq!(
        request.json(),
        json!({"channel_id": null, "communication_disabled_until": null})
    );
}

#[tokio::test]
async fn edit_current_member() {
    let (client, transport) = setup_json(&member_json());

    client
        .edit_current_member(GUILD)
        .nick("Bot")
        .avatar(ImageData::from_data_uri("data:image/png;base64,AAAA"))
        .clear_banner()
        .bio("I help")
        .reason("profile")
        .await
        .unwrap();

    let request = single_request(&transport, "PATCH", "/guilds/100/members/@me");
    assert_reason(&request, "profile");
    assert_eq!(
        request.json(),
        json!({"nick": "Bot", "banner": null, "avatar": "data:image/png;base64,AAAA", "bio": "I help"})
    );
}

#[tokio::test]
#[allow(deprecated)]
async fn edit_current_user_nick() {
    let (client, transport) = setup_json(&json!({"nick": "Bot"}));

    let response = client
        .edit_current_user_nick(GUILD)
        .nick("Bot")
        .reason("nick")
        .await
        .unwrap();

    let request = single_request(&transport, "PATCH", "/guilds/100/members/@me/nick");
    assert_reason(&request, "nick");
    assert_eq!(request.json(), json!({"nick": "Bot"}));
    assert_eq!(response["nick"], json!("Bot"));
}

#[tokio::test]
async fn add_guild_member_role() {
    let (client, transport) = setup_empty();

    client
        .add_guild_member_role(GUILD, USER, ROLE)
        .reason("promotion")
        .await
        .unwrap();

    let request = single_request(
        &transport,
        "PUT",
        "/guilds/100/members/53908099506183680/roles/41771983423143936",
    );
    assert_reason(&request, "promotion");
    assert!(request.body.is_empty());
}

#[tokio::test]
async fn remove_guild_member_role() {
    let (client, transport) = setup_empty();

    client
        .remove_guild_member_role(GUILD, USER, ROLE)
        .reason("demotion")
        .await
        .unwrap();

    let request = single_request(
        &transport,
        "DELETE",
        "/guilds/100/members/53908099506183680/roles/41771983423143936",
    );
    assert_reason(&request, "demotion");
}

#[tokio::test]
async fn remove_guild_member() {
    let (client, transport) = setup_empty();

    client
        .remove_guild_member(GUILD, USER)
        .reason("spam bot")
        .await
        .unwrap();

    let request = single_request(
        &transport,
        "DELETE",
        "/guilds/100/members/53908099506183680",
    );
    assert_reason(&request, "spam bot");
}

fn ban_json() -> serde_json::Value {
    json!({"reason": "mentioning b1nzy", "user": user_json("53908099506183680")})
}

#[tokio::test]
async fn get_guild_bans() {
    let (client, transport) = setup_json(&json!([ban_json()]));

    let bans = client
        .guild_bans(GUILD)
        .limit(50)
        .after(UserId::new_const(10))
        .await
        .unwrap();

    single_request(&transport, "GET", "/guilds/100/bans?limit=50&after=10");
    assert_eq!(bans[0].reason.as_deref(), Some("mentioning b1nzy"));
}

#[tokio::test]
async fn get_guild_ban() {
    let (client, transport) = setup_json(&ban_json());

    let ban = client.guild_ban(GUILD, USER).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/bans/53908099506183680");
    assert_eq!(ban.user.id, USER);
}

#[tokio::test]
async fn ban_member() {
    let (client, transport) = setup_empty();

    client
        .ban_member(GUILD, USER)
        .delete_message_seconds(3600)
        .reason("raid")
        .await
        .unwrap();

    let request = single_request(&transport, "PUT", "/guilds/100/bans/53908099506183680");
    assert_reason(&request, "raid");
    assert_eq!(request.json(), json!({"delete_message_seconds": 3600}));
}

#[tokio::test]
async fn ban_member_refuses_more_than_seven_days() {
    let (client, transport) = setup_empty();

    let result = client
        .ban_member(GUILD, USER)
        .delete_message_seconds(604_801)
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn unban_member() {
    let (client, transport) = setup_empty();

    client
        .unban_member(GUILD, USER)
        .reason("appeal")
        .await
        .unwrap();

    let request = single_request(&transport, "DELETE", "/guilds/100/bans/53908099506183680");
    assert_reason(&request, "appeal");
}

#[tokio::test]
async fn bulk_ban_members() {
    let (client, transport) = setup_json(&json!({"banned_users": ["1"], "failed_users": ["2"]}));

    let result = client
        .bulk_ban_members(GUILD, vec![UserId::new_const(1), UserId::new_const(2)])
        .delete_message_seconds(0)
        .reason("raid")
        .await
        .unwrap();

    let request = single_request(&transport, "POST", "/guilds/100/bulk-ban");
    assert_reason(&request, "raid");
    assert_eq!(
        request.json(),
        json!({"user_ids": ["1", "2"], "delete_message_seconds": 0})
    );
    assert_eq!(result.banned_users, vec![UserId::new_const(1)]);
    assert_eq!(result.failed_users, vec![UserId::new_const(2)]);
}

#[tokio::test]
async fn bulk_ban_members_refuses_more_than_200() {
    let (client, transport) = setup_empty();
    let user_ids = (1..=201).map(UserId::new_const).collect();

    let result = client.bulk_ban_members(GUILD, user_ids).await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn get_guild_roles() {
    let (client, transport) = setup_json(&json!([role_json("41771983423143936", 1)]));

    let roles = client.guild_roles(GUILD).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/roles");
    assert_eq!(roles[0].id, ROLE);
    assert_eq!(roles[0].colors.primary_color, Color(3_447_003));
}

#[tokio::test]
async fn get_guild_role() {
    let (client, transport) = setup_json(&role_json("41771983423143936", 1));

    let role = client.guild_role(GUILD, ROLE).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/roles/41771983423143936");
    assert!(role.hoist);
}

#[tokio::test]
async fn get_guild_role_permissions() {
    let (client, transport) = setup_json(&json!({"permissions": "8"}));

    let computed = client.guild_role_permissions(GUILD, ROLE).await.unwrap();

    single_request(
        &transport,
        "GET",
        "/guilds/100/permissions/role/41771983423143936",
    );
    assert_eq!(computed.permissions, Permissions::ADMINISTRATOR);
}

#[tokio::test]
async fn get_guild_role_member_counts() {
    let (client, transport) = setup_json(&json!({
        "613425648685547541": 1337,
        "1409696176629878905": 2,
        "697138785317814292": 67
    }));

    let counts = client.guild_role_member_counts(GUILD).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/roles/member-counts");
    assert_eq!(counts.len(), 3);
    assert_eq!(
        counts.get(&RoleId::new_const(613_425_648_685_547_541)),
        Some(&1337)
    );
}

#[tokio::test]
async fn create_guild_role() {
    let (client, transport) = setup_json(&role_json("5", 1));

    client
        .create_guild_role(GUILD)
        .name("Mods")
        .permissions(Permissions::KICK_MEMBERS | Permissions::BAN_MEMBERS)
        .colors(RoleColors::gradient(Color(0xFF_0000), Color(0x00_00FF)))
        .hoist(true)
        .unicode_emoji("🛡️")
        .mentionable(true)
        .reason("new role")
        .await
        .unwrap();

    let request = single_request(&transport, "POST", "/guilds/100/roles");
    assert_reason(&request, "new role");
    assert_eq!(
        request.json(),
        json!({
            "name": "Mods", "permissions": "6",
            "colors": {"primary_color": 16_711_680, "secondary_color": 255, "tertiary_color": null},
            "hoist": true, "unicode_emoji": "🛡️", "mentionable": true
        })
    );
}

#[tokio::test]
async fn create_guild_role_refuses_long_name() {
    let (client, transport) = setup_json(&role_json("5", 1));

    let result = client.create_guild_role(GUILD).name("a".repeat(101)).await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn edit_guild_role_positions() {
    let (client, transport) = setup_json(&json!([role_json("1", 2), role_json("2", 1)]));

    let roles = client
        .edit_guild_role_positions(
            GUILD,
            vec![
                RolePosition::new(RoleId::new_const(1), 2),
                RolePosition::new(RoleId::new_const(2), 1),
            ],
        )
        .reason("reorder")
        .await
        .unwrap();

    let request = single_request(&transport, "PATCH", "/guilds/100/roles");
    assert_reason(&request, "reorder");
    assert_eq!(
        request.json(),
        json!([{"id": "1", "position": 2}, {"id": "2", "position": 1}])
    );
    assert_eq!(roles.len(), 2);
}

#[tokio::test]
async fn edit_guild_role() {
    let (client, transport) = setup_json(&role_json("41771983423143936", 1));

    client
        .edit_guild_role(GUILD, ROLE)
        .name("Admins")
        .colors(RoleColors::HOLOGRAPHIC)
        .icon(ImageData::from_data_uri("data:image/png;base64,AAAA"))
        .clear_unicode_emoji()
        .reason("rename")
        .await
        .unwrap();

    let request = single_request(&transport, "PATCH", "/guilds/100/roles/41771983423143936");
    assert_reason(&request, "rename");
    assert_eq!(
        request.json(),
        json!({
            "name": "Admins",
            "colors": {"primary_color": 11_127_295, "secondary_color": 16_759_788, "tertiary_color": 16_761_760},
            "icon": "data:image/png;base64,AAAA", "unicode_emoji": null
        })
    );
}

#[tokio::test]
async fn delete_guild_role() {
    let (client, transport) = setup_empty();

    client
        .delete_guild_role(GUILD, ROLE)
        .reason("unused")
        .await
        .unwrap();

    let request = single_request(&transport, "DELETE", "/guilds/100/roles/41771983423143936");
    assert_reason(&request, "unused");
}
