//! Tests of the message, reaction, pin, and search endpoints with a fake transport.

use cordial_http::endpoints::message::{
    MessageSearch, SearchAuthorType, SearchEmbedType, SearchHas, SearchSortBy, SearchSortOrder,
};
use cordial_http::testing::{FakeResponse, FakeTransport, RecordedRequest};
use cordial_http::{Client, Error, Token};
use cordial_model::Timestamp;
use cordial_model::emoji::ReactionType;
use cordial_model::id::{ChannelId, EmojiId, GuildId, MessageId, RoleId, UserId};
use cordial_model::message::ReactionKind;
use serde_json::json;

const API: &str = "https://discord.com/api/v10";
const CHANNEL: ChannelId = ChannelId::new_const(1234);
const MESSAGE: MessageId = MessageId::new_const(5678);
const GUILD: GuildId = GuildId::new_const(42);

fn client(transport: &FakeTransport) -> Client {
    Client::builder()
        .token(Token::bot("abc"))
        .transport(transport.clone())
        .build()
}

fn message_json() -> serde_json::Value {
    json!({
        "id": "5678", "channel_id": "1234", "content": "Hello",
        "timestamp": "2026-01-01T00:00:00+00:00", "edited_timestamp": null,
        "tts": false, "mention_everyone": false, "mentions": [], "mention_roles": [],
        "attachments": [], "embeds": [], "pinned": false, "type": 0,
        "author": {"id": "9", "username": "bot", "discriminator": "0", "global_name": null, "avatar": null}
    })
}

fn user_json(id: &str) -> serde_json::Value {
    json!({
        "id": id, "username": "test", "discriminator": "7479",
        "avatar": "b004ec1740a63ca06ae2e14c5cee11f3", "public_flags": 131328
    })
}

/// Gives the one request that the transport received.
fn only_request(transport: &FakeTransport) -> RecordedRequest {
    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    requests.into_iter().next().unwrap()
}

fn assert_bot_auth(request: &RecordedRequest) {
    assert_eq!(request.header("authorization"), Some("Bot abc"));
}

#[tokio::test]
async fn crossposts_a_message() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &message_json()));

    let message = client(&transport)
        .crosspost_message(CHANNEL, MESSAGE)
        .await
        .unwrap();

    assert_eq!(message.id, MESSAGE);
    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        format!("{API}/channels/1234/messages/5678/crosspost")
    );
    assert_bot_auth(&request);
    assert!(request.body.is_empty());
}

#[tokio::test]
async fn creates_a_unicode_reaction() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .create_reaction(CHANNEL, MESSAGE, "👍")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PUT");
    assert_eq!(
        request.url,
        format!("{API}/channels/1234/messages/5678/reactions/%F0%9F%91%8D/@me")
    );
    assert_bot_auth(&request);
}

#[tokio::test]
async fn deletes_own_custom_reaction() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_own_reaction(
            CHANNEL,
            MESSAGE,
            ReactionType::custom(EmojiId::new_const(41771983429993937), "LUL"),
        )
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(
        request.url,
        format!("{API}/channels/1234/messages/5678/reactions/LUL%3A41771983429993937/@me")
    );
    assert_bot_auth(&request);
}

#[tokio::test]
async fn deletes_user_reaction() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_user_reaction(CHANNEL, MESSAGE, "🔥", UserId::new_const(77))
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(
        request.url,
        format!("{API}/channels/1234/messages/5678/reactions/%F0%9F%94%A5/77")
    );
    assert_bot_auth(&request);
}

#[tokio::test]
async fn gets_reactions_with_query() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(
        200,
        &json!([user_json("190320984123768832")]),
    ));

    let users = client(&transport)
        .reactions(CHANNEL, MESSAGE, "👍")
        .kind(ReactionKind::Burst)
        .after(UserId::new_const(10))
        .limit(200)
        .await
        .unwrap();

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].username, "test");
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!(
            "{API}/channels/1234/messages/5678/reactions/%F0%9F%91%8D?type=1&after=10&limit=100"
        )
    );
    assert_bot_auth(&request);
}

#[tokio::test]
async fn deletes_all_reactions() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_all_reactions(CHANNEL, MESSAGE)
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(
        request.url,
        format!("{API}/channels/1234/messages/5678/reactions")
    );
    assert_bot_auth(&request);
}

#[tokio::test]
async fn deletes_all_reactions_for_emoji() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_all_reactions_for_emoji(CHANNEL, MESSAGE, "👍")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(
        request.url,
        format!("{API}/channels/1234/messages/5678/reactions/%F0%9F%91%8D")
    );
    assert_bot_auth(&request);
}

#[tokio::test]
async fn bulk_deletes_messages() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .bulk_delete_messages(
            CHANNEL,
            vec![MessageId::new_const(1), MessageId::new_const(2)],
        )
        .reason("spam cleanup")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        format!("{API}/channels/1234/messages/bulk-delete")
    );
    assert_bot_auth(&request);
    assert_eq!(request.header("x-audit-log-reason"), Some("spam%20cleanup"));
    assert_eq!(request.json(), json!({"messages": ["1", "2"]}));
}

#[tokio::test]
async fn refuses_bulk_delete_with_bad_ids() {
    let transport = FakeTransport::new();
    let client = client(&transport);

    let one = client
        .bulk_delete_messages(CHANNEL, vec![MessageId::new_const(1)])
        .await;
    let duplicate = client
        .bulk_delete_messages(
            CHANNEL,
            vec![MessageId::new_const(1), MessageId::new_const(1)],
        )
        .await;
    let too_many = client
        .bulk_delete_messages(CHANNEL, (1..=101).map(MessageId::new_const).collect())
        .await;

    assert!(matches!(one, Err(Error::Validation(_))));
    assert!(matches!(duplicate, Err(Error::Validation(_))));
    assert!(matches!(too_many, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn gets_channel_pins_page() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(
        200,
        &json!({
            "items": [{"pinned_at": "2026-02-01T10:00:00.000000+00:00", "message": message_json()}],
            "has_more": true
        }),
    ));
    let before = Timestamp::parse("2026-03-01T00:00:00+00:00").unwrap();

    let pins = client(&transport)
        .channel_pins(CHANNEL)
        .before(before)
        .limit(10)
        .await
        .unwrap();

    assert!(pins.has_more);
    assert_eq!(pins.items.len(), 1);
    assert_eq!(pins.items[0].message.id, MESSAGE);
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!(
            "{API}/channels/1234/messages/pins?before=2026-03-01T00%3A00%3A00.000000%2B00%3A00&limit=10"
        )
    );
    assert_bot_auth(&request);
}

#[tokio::test]
async fn pins_a_message_with_reason() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .pin_message(CHANNEL, MESSAGE)
        .reason("important")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PUT");
    assert_eq!(
        request.url,
        format!("{API}/channels/1234/messages/pins/5678")
    );
    assert_bot_auth(&request);
    assert_eq!(request.header("x-audit-log-reason"), Some("important"));
}

#[tokio::test]
async fn unpins_a_message_with_reason() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .unpin_message(CHANNEL, MESSAGE)
        .reason("old")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(
        request.url,
        format!("{API}/channels/1234/messages/pins/5678")
    );
    assert_bot_auth(&request);
    assert_eq!(request.header("x-audit-log-reason"), Some("old"));
}

#[tokio::test]
#[allow(deprecated)]
async fn uses_legacy_pin_routes() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &json!([message_json()])));
    transport.push(FakeResponse::empty(204));
    transport.push(FakeResponse::empty(204));
    let client = client(&transport);

    let pinned = client.pinned_messages_legacy(CHANNEL).await.unwrap();
    client.pin_message_legacy(CHANNEL, MESSAGE).await.unwrap();
    client.unpin_message_legacy(CHANNEL, MESSAGE).await.unwrap();

    assert_eq!(pinned.len(), 1);
    let requests = transport.requests();
    let calls: Vec<(&str, &str)> = requests
        .iter()
        .map(|request| (request.method.as_str(), request.url.as_str()))
        .collect();
    assert_eq!(
        calls,
        [
            ("GET", "https://discord.com/api/v10/channels/1234/pins"),
            ("PUT", "https://discord.com/api/v10/channels/1234/pins/5678"),
            (
                "DELETE",
                "https://discord.com/api/v10/channels/1234/pins/5678"
            ),
        ]
    );
    for request in &requests {
        assert_bot_auth(request);
    }
}

#[tokio::test]
async fn searches_guild_messages_with_all_filters() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(
        200,
        &json!({
            "doing_deep_historical_index": false,
            "total_results": 1,
            "messages": [[message_json()]],
            "threads": [],
            "members": []
        }),
    ));

    let search = client(&transport)
        .search_guild_messages(GUILD)
        .limit(30)
        .offset(25)
        .max_id(MessageId::new_const(900))
        .min_id(MessageId::new_const(100))
        .slop(3)
        .content("hello world")
        .channel_id(ChannelId::new_const(1))
        .channel_id(ChannelId::new_const(2))
        .author_type(SearchAuthorType::User)
        .exclude_author_type(SearchAuthorType::Bot)
        .author_id(UserId::new_const(3))
        .mentions(UserId::new_const(4))
        .mentions_role_id(RoleId::new_const(5))
        .mention_everyone(false)
        .replied_to_user_id(UserId::new_const(6))
        .replied_to_message_id(MessageId::new_const(7))
        .pinned(true)
        .has(SearchHas::Image)
        .has_not(SearchHas::Link)
        .embed_type(SearchEmbedType::Gif)
        .embed_provider("Tenor")
        .link_hostname("discordapp.com")
        .attachment_filename("notes.txt")
        .attachment_extension("txt")
        .sort_by(SearchSortBy::Relevance)
        .sort_order(SearchSortOrder::Ascending)
        .include_nsfw(true)
        .await
        .unwrap();

    let MessageSearch::Results(results) = search else {
        panic!("expected results, got {search:?}");
    };
    assert_eq!(results.total_results, 1);
    assert_eq!(results.iter_messages().next().unwrap().content, "Hello");

    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    let query = [
        "limit=25",
        "offset=25",
        "max_id=900",
        "min_id=100",
        "slop=3",
        "content=hello%20world",
        "channel_id=1",
        "channel_id=2",
        "author_type=user",
        "author_type=-bot",
        "author_id=3",
        "mentions=4",
        "mentions_role_id=5",
        "mention_everyone=false",
        "replied_to_user_id=6",
        "replied_to_message_id=7",
        "pinned=true",
        "has=image",
        "has=-link",
        "embed_type=gif",
        "embed_provider=Tenor",
        "link_hostname=discordapp.com",
        "attachment_filename=notes.txt",
        "attachment_extension=txt",
        "sort_by=relevance",
        "sort_order=asc",
        "include_nsfw=true",
    ]
    .join("&");
    assert_eq!(
        request.url,
        format!("{API}/guilds/42/messages/search?{query}")
    );
    assert_bot_auth(&request);
}

#[tokio::test]
async fn search_gives_not_indexed_for_status_202() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(
        202,
        &json!({
            "message": "Index not yet available. Try again later",
            "code": 110000,
            "documents_indexed": 0,
            "retry_after": 2
        }),
    ));

    let search = client(&transport)
        .search_guild_messages(GUILD)
        .content("cat")
        .await
        .unwrap();

    let MessageSearch::NotIndexed(not_ready) = search else {
        panic!("expected not indexed, got {search:?}");
    };
    assert_eq!(not_ready.code, 110000);
    assert!((not_ready.retry_after - 2.0).abs() < f64::EPSILON);
    assert_eq!(
        only_request(&transport).url,
        format!("{API}/guilds/42/messages/search?content=cat")
    );
}

#[tokio::test]
async fn refuses_search_with_long_content() {
    let transport = FakeTransport::new();

    let result = client(&transport)
        .search_guild_messages(GUILD)
        .content("a".repeat(1025))
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}
