//! Tests of the channel, thread, invite, poll, and stage instance endpoints.

use cordial_http::endpoints::thread::{
    SearchOrder, ThreadSearchResponse, ThreadSearchSort, ThreadSearchTagMode,
};
use cordial_http::request::{FileUpload, ImageData};
use cordial_http::testing::{FakeResponse, FakeTransport, RecordedRequest};
use cordial_http::{Client, Error, Token};
use cordial_model::Timestamp;
use cordial_model::channel::{
    AutoArchiveDuration, ChannelFlags, ChannelType, DefaultReaction, ForumTag, SortOrder,
};
use cordial_model::id::{
    ApplicationId, ChannelId, ForumTagId, MessageId, RoleId, ScheduledEventId, UserId,
};
use cordial_model::invite::{InviteTargetType, InviteTargetUsersJobStatus};
use cordial_model::permissions::{PermissionOverwrite, Permissions};
use cordial_model::voice::StagePrivacyLevel;
use serde_json::{Value, json};

const BASE: &str = "https://discord.com/api/v10";
const CHANNEL: ChannelId = ChannelId::new_const(41_771_983_423_143_937);
const MESSAGE: MessageId = MessageId::new_const(155_117_677_105_512_449);
const USER: UserId = UserId::new_const(82_198_898_841_029_460);

fn client(transport: &FakeTransport) -> Client {
    Client::builder()
        .token(Token::bot("abc"))
        .transport(transport.clone())
        .build()
}

/// Gives a client with one scripted response.
fn client_with(response: FakeResponse) -> (Client, FakeTransport) {
    let transport = FakeTransport::new();
    transport.push(response);
    (client(&transport), transport)
}

fn only_request(transport: &FakeTransport) -> RecordedRequest {
    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    requests.into_iter().next().unwrap()
}

fn assert_request(request: &RecordedRequest, method: &str, path: &str) {
    assert_eq!(request.method, method);
    assert_eq!(request.url, format!("{BASE}{path}"));
}

fn user_json() -> Value {
    json!({
        "username": "test", "discriminator": "9999",
        "id": "82198898841029460", "avatar": "33ecab261d4681afa4d85a04691c4a01"
    })
}

fn text_channel_json() -> Value {
    json!({
        "id": "41771983423143937", "guild_id": "41771983423143937", "name": "general",
        "type": 0, "position": 6, "permission_overwrites": [], "rate_limit_per_user": 2,
        "nsfw": true, "topic": "24/7 chat about how to gank Mike #2",
        "last_message_id": "155117677105512449", "parent_id": "399942396007890945",
        "default_auto_archive_duration": 60
    })
}

fn thread_json() -> Value {
    json!({
        "id": "41771983423143937", "guild_id": "41771983423143937",
        "parent_id": "41771983423143937", "owner_id": "41771983423143937",
        "name": "don't buy dota-2", "type": 11, "last_message_id": "155117677105512449",
        "message_count": 1, "member_count": 5, "rate_limit_per_user": 2,
        "thread_metadata": {
            "archived": false, "auto_archive_duration": 1440,
            "archive_timestamp": "2021-04-12T23:40:39.855793+00:00", "locked": false
        },
        "total_message_sent": 1
    })
}

fn group_dm_json() -> Value {
    json!({
        "name": "Some test channel", "icon": null, "recipients": [user_json()],
        "last_message_id": "3343820033257021450", "type": 3,
        "id": "319674150115710528", "owner_id": "82198810841029460"
    })
}

fn message_json() -> Value {
    json!({
        "id": "155117677105512449", "channel_id": "41771983423143937", "content": "Hello",
        "timestamp": "2026-01-01T00:00:00+00:00", "edited_timestamp": null,
        "tts": false, "mention_everyone": false, "mentions": [], "mention_roles": [],
        "attachments": [], "embeds": [], "pinned": false, "type": 0,
        "author": user_json()
    })
}

fn thread_member_json() -> Value {
    json!({
        "id": "41771983423143937", "user_id": "82198898841029460",
        "join_timestamp": "2021-04-12T23:40:39.855793+00:00", "flags": 0
    })
}

fn invite_json() -> Value {
    json!({
        "type": 0, "code": "0vCdhLbwjZZTWZLD",
        "guild": {
            "id": "165176875973476352", "name": "CS:GO Fraggers Only", "splash": null,
            "banner": null, "description": "Very good description", "icon": null,
            "features": ["NEWS", "DISCOVERABLE"], "verification_level": 2,
            "vanity_url_code": null, "nsfw_level": 0, "premium_subscription_count": 5
        },
        "channel": {"id": "165176875973476352", "name": "illuminati", "type": 0},
        "inviter": {
            "id": "115590097100865541", "username": "speed", "avatar": "deadbeef",
            "discriminator": "7653", "public_flags": 131_328
        },
        "target_type": 1,
        "target_user": {
            "id": "165176875973476352", "username": "bob", "avatar": "deadbeef",
            "discriminator": "1234", "public_flags": 64
        },
        "expires_at": null
    })
}

fn invite_with_metadata_json() -> Value {
    let mut invite = invite_json();
    let fields = json!({
        "uses": 0, "max_uses": 0, "max_age": 0, "temporary": false,
        "created_at": "2016-03-31T19:15:39.954000+00:00"
    });
    for (key, value) in fields.as_object().unwrap() {
        invite[key] = value.clone();
    }
    invite
}

fn stage_json() -> Value {
    json!({
        "id": "840647391636226060", "guild_id": "197038439483310086",
        "channel_id": "733488538393510049", "topic": "Testing Testing, 123",
        "privacy_level": 1, "discoverable_disabled": false,
        "guild_scheduled_event_id": "947656305244532806"
    })
}

fn multipart_text(request: &RecordedRequest) -> String {
    assert!(
        request
            .header("content-type")
            .unwrap()
            .starts_with("multipart/form-data; boundary=")
    );
    String::from_utf8(request.body.to_vec()).unwrap()
}

// Channels.

#[tokio::test]
async fn gets_a_channel() {
    let (client, transport) = client_with(FakeResponse::json(200, &text_channel_json()));

    let channel = client.channel(CHANNEL).await.unwrap();

    assert_eq!(channel.name.as_deref(), Some("general"));
    assert_eq!(channel.kind, ChannelType::GuildText);
    let request = only_request(&transport);
    assert_request(&request, "GET", "/channels/41771983423143937");
    assert!(request.body.is_empty());
}

#[tokio::test]
async fn edits_a_guild_channel() {
    let (client, transport) = client_with(FakeResponse::json(200, &text_channel_json()));
    let reaction: DefaultReaction =
        serde_json::from_value(json!({"emoji_id": null, "emoji_name": "👍"})).unwrap();

    let channel = client
        .edit_channel(CHANNEL)
        .name("general")
        .clear_topic()
        .nsfw(true)
        .rate_limit_per_user(2)
        .clear_parent()
        .permission_overwrites(vec![PermissionOverwrite::role(
            RoleId::new_const(5),
            Permissions::VIEW_CHANNEL,
            Permissions::SEND_MESSAGES,
        )])
        .available_tags(vec![ForumTag::new("news")])
        .default_reaction_emoji(reaction)
        .default_sort_order(SortOrder::CreationDate)
        .reason("cleanup")
        .await
        .unwrap();

    assert_eq!(channel.rate_limit_per_user, Some(2));
    let request = only_request(&transport);
    assert_request(&request, "PATCH", "/channels/41771983423143937");
    assert_eq!(request.header("x-audit-log-reason"), Some("cleanup"));
    assert_eq!(
        request.json(),
        json!({
            "name": "general", "topic": null, "nsfw": true, "rate_limit_per_user": 2,
            "parent_id": null,
            "permission_overwrites": [{"id": "5", "type": 0, "allow": "1024", "deny": "2048"}],
            "available_tags": [{"name": "news", "moderated": false, "emoji_id": null, "emoji_name": null}],
            "default_reaction_emoji": {"emoji_id": null, "emoji_name": "👍"},
            "default_sort_order": 1
        })
    );
}

#[tokio::test]
async fn edits_a_thread_and_a_group_dm() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &thread_json()));
    transport.push(FakeResponse::json(200, &group_dm_json()));
    let client = client(&transport);

    let thread = client
        .edit_channel(CHANNEL)
        .archived(false)
        .locked(true)
        .invitable(false)
        .auto_archive_duration(AutoArchiveDuration::Week)
        .applied_tags(vec![ForumTagId::new_const(7)])
        .flags(ChannelFlags::PINNED)
        .await
        .unwrap();
    let group = client
        .edit_channel(ChannelId::new_const(319_674_150_115_710_528))
        .name("Some test channel")
        .icon(ImageData::from_data_uri("data:image/png;base64,AAAA"))
        .await
        .unwrap();

    assert!(thread.is_thread());
    assert_eq!(group.kind, ChannelType::GroupDm);
    let requests = transport.requests();
    assert_eq!(
        requests[0].json(),
        json!({
            "archived": false, "locked": true, "invitable": false,
            "auto_archive_duration": 10080, "applied_tags": ["7"], "flags": 2
        })
    );
    assert_eq!(
        requests[1].json(),
        json!({"name": "Some test channel", "icon": "data:image/png;base64,AAAA"})
    );
}

#[tokio::test]
async fn refuses_a_long_channel_name() {
    let transport = FakeTransport::new();

    let result = client(&transport)
        .edit_channel(CHANNEL)
        .name("a".repeat(101))
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn deletes_a_channel() {
    let (client, transport) = client_with(FakeResponse::json(200, &text_channel_json()));

    let channel = client.delete_channel(CHANNEL).reason("old").await.unwrap();

    assert_eq!(channel.id, CHANNEL);
    let request = only_request(&transport);
    assert_request(&request, "DELETE", "/channels/41771983423143937");
    assert_eq!(request.header("x-audit-log-reason"), Some("old"));
}

#[tokio::test]
async fn edits_channel_permissions() {
    let (client, transport) = client_with(FakeResponse::empty(204));
    let overwrite = PermissionOverwrite::member(
        USER,
        Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
        Permissions::empty(),
    );

    client
        .edit_channel_permissions(CHANNEL, &overwrite)
        .reason("access")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_request(
        &request,
        "PUT",
        "/channels/41771983423143937/permissions/82198898841029460",
    );
    assert_eq!(request.header("x-audit-log-reason"), Some("access"));
    assert_eq!(
        request.json(),
        json!({"allow": "3072", "deny": "0", "type": 1})
    );
}

#[tokio::test]
async fn deletes_a_channel_permission() {
    let (client, transport) = client_with(FakeResponse::empty(204));

    client
        .delete_channel_permission(CHANNEL, RoleId::new_const(5).cast())
        .reason("reset")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_request(
        &request,
        "DELETE",
        "/channels/41771983423143937/permissions/5",
    );
    assert_eq!(request.header("x-audit-log-reason"), Some("reset"));
}

#[tokio::test]
async fn gets_computed_permissions() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &json!({"permissions": "3072"})));
    transport.push(FakeResponse::json(200, &json!({"permissions": "1024"})));
    let client = client(&transport);

    let member = client
        .channel_member_permissions(CHANNEL, USER)
        .implicit(false)
        .await
        .unwrap();
    let role = client
        .channel_role_permissions(CHANNEL, RoleId::new_const(5))
        .await
        .unwrap();

    assert_eq!(
        member.permissions,
        Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES
    );
    assert_eq!(role.permissions, Permissions::VIEW_CHANNEL);
    let requests = transport.requests();
    assert_request(
        &requests[0],
        "GET",
        "/channels/41771983423143937/permissions/member/82198898841029460?implicit=false",
    );
    assert_request(
        &requests[1],
        "GET",
        "/channels/41771983423143937/permissions/role/5",
    );
}

#[tokio::test]
async fn sets_and_removes_the_voice_status() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));
    transport.push(FakeResponse::empty(204));
    let client = client(&transport);

    client
        .set_voice_channel_status(CHANNEL)
        .status("Playing chess")
        .reason("event")
        .await
        .unwrap();
    client.set_voice_channel_status(CHANNEL).await.unwrap();

    let requests = transport.requests();
    assert_request(
        &requests[0],
        "PUT",
        "/channels/41771983423143937/voice-status",
    );
    assert_eq!(requests[0].header("x-audit-log-reason"), Some("event"));
    assert_eq!(requests[0].json(), json!({"status": "Playing chess"}));
    assert_eq!(requests[1].json(), json!({"status": null}));
}

#[tokio::test]
async fn follows_an_announcement_channel() {
    let (client, transport) = client_with(FakeResponse::json(
        200,
        &json!({"channel_id": "41771983423143937", "webhook_id": "99"}),
    ));

    let followed = client
        .follow_announcement_channel(CHANNEL, ChannelId::new_const(10))
        .reason("news")
        .await
        .unwrap();

    assert_eq!(followed.channel_id, CHANNEL);
    assert_eq!(followed.webhook_id.get(), 99);
    let request = only_request(&transport);
    assert_request(&request, "POST", "/channels/41771983423143937/followers");
    assert_eq!(request.header("x-audit-log-reason"), Some("news"));
    assert_eq!(request.json(), json!({"webhook_channel_id": "10"}));
}

#[tokio::test]
async fn triggers_typing_with_both_answers() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));
    transport.push(FakeResponse::json(200, &json!({})));
    let client = client(&transport);

    client.trigger_typing(CHANNEL).await.unwrap();
    client.trigger_typing(CHANNEL).await.unwrap();

    let requests = transport.requests();
    assert_request(&requests[0], "POST", "/channels/41771983423143937/typing");
    assert!(requests[0].body.is_empty());
}

#[tokio::test]
async fn adds_and_removes_group_dm_recipients() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(201, &group_dm_json()));
    transport.push(FakeResponse::empty(204));
    transport.push(FakeResponse::empty(204));
    let client = client(&transport);
    let group = ChannelId::new_const(319_674_150_115_710_528);

    let added = client
        .add_group_dm_recipient(group, USER, "secret")
        .nick("Tester")
        .await
        .unwrap();
    let again = client
        .add_group_dm_recipient(group, USER, "secret")
        .await
        .unwrap();
    client.remove_group_dm_recipient(group, USER).await.unwrap();

    assert_eq!(added.unwrap().kind, ChannelType::GroupDm);
    assert!(again.is_none());
    let requests = transport.requests();
    assert_request(
        &requests[0],
        "PUT",
        "/channels/319674150115710528/recipients/82198898841029460",
    );
    assert_eq!(
        requests[0].json(),
        json!({"access_token": "secret", "nick": "Tester"})
    );
    assert_request(
        &requests[2],
        "DELETE",
        "/channels/319674150115710528/recipients/82198898841029460",
    );
}

#[test]
fn group_dm_debug_hides_the_access_token() {
    let client = Client::new(Token::bot("abc"));

    let request = client.add_group_dm_recipient(CHANNEL, USER, "secret-token");

    let text = format!("{request:?}");
    assert!(!text.contains("secret-token"));
}

// Threads.

#[tokio::test]
async fn starts_a_thread_from_a_message() {
    let (client, transport) = client_with(FakeResponse::json(200, &thread_json()));

    let thread = client
        .create_thread_from_message(CHANNEL, MESSAGE, "don't buy dota-2")
        .auto_archive_duration(AutoArchiveDuration::Day)
        .rate_limit_per_user(2)
        .reason("talk")
        .await
        .unwrap();

    assert_eq!(thread.kind, ChannelType::PublicThread);
    let request = only_request(&transport);
    assert_request(
        &request,
        "POST",
        "/channels/41771983423143937/messages/155117677105512449/threads",
    );
    assert_eq!(request.header("x-audit-log-reason"), Some("talk"));
    assert_eq!(
        request.json(),
        json!({"name": "don't buy dota-2", "auto_archive_duration": 1440, "rate_limit_per_user": 2})
    );
}

#[tokio::test]
async fn starts_a_thread_without_a_message() {
    let (client, transport) = client_with(FakeResponse::json(200, &thread_json()));

    client
        .create_thread(CHANNEL, "private talk")
        .kind(ChannelType::PrivateThread)
        .invitable(false)
        .reason("talk")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_request(&request, "POST", "/channels/41771983423143937/threads");
    assert_eq!(request.header("x-audit-log-reason"), Some("talk"));
    assert_eq!(
        request.json(),
        json!({"name": "private talk", "type": 12, "invitable": false})
    );
}

#[tokio::test]
async fn refuses_a_thread_with_a_large_slowmode() {
    let transport = FakeTransport::new();

    let result = client(&transport)
        .create_thread(CHANNEL, "slow")
        .rate_limit_per_user(21_601)
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn starts_a_forum_thread() {
    let mut response = thread_json();
    response["message"] = message_json();
    let (client, transport) = client_with(FakeResponse::json(201, &response));

    let thread = client
        .create_forum_thread(CHANNEL, "Help")
        .content("Hello")
        .applied_tags(vec![ForumTagId::new_const(7)])
        .reason("post")
        .await
        .unwrap();

    assert_eq!(thread.channel.name.as_deref(), Some("don't buy dota-2"));
    assert_eq!(thread.message.unwrap().content, "Hello");
    let request = only_request(&transport);
    assert_request(&request, "POST", "/channels/41771983423143937/threads");
    assert_eq!(request.header("x-audit-log-reason"), Some("post"));
    assert_eq!(
        request.json(),
        json!({"name": "Help", "applied_tags": ["7"], "message": {"content": "Hello"}})
    );
}

#[tokio::test]
async fn starts_a_forum_thread_with_a_file() {
    let (client, transport) = client_with(FakeResponse::json(201, &thread_json()));

    client
        .create_forum_thread(CHANNEL, "Cats")
        .file(FileUpload::new("cat.png", &b"PNG"[..]).content_type("image/png"))
        .await
        .unwrap();

    let body = multipart_text(&only_request(&transport));
    assert!(
        body.contains(
            r#"{"name":"Cats","message":{"attachments":[{"id":0,"filename":"cat.png"}]}}"#
        )
    );
    assert!(body.contains("name=\"files[0]\"; filename=\"cat.png\""));
}

#[tokio::test]
async fn refuses_an_empty_forum_thread() {
    let transport = FakeTransport::new();

    let result = client(&transport)
        .create_forum_thread(CHANNEL, "Empty")
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn joins_and_leaves_threads() {
    let transport = FakeTransport::new();
    for _ in 0..4 {
        transport.push(FakeResponse::empty(204));
    }
    let client = client(&transport);

    client.join_thread(CHANNEL).await.unwrap();
    client.leave_thread(CHANNEL).await.unwrap();
    client.add_thread_member(CHANNEL, USER).await.unwrap();
    client.remove_thread_member(CHANNEL, USER).await.unwrap();

    let requests = transport.requests();
    assert_request(
        &requests[0],
        "PUT",
        "/channels/41771983423143937/thread-members/@me",
    );
    assert_request(
        &requests[1],
        "DELETE",
        "/channels/41771983423143937/thread-members/@me",
    );
    assert_request(
        &requests[2],
        "PUT",
        "/channels/41771983423143937/thread-members/82198898841029460",
    );
    assert_request(
        &requests[3],
        "DELETE",
        "/channels/41771983423143937/thread-members/82198898841029460",
    );
}

#[tokio::test]
async fn gets_a_thread_member_with_the_guild_member() {
    let mut member = thread_member_json();
    member["member"] = json!({
        "user": user_json(), "roles": [], "joined_at": "2015-04-26T06:26:56.936000+00:00",
        "deaf": false, "mute": false, "flags": 0
    });
    let (client, transport) = client_with(FakeResponse::json(200, &member));

    let member = client
        .thread_member(CHANNEL, USER)
        .with_member(true)
        .await
        .unwrap();

    assert_eq!(member.user_id, Some(USER));
    assert!(member.member.is_some());
    assert_request(
        &only_request(&transport),
        "GET",
        "/channels/41771983423143937/thread-members/82198898841029460?with_member=true",
    );
}

#[tokio::test]
async fn lists_thread_members_with_pagination() {
    let (client, transport) = client_with(FakeResponse::json(200, &json!([thread_member_json()])));

    let members = client
        .thread_members(CHANNEL)
        .with_member(true)
        .after(UserId::new_const(1))
        .limit(50)
        .await
        .unwrap();

    assert_eq!(members.len(), 1);
    assert_request(
        &only_request(&transport),
        "GET",
        "/channels/41771983423143937/thread-members?with_member=true&after=1&limit=50",
    );
}

#[tokio::test]
async fn lists_archived_threads() {
    let list =
        json!({"threads": [thread_json()], "members": [thread_member_json()], "has_more": true});
    let transport = FakeTransport::new();
    for _ in 0..3 {
        transport.push(FakeResponse::json(200, &list));
    }
    let client = client(&transport);
    let before = Timestamp::parse("2021-04-12T23:40:39.855793+00:00").unwrap();

    let public = client
        .public_archived_threads(CHANNEL)
        .before(before)
        .limit(10)
        .await
        .unwrap();
    client
        .private_archived_threads(CHANNEL)
        .limit(1)
        .await
        .unwrap();
    client
        .joined_private_archived_threads(CHANNEL)
        .before(ChannelId::new_const(3))
        .await
        .unwrap();

    assert_eq!(public.threads.len(), 1);
    assert_eq!(public.has_more, Some(true));
    let requests = transport.requests();
    assert_request(
        &requests[0],
        "GET",
        "/channels/41771983423143937/threads/archived/public?before=2021-04-12T23%3A40%3A39.855793%2B00%3A00&limit=10",
    );
    assert_request(
        &requests[1],
        "GET",
        "/channels/41771983423143937/threads/archived/private?limit=2",
    );
    assert_request(
        &requests[2],
        "GET",
        "/channels/41771983423143937/users/@me/threads/archived/private?before=3",
    );
}

#[tokio::test]
async fn searches_threads() {
    let results = json!({
        "threads": [thread_json()], "members": [], "has_more": false,
        "first_messages": [message_json()], "total_results": 1
    });
    let (client, transport) = client_with(FakeResponse::json(200, &results));

    let response = client
        .search_threads(CHANNEL)
        .name("dota")
        .tag(ForumTagId::new_const(7))
        .tag(ForumTagId::new_const(8))
        .tag_mode(ThreadSearchTagMode::MatchSome)
        .archived(true)
        .sort_by(ThreadSearchSort::LastMessageTime)
        .sort_order(SearchOrder::Descending)
        .limit(30)
        .offset(25)
        .await
        .unwrap();

    let ThreadSearchResponse::Results(results) = response else {
        panic!("expected results");
    };
    assert_eq!(results.total_results, 1);
    assert_eq!(results.first_messages.len(), 1);
    assert_request(
        &only_request(&transport),
        "GET",
        "/channels/41771983423143937/threads/search?name=dota&tag=7&tag=8&tag_setting=match_some&archived=true&sort_by=last_message_time&sort_order=desc&limit=25&offset=25",
    );
}

#[tokio::test]
async fn thread_search_reports_an_index_that_is_not_ready() {
    let (client, _transport) = client_with(FakeResponse::json(
        202,
        &json!({"message": "Index not yet available. Try again later", "code": 110_000, "documents_indexed": 0, "retry_after": 2}),
    ));

    let response = client.search_threads(CHANNEL).await.unwrap();

    let ThreadSearchResponse::IndexNotReady(not_ready) = response else {
        panic!("expected an index that is not ready");
    };
    assert_eq!(not_ready.retry_after, 2);
}

// Invites.

#[tokio::test]
async fn gets_channel_invites() {
    let (client, transport) = client_with(FakeResponse::json(
        200,
        &json!([invite_with_metadata_json()]),
    ));

    let invites = client.channel_invites(CHANNEL).await.unwrap();

    assert_eq!(invites[0].code, "0vCdhLbwjZZTWZLD");
    assert_eq!(invites[0].uses, Some(0));
    assert_request(
        &only_request(&transport),
        "GET",
        "/channels/41771983423143937/invites",
    );
}

#[tokio::test]
async fn creates_a_channel_invite() {
    let (client, transport) = client_with(FakeResponse::json(200, &invite_json()));

    let invite = client
        .create_channel_invite(CHANNEL)
        .max_age(3600)
        .max_uses(5)
        .unique(true)
        .target_stream(USER)
        .target_user_ids(vec![UserId::new_const(1), UserId::new_const(2)])
        .role_ids(vec![RoleId::new_const(5)])
        .reason("party")
        .await
        .unwrap();

    assert_eq!(invite.target_type, Some(InviteTargetType::Stream));
    let request = only_request(&transport);
    assert_request(&request, "POST", "/channels/41771983423143937/invites");
    assert_eq!(request.header("x-audit-log-reason"), Some("party"));
    assert_eq!(
        request.json(),
        json!({
            "max_age": 3600, "max_uses": 5, "unique": true, "target_type": 1,
            "target_user_id": "82198898841029460", "target_user_ids": ["1", "2"],
            "role_ids": ["5"]
        })
    );
}

#[tokio::test]
async fn creates_an_empty_channel_invite() {
    let (client, transport) = client_with(FakeResponse::json(200, &invite_json()));

    client
        .create_channel_invite(CHANNEL)
        .target_application(ApplicationId::new_const(9))
        .await
        .unwrap();
    let request = only_request(&transport);

    assert_eq!(
        request.json(),
        json!({"target_type": 2, "target_application_id": "9"})
    );
}

#[tokio::test]
async fn creates_a_channel_invite_with_a_target_users_file() {
    let (client, transport) = client_with(FakeResponse::json(200, &invite_json()));

    client
        .create_channel_invite(CHANNEL)
        .max_uses(1)
        .target_users_file(FileUpload::new("users.csv", &b"user_id\n1\n2\n"[..]))
        .await
        .unwrap();

    let body = multipart_text(&only_request(&transport));
    assert!(body.contains(
        "name=\"payload_json\"\r\nContent-Type: application/json\r\n\r\n{\"max_uses\":1}\r\n"
    ));
    assert!(body.contains(
        "name=\"target_users_file\"; filename=\"users.csv\"\r\nContent-Type: text/csv\r\n\r\nuser_id\n1\n2\n\r\n"
    ));
}

#[tokio::test]
async fn refuses_target_user_ids_with_a_file() {
    let transport = FakeTransport::new();

    let result = client(&transport)
        .create_channel_invite(CHANNEL)
        .target_user_ids(vec![USER])
        .target_users_file(FileUpload::new("users.csv", &b"1"[..]))
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn gets_an_invite() {
    let mut invite = invite_json();
    invite["approximate_member_count"] = json!(100);
    invite["approximate_presence_count"] = json!(10);
    let (client, transport) = client_with(FakeResponse::json(200, &invite));

    let invite = client
        .invite("0vCdhLbwjZZTWZLD")
        .with_counts(true)
        .scheduled_event(ScheduledEventId::new_const(4))
        .await
        .unwrap();

    assert_eq!(invite.approximate_member_count, Some(100));
    assert_eq!(invite.guild.unwrap().name, "CS:GO Fraggers Only");
    assert_request(
        &only_request(&transport),
        "GET",
        "/invites/0vCdhLbwjZZTWZLD?with_counts=true&guild_scheduled_event_id=4",
    );
}

#[tokio::test]
async fn deletes_an_invite() {
    let (client, transport) = client_with(FakeResponse::json(200, &invite_json()));

    let invite = client
        .delete_invite("0vCdhLbwjZZTWZLD")
        .reason("leak")
        .await
        .unwrap();

    assert_eq!(invite.code, "0vCdhLbwjZZTWZLD");
    let request = only_request(&transport);
    assert_request(&request, "DELETE", "/invites/0vCdhLbwjZZTWZLD");
    assert_eq!(request.header("x-audit-log-reason"), Some("leak"));
}

#[tokio::test]
async fn encodes_the_invite_code() {
    let (client, transport) = client_with(FakeResponse::json(200, &invite_json()));

    client.invite("a/b").await.unwrap();

    assert_request(&only_request(&transport), "GET", "/invites/a%2Fb");
}

#[tokio::test]
async fn gets_invite_target_users_as_csv() {
    let mut response = FakeResponse::empty(200).header("content-type", "text/csv");
    response.body = "user_id\n1\n2\n".into();
    let (client, transport) = client_with(response);

    let csv = client.invite_target_users("abc").await.unwrap();

    assert_eq!(csv, "user_id\n1\n2\n");
    assert_request(
        &only_request(&transport),
        "GET",
        "/invites/abc/target-users",
    );
}

#[tokio::test]
async fn updates_invite_target_users() {
    let (client, transport) = client_with(FakeResponse::empty(204));

    client
        .update_invite_target_users(
            "abc",
            FileUpload::new("users.csv", &b"1\n2\n"[..]).content_type("text/plain"),
        )
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_request(&request, "PUT", "/invites/abc/target-users");
    let body = multipart_text(&request);
    assert!(body.contains(
        "name=\"target_users_file\"; filename=\"users.csv\"\r\nContent-Type: text/plain\r\n\r\n1\n2\n\r\n"
    ));
    assert!(!body.contains("payload_json"));
}

#[tokio::test]
async fn adds_and_removes_invite_target_users() {
    let transport = FakeTransport::new();
    for _ in 0..4 {
        transport.push(FakeResponse::empty(204));
    }
    let client = client(&transport);

    client.add_invite_target_user("abc", USER).await.unwrap();
    client.remove_invite_target_user("abc", USER).await.unwrap();
    client
        .bulk_add_invite_target_users("abc", vec![UserId::new_const(1), UserId::new_const(2)])
        .await
        .unwrap();
    client
        .bulk_delete_invite_target_users("abc", vec![UserId::new_const(1)])
        .await
        .unwrap();

    let requests = transport.requests();
    assert_request(
        &requests[0],
        "PUT",
        "/invites/abc/target-users/82198898841029460",
    );
    assert_request(
        &requests[1],
        "DELETE",
        "/invites/abc/target-users/82198898841029460",
    );
    assert_request(&requests[2], "POST", "/invites/abc/target-users/bulk-add");
    assert_eq!(requests[2].json(), json!({"user_ids": ["1", "2"]}));
    assert_request(
        &requests[3],
        "POST",
        "/invites/abc/target-users/bulk-delete",
    );
    assert_eq!(requests[3].json(), json!({"user_ids": ["1"]}));
}

#[tokio::test]
async fn refuses_an_empty_bulk_add() {
    let transport = FakeTransport::new();

    let result = client(&transport)
        .bulk_add_invite_target_users("abc", Vec::new())
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
}

#[tokio::test]
async fn gets_the_target_users_job_status() {
    let (client, transport) = client_with(FakeResponse::json(
        200,
        &json!({
            "status": 3, "total_users": 100, "processed_users": 41,
            "created_at": "2025-01-08T12:00:00.000000+00:00", "completed_at": null,
            "error_message": "Failed to parse CSV file"
        }),
    ));

    let job = client.invite_target_users_job_status("abc").await.unwrap();

    assert_eq!(job.status, InviteTargetUsersJobStatus::Failed);
    assert_eq!(job.processed_users, Some(41));
    assert_request(
        &only_request(&transport),
        "GET",
        "/invites/abc/target-users/job-status",
    );
}

// Polls.

#[tokio::test]
async fn gets_answer_voters() {
    let (client, transport) =
        client_with(FakeResponse::json(200, &json!({"users": [user_json()]})));

    let voters = client
        .answer_voters(CHANNEL, MESSAGE, 1)
        .after(UserId::new_const(5))
        .limit(10)
        .await
        .unwrap();

    assert_eq!(voters.users[0].id, USER);
    assert_request(
        &only_request(&transport),
        "GET",
        "/channels/41771983423143937/polls/155117677105512449/answers/1?after=5&limit=10",
    );
}

#[tokio::test]
async fn ends_a_poll() {
    let (client, transport) = client_with(FakeResponse::json(200, &message_json()));

    let message = client.end_poll(CHANNEL, MESSAGE).await.unwrap();

    assert_eq!(message.id, MESSAGE);
    let request = only_request(&transport);
    assert_request(
        &request,
        "POST",
        "/channels/41771983423143937/polls/155117677105512449/expire",
    );
    assert!(request.body.is_empty());
}

// Stage instances.

#[tokio::test]
async fn creates_a_stage_instance() {
    let (client, transport) = client_with(FakeResponse::json(200, &stage_json()));
    let stage_channel = ChannelId::new_const(733_488_538_393_510_049);

    let stage = client
        .create_stage_instance(stage_channel, "Testing Testing, 123")
        .privacy_level(StagePrivacyLevel::GuildOnly)
        .send_start_notification(true)
        .scheduled_event(ScheduledEventId::new_const(947_656_305_244_532_806))
        .reason("show")
        .await
        .unwrap();

    assert_eq!(stage.topic, "Testing Testing, 123");
    assert_eq!(stage.privacy_level, StagePrivacyLevel::Public);
    let request = only_request(&transport);
    assert_request(&request, "POST", "/stage-instances");
    assert_eq!(request.header("x-audit-log-reason"), Some("show"));
    assert_eq!(
        request.json(),
        json!({
            "channel_id": "733488538393510049", "topic": "Testing Testing, 123",
            "privacy_level": 2, "send_start_notification": true,
            "guild_scheduled_event_id": "947656305244532806"
        })
    );
}

#[tokio::test]
async fn refuses_an_empty_stage_topic() {
    let transport = FakeTransport::new();

    let result = client(&transport).create_stage_instance(CHANNEL, "").await;

    assert!(matches!(result, Err(Error::Validation(_))));
}

#[tokio::test]
async fn gets_edits_and_deletes_a_stage_instance() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &stage_json()));
    transport.push(FakeResponse::json(200, &stage_json()));
    transport.push(FakeResponse::empty(204));
    let client = client(&transport);
    let stage_channel = ChannelId::new_const(733_488_538_393_510_049);

    let stage = client.stage_instance(stage_channel).await.unwrap();
    client
        .edit_stage_instance(stage_channel)
        .topic("New topic")
        .reason("rename")
        .await
        .unwrap();
    client
        .delete_stage_instance(stage_channel)
        .reason("done")
        .await
        .unwrap();

    assert_eq!(stage.channel_id, stage_channel);
    let requests = transport.requests();
    assert_request(&requests[0], "GET", "/stage-instances/733488538393510049");
    assert_request(&requests[1], "PATCH", "/stage-instances/733488538393510049");
    assert_eq!(requests[1].header("x-audit-log-reason"), Some("rename"));
    assert_eq!(requests[1].json(), json!({"topic": "New topic"}));
    assert_request(
        &requests[2],
        "DELETE",
        "/stage-instances/733488538393510049",
    );
    assert_eq!(requests[2].header("x-audit-log-reason"), Some("done"));
}
