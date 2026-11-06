//! Tests of the emoji, sticker, and voice endpoints.

use cordial_http::request::FileUpload;
use cordial_http::testing::{FakeResponse, FakeTransport, RecordedRequest};
use cordial_http::{Client, Error, ImageData, Token};
use cordial_model::Timestamp;
use cordial_model::id::{
    ApplicationId, ChannelId, EmojiId, GuildId, RoleId, StickerId, StickerPackId, UserId,
};
use cordial_model::sticker::StickerFormatType;
use serde_json::{Value, json};

const APPLICATION: ApplicationId = ApplicationId::new_const(172_150_183_260_323_840);
const GUILD: GuildId = GuildId::new_const(290_926_798_626_357_260);
const EMOJI: EmojiId = EmojiId::new_const(41_771_983_429_993_937);
const STICKER: StickerId = StickerId::new_const(749_054_660_769_218_631);
const BASE: &str = "https://discord.com/api/v10";

fn client(transport: &FakeTransport) -> Client {
    Client::builder()
        .token(Token::bot("abc"))
        .transport(transport.clone())
        .build()
}

fn only_request(transport: &FakeTransport) -> RecordedRequest {
    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    requests[0].clone()
}

/// The example emoji object of the Discord docs.
fn emoji_json() -> Value {
    json!({
        "id": "41771983429993937",
        "name": "LUL",
        "roles": ["41771983429993000", "41771983429993111"],
        "user": {
            "username": "Luigi",
            "discriminator": "0002",
            "id": "96008815106887111",
            "avatar": "5500909a3274e1812beb4e8de6631111",
            "public_flags": 131328
        },
        "require_colons": true,
        "managed": false,
        "animated": false
    })
}

/// The example sticker object of the Discord docs.
fn sticker_json() -> Value {
    json!({
        "id": "749054660769218631",
        "name": "Wave",
        "tags": "wumpus, hello, sup, hi, oi, heyo, heya, yo, greetings, greet, welcome, wave",
        "type": 1,
        "format_type": 3,
        "description": "Wumpus waves hello",
        "pack_id": "847199849233514549",
        "sort_value": 12
    })
}

/// The example sticker pack object of the Discord docs.
fn sticker_pack_json() -> Value {
    json!({
        "id": "847199849233514549",
        "stickers": [],
        "name": "Wumpus Beyond",
        "sku_id": "847199849233514547",
        "cover_sticker_id": "749053689419006003",
        "description": "Say hello to Wumpus!",
        "banner_asset_id": "761773777976819732"
    })
}

/// The example voice state object of the Discord docs.
fn voice_state_json() -> Value {
    json!({
        "channel_id": "157733188964188161",
        "user_id": "80351110224678912",
        "session_id": "90326bd25d71d39b9ef95b299e3872ff",
        "deaf": false,
        "mute": false,
        "self_deaf": false,
        "self_mute": true,
        "suppress": false,
        "request_to_speak_timestamp": "2021-03-31T18:45:31.297561+00:00"
    })
}

fn png() -> ImageData {
    ImageData::from_data_uri("data:image/png;base64,iVBORw0KGgo=")
}

#[tokio::test]
async fn gets_guild_emojis() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &json!([emoji_json()])));

    let emojis = client(&transport).guild_emojis(GUILD).await.unwrap();

    assert_eq!(emojis[0].name.as_deref(), Some("LUL"));
    assert_eq!(emojis[0].roles.len(), 2);
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!("{BASE}/guilds/290926798626357260/emojis")
    );
}

#[tokio::test]
async fn gets_guild_emoji() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &emoji_json()));

    let emoji = client(&transport).guild_emoji(GUILD, EMOJI).await.unwrap();

    assert_eq!(emoji.id, Some(EMOJI));
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!("{BASE}/guilds/290926798626357260/emojis/41771983429993937")
    );
}

#[tokio::test]
async fn creates_guild_emoji_with_reason() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(201, &emoji_json()));

    client(&transport)
        .create_guild_emoji(GUILD, "LUL", png())
        .roles(vec![RoleId::new_const(41_771_983_429_993_000)])
        .reason("new emoji")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        format!("{BASE}/guilds/290926798626357260/emojis")
    );
    assert_eq!(request.header("x-audit-log-reason"), Some("new%20emoji"));
    assert_eq!(
        request.json(),
        json!({
            "name": "LUL",
            "image": "data:image/png;base64,iVBORw0KGgo=",
            "roles": ["41771983429993000"]
        })
    );
}

#[tokio::test]
async fn refuses_large_emoji_image() {
    let transport = FakeTransport::new();
    let image = ImageData::new("image/png", &vec![0_u8; 256 * 1024 + 1]);

    let result = client(&transport)
        .create_guild_emoji(GUILD, "big", image)
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn edits_guild_emoji() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &emoji_json()));

    client(&transport)
        .edit_guild_emoji(GUILD, EMOJI)
        .name("LOL")
        .clear_roles()
        .reason("rename")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PATCH");
    assert_eq!(
        request.url,
        format!("{BASE}/guilds/290926798626357260/emojis/41771983429993937")
    );
    assert_eq!(request.header("x-audit-log-reason"), Some("rename"));
    assert_eq!(request.json(), json!({"name": "LOL", "roles": null}));
}

#[tokio::test]
async fn deletes_guild_emoji() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_guild_emoji(GUILD, EMOJI)
        .reason("unused")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(
        request.url,
        format!("{BASE}/guilds/290926798626357260/emojis/41771983429993937")
    );
    assert_eq!(request.header("x-audit-log-reason"), Some("unused"));
    assert!(request.body.is_empty());
}

#[tokio::test]
async fn gets_application_emojis() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &json!({"items": [emoji_json()]})));

    let emojis = client(&transport)
        .application_emojis(APPLICATION)
        .await
        .unwrap();

    assert_eq!(emojis.items.len(), 1);
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/emojis")
    );
}

#[tokio::test]
async fn gets_application_emoji() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &emoji_json()));

    let emoji = client(&transport)
        .application_emoji(APPLICATION, EMOJI)
        .await
        .unwrap();

    assert!(emoji.user.is_some());
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/emojis/41771983429993937")
    );
}

#[tokio::test]
async fn creates_application_emoji() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(201, &emoji_json()));

    client(&transport)
        .create_application_emoji(APPLICATION, "LUL", png())
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/emojis")
    );
    assert_eq!(request.header("x-audit-log-reason"), None);
    assert_eq!(
        request.json(),
        json!({"name": "LUL", "image": "data:image/png;base64,iVBORw0KGgo="})
    );
}

#[tokio::test]
async fn edits_application_emoji() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &emoji_json()));

    client(&transport)
        .edit_application_emoji(APPLICATION, EMOJI)
        .name("LOL")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PATCH");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/emojis/41771983429993937")
    );
    assert_eq!(request.json(), json!({"name": "LOL"}));
}

#[tokio::test]
async fn deletes_application_emoji() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_application_emoji(APPLICATION, EMOJI)
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/emojis/41771983429993937")
    );
}

#[tokio::test]
async fn gets_sticker() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &sticker_json()));

    let sticker = client(&transport).sticker(STICKER).await.unwrap();

    assert_eq!(sticker.format_type, StickerFormatType::Lottie);
    assert_eq!(sticker.sort_value, Some(12));
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, format!("{BASE}/stickers/749054660769218631"));
}

#[tokio::test]
async fn gets_sticker_packs() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(
        200,
        &json!({"sticker_packs": [sticker_pack_json()]}),
    ));

    let packs = client(&transport).sticker_packs().await.unwrap();

    assert_eq!(packs.sticker_packs[0].name, "Wumpus Beyond");
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, format!("{BASE}/sticker-packs"));
}

#[tokio::test]
async fn gets_sticker_pack() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &sticker_pack_json()));

    let pack = client(&transport)
        .sticker_pack(StickerPackId::new_const(847_199_849_233_514_549))
        .await
        .unwrap();

    assert_eq!(pack.description, "Say hello to Wumpus!");
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!("{BASE}/sticker-packs/847199849233514549")
    );
}

#[tokio::test]
async fn gets_guild_stickers() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &json!([sticker_json()])));

    let stickers = client(&transport).guild_stickers(GUILD).await.unwrap();

    assert_eq!(stickers[0].name, "Wave");
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!("{BASE}/guilds/290926798626357260/stickers")
    );
}

#[tokio::test]
async fn gets_guild_sticker() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &sticker_json()));

    let sticker = client(&transport)
        .guild_sticker(GUILD, STICKER)
        .await
        .unwrap();

    assert_eq!(sticker.id, STICKER);
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!("{BASE}/guilds/290926798626357260/stickers/749054660769218631")
    );
}

#[tokio::test]
async fn creates_guild_sticker_as_multipart() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(201, &sticker_json()));

    let sticker = client(&transport)
        .create_guild_sticker(
            GUILD,
            "Wave",
            "wave",
            FileUpload::new("wave.png", &b"PNG"[..]).content_type("image/png"),
        )
        .description("Wumpus waves hello")
        .reason("new sticker")
        .await
        .unwrap();

    assert_eq!(sticker.name, "Wave");
    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        format!("{BASE}/guilds/290926798626357260/stickers")
    );
    assert_eq!(request.header("x-audit-log-reason"), Some("new%20sticker"));
    assert!(
        request
            .header("content-type")
            .unwrap()
            .starts_with("multipart/form-data; boundary=")
    );
    let body = String::from_utf8(request.body.to_vec()).unwrap();
    assert!(body.contains("name=\"name\"\r\n\r\nWave\r\n"));
    assert!(body.contains("name=\"description\"\r\n\r\nWumpus waves hello\r\n"));
    assert!(body.contains("name=\"tags\"\r\n\r\nwave\r\n"));
    assert!(body.contains(
        "name=\"file\"; filename=\"wave.png\"\r\nContent-Type: image/png\r\n\r\nPNG\r\n"
    ));
    assert!(!body.contains("payload_json"));
}

#[tokio::test]
async fn refuses_short_sticker_name() {
    let transport = FakeTransport::new();

    let result = client(&transport)
        .create_guild_sticker(GUILD, "W", "wave", FileUpload::new("w.png", &b"x"[..]))
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn edits_guild_sticker() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &sticker_json()));

    client(&transport)
        .edit_guild_sticker(GUILD, STICKER)
        .name("Wave 2")
        .clear_description()
        .tags("wave")
        .reason("update")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PATCH");
    assert_eq!(
        request.url,
        format!("{BASE}/guilds/290926798626357260/stickers/749054660769218631")
    );
    assert_eq!(request.header("x-audit-log-reason"), Some("update"));
    assert_eq!(
        request.json(),
        json!({"name": "Wave 2", "description": null, "tags": "wave"})
    );
}

#[tokio::test]
async fn deletes_guild_sticker() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_guild_sticker(GUILD, STICKER)
        .reason("old")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(
        request.url,
        format!("{BASE}/guilds/290926798626357260/stickers/749054660769218631")
    );
    assert_eq!(request.header("x-audit-log-reason"), Some("old"));
}

#[tokio::test]
async fn gets_voice_regions() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(
        200,
        &json!([{"id": "us-west", "name": "US West", "optimal": true, "deprecated": false, "custom": false}]),
    ));

    let regions = client(&transport).voice_regions().await.unwrap();

    assert_eq!(regions[0].id, "us-west");
    assert!(regions[0].optimal);
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, format!("{BASE}/voice/regions"));
}

#[tokio::test]
async fn gets_current_voice_state() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &voice_state_json()));

    let state = client(&transport).current_voice_state(GUILD).await.unwrap();

    assert!(state.self_mute);
    assert!(state.request_to_speak_timestamp.is_some());
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!("{BASE}/guilds/290926798626357260/voice-states/@me")
    );
}

#[tokio::test]
async fn gets_user_voice_state() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &voice_state_json()));

    let state = client(&transport)
        .voice_state(GUILD, UserId::new_const(80_351_110_224_678_912))
        .await
        .unwrap();

    assert_eq!(
        state.channel_id,
        Some(ChannelId::new_const(157_733_188_964_188_161))
    );
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!("{BASE}/guilds/290926798626357260/voice-states/80351110224678912")
    );
}

#[tokio::test]
async fn edits_current_voice_state() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .edit_current_voice_state(GUILD)
        .channel_id(ChannelId::new_const(157_733_188_964_188_161))
        .suppress(false)
        .request_to_speak_timestamp(Timestamp::from_unix_seconds(1_617_216_331))
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PATCH");
    assert_eq!(
        request.url,
        format!("{BASE}/guilds/290926798626357260/voice-states/@me")
    );
    let body = request.json();
    assert_eq!(body["channel_id"], "157733188964188161");
    assert_eq!(body["suppress"], false);
    let timestamp = body["request_to_speak_timestamp"].as_str().unwrap();
    assert_eq!(
        Timestamp::parse(timestamp).unwrap(),
        Timestamp::from_unix_seconds(1_617_216_331)
    );
}

#[tokio::test]
async fn clears_request_to_speak() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .edit_current_voice_state(GUILD)
        .clear_request_to_speak()
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.json(), json!({"request_to_speak_timestamp": null}));
}

#[tokio::test]
async fn edits_user_voice_state() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .edit_voice_state(GUILD, UserId::new_const(80_351_110_224_678_912))
        .channel_id(ChannelId::new_const(157_733_188_964_188_161))
        .suppress(true)
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PATCH");
    assert_eq!(
        request.url,
        format!("{BASE}/guilds/290926798626357260/voice-states/80351110224678912")
    );
    assert_eq!(
        request.json(),
        json!({"channel_id": "157733188964188161", "suppress": true})
    );
}
