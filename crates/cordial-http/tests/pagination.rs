//! Tests of pagination.

use cordial_http::testing::{FakeResponse, FakeTransport};
use cordial_http::{Client, Pages, Token};
use cordial_model::id::ChannelId;
use serde_json::{Value, json};

fn message(id: u64) -> Value {
    json!({
        "id": id.to_string(), "channel_id": "1", "content": "", "timestamp": "2026-01-01T00:00:00+00:00",
        "edited_timestamp": null, "tts": false, "mention_everyone": false, "mentions": [], "mention_roles": [],
        "attachments": [], "embeds": [], "pinned": false, "type": 0,
        "author": {"id": "9", "username": "u", "discriminator": "0", "avatar": null}
    })
}

#[tokio::test]
async fn generic_pages_follow_the_cursor() {
    let data: Vec<u32> = (1..=5).collect();
    let pages = Pages::new(
        2,
        move |after: Option<u32>| {
            let page: Vec<u32> = data
                .iter()
                .copied()
                .filter(|value| after.is_none_or(|after| *value > after))
                .take(2)
                .collect();
            Box::pin(async move { Ok(page) })
        },
        |value: &u32| Some(*value),
    );

    let all = pages.collect(None).await.unwrap();

    assert_eq!(all, vec![1, 2, 3, 4, 5]);
}

#[tokio::test]
async fn collect_stops_at_the_limit() {
    let pages = Pages::new(
        2,
        |after: Option<u32>| {
            let start = after.unwrap_or(0);
            Box::pin(async move { Ok(vec![start + 1, start + 2]) })
        },
        |value: &u32| Some(*value),
    );

    let some = pages.collect(Some(5)).await.unwrap();

    assert_eq!(some, vec![1, 2, 3, 4, 5]);
}

#[tokio::test]
async fn message_history_uses_before_cursor() {
    let transport = FakeTransport::new();
    let first: Vec<Value> = (101..=200).rev().map(message).collect();
    transport.push(FakeResponse::json(200, &Value::Array(first)));
    transport.push(FakeResponse::json(200, &json!([message(50)])));
    let client = Client::builder()
        .token(Token::bot("abc"))
        .transport(transport.clone())
        .build();

    let all = client
        .message_history(ChannelId::new_const(1))
        .collect(None)
        .await
        .unwrap();

    assert_eq!(all.len(), 101);
    let requests = transport.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].url.ends_with("/channels/1/messages?limit=100"));
    assert!(
        requests[1]
            .url
            .ends_with("/channels/1/messages?before=101&limit=100")
    );
}
