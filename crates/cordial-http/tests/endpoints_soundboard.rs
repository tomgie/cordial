//! Tests of the soundboard endpoints with a fake transport.

use cordial_http::endpoints::soundboard::SoundData;
use cordial_http::testing::{FakeResponse, FakeTransport, RecordedRequest};
use cordial_http::{Client, Error, Token};
use cordial_model::id::{ChannelId, EmojiId, GuildId, SoundboardSoundId};
use serde_json::json;

const API: &str = "https://discord.com/api/v10";
const GUILD: GuildId = GuildId::new_const(613425648685547541);
const SOUND: SoundboardSoundId = SoundboardSoundId::new_const(1106714396018884649);

fn client(transport: &FakeTransport) -> Client {
    Client::builder()
        .token(Token::bot("abc"))
        .transport(transport.clone())
        .build()
}

fn guild_sound_json() -> serde_json::Value {
    json!({
        "name": "Yay",
        "sound_id": "1106714396018884649",
        "volume": 1,
        "emoji_id": "989193655938064464",
        "emoji_name": null,
        "guild_id": "613425648685547541",
        "available": true
    })
}

fn only_request(transport: &FakeTransport) -> RecordedRequest {
    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    requests.into_iter().next().unwrap()
}

fn sounds_url(rest: &str) -> String {
    format!("{API}/guilds/613425648685547541/soundboard-sounds{rest}")
}

#[tokio::test]
async fn sends_a_soundboard_sound() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .send_soundboard_sound(ChannelId::new_const(99), SOUND)
        .source_guild_id(GUILD)
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        format!("{API}/channels/99/send-soundboard-sound")
    );
    assert_eq!(request.header("authorization"), Some("Bot abc"));
    assert_eq!(
        request.json(),
        json!({"sound_id": "1106714396018884649", "source_guild_id": "613425648685547541"})
    );
}

#[tokio::test]
async fn lists_default_sounds() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(
        200,
        &json!([{
            "name": "quack",
            "sound_id": "1",
            "volume": 1.0,
            "emoji_id": null,
            "emoji_name": "🦆",
            "available": true
        }]),
    ));

    let sounds = client(&transport)
        .default_soundboard_sounds()
        .await
        .unwrap();

    assert_eq!(sounds[0].name, "quack");
    assert_eq!(sounds[0].emoji_name.as_deref(), Some("🦆"));
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, format!("{API}/soundboard-default-sounds"));
    assert_eq!(request.header("authorization"), Some("Bot abc"));
}

#[tokio::test]
async fn lists_guild_sounds() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(
        200,
        &json!({"items": [guild_sound_json()]}),
    ));

    let sounds = client(&transport)
        .guild_soundboard_sounds(GUILD)
        .await
        .unwrap();

    assert_eq!(sounds.items.len(), 1);
    assert_eq!(sounds.items[0].guild_id, Some(GUILD));
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, sounds_url(""));
    assert_eq!(request.header("authorization"), Some("Bot abc"));
}

#[tokio::test]
async fn gets_a_guild_sound() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &guild_sound_json()));

    let sound = client(&transport)
        .guild_soundboard_sound(GUILD, SOUND)
        .await
        .unwrap();

    assert_eq!(sound.sound_id, SOUND);
    assert_eq!(sound.emoji_id, Some(EmojiId::new_const(989193655938064464)));
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, sounds_url("/1106714396018884649"));
    assert_eq!(request.header("authorization"), Some("Bot abc"));
}

#[tokio::test]
async fn creates_a_guild_sound() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(201, &guild_sound_json()));

    let sound = client(&transport)
        .create_guild_soundboard_sound(GUILD, "Yay", SoundData::from_bytes(b"OggS\0\0"))
        .volume(0.5)
        .emoji_id(EmojiId::new_const(989193655938064464))
        .reason("new sound")
        .await
        .unwrap();

    assert_eq!(sound.name, "Yay");
    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, sounds_url(""));
    assert_eq!(request.header("authorization"), Some("Bot abc"));
    assert_eq!(request.header("x-audit-log-reason"), Some("new%20sound"));
    assert_eq!(
        request.json(),
        json!({
            "name": "Yay",
            "sound": "data:audio/ogg;base64,T2dnUwAA",
            "volume": 0.5,
            "emoji_id": "989193655938064464"
        })
    );
}

#[tokio::test]
async fn refuses_a_bad_sound_name_or_volume() {
    let transport = FakeTransport::new();
    let client = client(&transport);

    let short = client
        .create_guild_soundboard_sound(GUILD, "Y", SoundData::from_bytes(b"ID3"))
        .await;
    let loud = client
        .modify_guild_soundboard_sound(GUILD, SOUND)
        .volume(Some(1.5))
        .await;

    assert!(matches!(short, Err(Error::Validation(_))));
    assert!(matches!(loud, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn modifies_a_guild_sound() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &guild_sound_json()));

    client(&transport)
        .modify_guild_soundboard_sound(GUILD, SOUND)
        .name("Hooray")
        .emoji_id(None)
        .emoji_name(Some("🎉".to_owned()))
        .reason("rename")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.url, sounds_url("/1106714396018884649"));
    assert_eq!(request.header("authorization"), Some("Bot abc"));
    assert_eq!(request.header("x-audit-log-reason"), Some("rename"));
    assert_eq!(
        request.json(),
        json!({"name": "Hooray", "emoji_id": null, "emoji_name": "🎉"})
    );
}

#[tokio::test]
async fn deletes_a_guild_sound() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_guild_soundboard_sound(GUILD, SOUND)
        .reason("cleanup")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.url, sounds_url("/1106714396018884649"));
    assert_eq!(request.header("authorization"), Some("Bot abc"));
    assert_eq!(request.header("x-audit-log-reason"), Some("cleanup"));
}
