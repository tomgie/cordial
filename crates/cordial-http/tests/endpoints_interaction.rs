//! Tests of the interaction response and follow-up endpoints with a fake transport.

use cordial_http::request::FileUpload;
use cordial_http::testing::{FakeResponse, FakeTransport, RecordedRequest};
use cordial_http::{Client, Error, Token};
use cordial_model::id::{ApplicationId, InteractionId, MessageId};
use cordial_model::interaction::{InteractionCallbackType, InteractionResponse};
use serde_json::json;

const API: &str = "https://discord.com/api/v10";
const APPLICATION: ApplicationId = ApplicationId::new_const(11);
const INTERACTION: InteractionId = InteractionId::new_const(22);
const TOKEN: &str = "aW50ZXJhY3Rpb246MjI6c2VjcmV0";

fn client(transport: &FakeTransport) -> Client {
    Client::builder()
        .token(Token::bot("abc"))
        .transport(transport.clone())
        .build()
}

fn message_json() -> serde_json::Value {
    json!({
        "id": "5678", "channel_id": "1234", "content": "Pong!",
        "timestamp": "2026-01-01T00:00:00+00:00", "edited_timestamp": null,
        "tts": false, "mention_everyone": false, "mentions": [], "mention_roles": [],
        "attachments": [], "embeds": [], "pinned": false, "type": 20, "flags": 64,
        "webhook_id": "11", "application_id": "11",
        "author": {"id": "11", "username": "bot", "discriminator": "0", "avatar": null, "bot": true}
    })
}

fn response_from(value: serde_json::Value) -> InteractionResponse {
    serde_json::from_value(value).unwrap()
}

fn only_request(transport: &FakeTransport) -> RecordedRequest {
    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    requests.into_iter().next().unwrap()
}

fn webhook_url(rest: &str) -> String {
    format!("{API}/webhooks/11/{TOKEN}{rest}")
}

#[tokio::test]
async fn creates_an_interaction_response() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));
    let response = response_from(json!({
        "type": 4,
        "data": {"content": "Congrats on sending your command!"}
    }));

    let () = client(&transport)
        .create_interaction_response(INTERACTION, TOKEN, response)
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        format!("{API}/interactions/22/{TOKEN}/callback")
    );
    assert_eq!(request.header("authorization"), None);
    assert_eq!(request.header("content-type"), Some("application/json"));
    assert_eq!(
        request.json(),
        json!({"type": 4, "data": {"content": "Congrats on sending your command!"}})
    );
}

#[tokio::test]
async fn creates_an_interaction_response_with_callback_and_file() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(
        200,
        &json!({
            "interaction": {
                "id": "22",
                "type": 2,
                "response_message_id": "5678",
                "response_message_loading": false,
                "response_message_ephemeral": true
            },
            "resource": {"type": 4, "message": message_json()}
        }),
    ));
    let response = response_from(json!({
        "type": 4,
        "data": {"content": "Here is the chart", "flags": 64}
    }));

    let callback = client(&transport)
        .create_interaction_response(INTERACTION, TOKEN, response)
        .file(FileUpload::new("chart.png", &b"PNG"[..]).content_type("image/png"))
        .with_response()
        .await
        .unwrap();

    assert_eq!(
        callback.interaction.response_message_id,
        Some(MessageId::new_const(5678))
    );
    let resource = callback.resource.unwrap();
    assert_eq!(
        resource.kind,
        InteractionCallbackType::ChannelMessageWithSource
    );
    assert_eq!(resource.message.unwrap().content, "Pong!");

    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        format!("{API}/interactions/22/{TOKEN}/callback?with_response=true")
    );
    assert_eq!(request.header("authorization"), None);
    assert!(
        request
            .header("content-type")
            .unwrap()
            .starts_with("multipart/form-data; boundary=")
    );
    let body = String::from_utf8(request.body.to_vec()).unwrap();
    assert!(body.contains(r#""attachments":[{"id":0,"filename":"chart.png"}]"#));
    assert!(body.contains("name=\"files[0]\"; filename=\"chart.png\""));
}

#[tokio::test]
async fn refuses_an_invalid_modal() {
    let transport = FakeTransport::new();
    let response = response_from(json!({
        "type": 9,
        "data": {"custom_id": "form", "title": "A title", "components": []}
    }));

    let result = client(&transport)
        .create_interaction_response(INTERACTION, TOKEN, response)
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn gets_the_original_response() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &message_json()));

    let message = client(&transport)
        .original_interaction_response(APPLICATION, TOKEN)
        .await
        .unwrap();

    assert_eq!(message.content, "Pong!");
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, webhook_url("/messages/@original"));
    assert_eq!(request.header("authorization"), None);
}

#[tokio::test]
async fn edits_the_original_response() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &message_json()));

    client(&transport)
        .edit_original_interaction_response(APPLICATION, TOKEN)
        .content("Done")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.url, webhook_url("/messages/@original"));
    assert_eq!(request.header("authorization"), None);
    assert_eq!(request.json(), json!({"content": "Done"}));
}

#[tokio::test]
async fn deletes_the_original_response() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_original_interaction_response(APPLICATION, TOKEN)
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.url, webhook_url("/messages/@original"));
    assert_eq!(request.header("authorization"), None);
}

#[tokio::test]
async fn creates_an_ephemeral_followup() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &message_json()));

    let message = client(&transport)
        .create_followup(APPLICATION, TOKEN)
        .content("Only you can see this")
        .ephemeral()
        .await
        .unwrap();

    assert_eq!(message.id, MessageId::new_const(5678));
    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, webhook_url(""));
    assert_eq!(request.header("authorization"), None);
    assert_eq!(
        request.json(),
        json!({"content": "Only you can see this", "flags": 64})
    );
}

#[tokio::test]
async fn refuses_an_empty_followup() {
    let transport = FakeTransport::new();

    let result = client(&transport).create_followup(APPLICATION, TOKEN).await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn gets_a_followup() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &message_json()));

    client(&transport)
        .followup(APPLICATION, TOKEN, MessageId::new_const(5678))
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, webhook_url("/messages/5678"));
    assert_eq!(request.header("authorization"), None);
}

#[tokio::test]
async fn edits_a_followup_with_a_file() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &message_json()));

    client(&transport)
        .edit_followup(APPLICATION, TOKEN, MessageId::new_const(5678))
        .clear_content()
        .file(FileUpload::new("result.txt", &b"42"[..]))
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.url, webhook_url("/messages/5678"));
    assert_eq!(request.header("authorization"), None);
    let body = String::from_utf8(request.body.to_vec()).unwrap();
    assert!(body.contains(r#""content":"""#));
    assert!(body.contains("name=\"files[0]\"; filename=\"result.txt\""));
}

#[tokio::test]
async fn deletes_a_followup() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_followup(APPLICATION, TOKEN, MessageId::new_const(5678))
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.url, webhook_url("/messages/5678"));
    assert_eq!(request.header("authorization"), None);
}
