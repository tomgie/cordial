//! Tests of the dispatch: events in, HTTP requests out.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use cordial_http::testing::{FakeResponse, FakeTransport, RecordedRequest};
use cordial_http::{Client, Token};
use cordial_model::command::{CommandDefinition, CommandType};
use cordial_model::gateway::event::{Event, MessageCreate};
use cordial_model::id::ApplicationId;
use cordial_model::interaction::CommandData;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::error::HandlerErrorKind;
use crate::{
    Bot, BotBuilder, Check, Command, CommandContext, ComponentContext, Context, HandlerError,
    ParseError, SlowResponse,
};

const BASE: &str = "https://discord.com/api/v10";

struct Ping;

impl Command for Ping {
    const NAME: &'static str = "ping";
    const KIND: CommandType = CommandType::ChatInput;

    fn definition() -> CommandDefinition {
        CommandDefinition::chat_input("ping", "Ping")
    }

    fn parse(_: &CommandData) -> Result<Self, ParseError> {
        Ok(Self)
    }
}

fn builder(transport: &FakeTransport) -> BotBuilder<()> {
    let client = Client::builder()
        .token(Token::bot("abc"))
        .transport(transport.clone())
        .build();
    Bot::builder(Token::bot("abc")).http(client)
}

fn start<S: Send + Sync + 'static>(builder: BotBuilder<S>) -> Bot<S> {
    let bot = builder.build().unwrap();
    let _ = bot
        .context
        .shared
        .application_id
        .set(ApplicationId::new_const(2));
    bot
}

fn event(name: &str, data: &Value) -> Arc<Event> {
    let raw = serde_json::value::RawValue::from_string(data.to_string()).unwrap();
    Arc::new(Event::decode(name, &raw).unwrap())
}

fn interaction(kind: u8, data: &Value) -> Arc<Event> {
    event(
        "INTERACTION_CREATE",
        &json!({
            "id": "1", "application_id": "2", "type": kind, "data": data,
            "guild_id": "4", "channel_id": "5", "token": "tok", "version": 1,
            "member": {"user": {"id": "6", "username": "user", "discriminator": "0", "avatar": null},
                       "roles": [], "joined_at": "2026-01-01T00:00:00+00:00", "deaf": false,
                       "mute": false, "flags": 0, "permissions": "0"},
            "app_permissions": "0", "locale": "en-US", "entitlements": [],
            "authorizing_integration_owners": {}, "attachment_size_limit": 1000
        }),
    )
}

fn command(name: &str) -> Arc<Event> {
    interaction(2, &json!({"id": "3", "name": name, "type": 1}))
}

fn dispatch<S: Send + Sync + 'static>(bot: &Bot<S>, event: Arc<Event>) {
    bot.dispatch(0, event, &mut HashMap::new());
}

/// Waits until the transport has a number of requests.
async fn requests(transport: &FakeTransport, count: usize) -> Vec<RecordedRequest> {
    for _ in 0..200 {
        let requests = transport.requests();
        if requests.len() >= count {
            return requests;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("expected {count} requests, got {:?}", transport.requests());
}

fn callback_path() -> String {
    format!("{BASE}/interactions/1/tok/callback")
}

#[tokio::test]
async fn command_reply_is_the_initial_response() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));
    let bot = start(builder(&transport).command::<Ping, _, _, _>(
        |ctx: CommandContext<()>, _| async move { ctx.reply("Pong").await },
    ));

    dispatch(&bot, command("ping"));

    let requests = requests(&transport, 1).await;
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].url, callback_path());
    assert_eq!(
        requests[0].json(),
        json!({"type": 4, "data": {"content": "Pong"}})
    );
}

#[tokio::test]
async fn reply_after_defer_edits_the_original() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));
    transport.push(FakeResponse::json(200, &message_json()));
    let bot = start(builder(&transport).command::<Ping, _, _, _>(
        |ctx: CommandContext<()>, _| async move {
            ctx.defer_ephemeral().await?;
            ctx.reply("Done").ephemeral().await
        },
    ));

    dispatch(&bot, command("ping"));

    let requests = requests(&transport, 2).await;
    assert_eq!(
        requests[0].json(),
        json!({"type": 5, "data": {"flags": 64}})
    );
    assert_eq!(requests[1].method, "PATCH");
    assert_eq!(
        requests[1].url,
        format!("{BASE}/webhooks/2/tok/messages/@original")
    );
    assert_eq!(requests[1].json()["content"], "Done");
}

#[tokio::test]
async fn failed_check_gives_an_ephemeral_reply() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));
    let runs = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&runs);
    let bot = start(builder(&transport).command_with::<Ping, _, _, _>(
        move |_: CommandContext<()>, _| {
            counter.fetch_add(1, Ordering::SeqCst);
            async {}
        },
        |settings| settings.check(Check::permissions(cordial_model::Permissions::BAN_MEMBERS)),
    ));

    dispatch(&bot, command("ping"));

    let requests = requests(&transport, 1).await;
    let body = requests[0].json();
    assert_eq!(body["data"]["flags"], 64);
    assert!(
        body["data"]["content"]
            .as_str()
            .unwrap()
            .contains("BAN_MEMBERS")
    );
    assert_eq!(runs.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn handler_error_without_hook_gives_an_error_reply() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));
    let bot = start(
        builder(&transport)
            .command::<Ping, _, _, _>(|_: CommandContext<()>, _| async { Err::<(), _>("broken") }),
    );

    dispatch(&bot, command("ping"));

    let requests = requests(&transport, 1).await;
    assert_eq!(
        requests[0].json(),
        json!({"type": 4, "data": {"content": "An error occurred.", "flags": 64}})
    );
}

#[tokio::test]
async fn panic_goes_to_the_error_hook() {
    let transport = FakeTransport::new();
    let errors: Arc<Mutex<Vec<String>>> = Arc::default();
    let sink = Arc::clone(&errors);
    let bot = start(
        builder(&transport)
            .on::<MessageCreate, _, _, _>(|_: Context<()>, _| async {
                if true {
                    panic!("boom");
                }
            })
            .on_error(move |_, error: HandlerError| {
                let sink = Arc::clone(&sink);
                async move {
                    if let HandlerErrorKind::Panic(message) = error.kind {
                        sink.lock().await.push(message);
                    }
                }
            }),
    );

    dispatch(&bot, event("MESSAGE_CREATE", &message_json()));

    for _ in 0..200 {
        if !errors.lock().await.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(*errors.lock().await, vec!["boom".to_owned()]);
}

#[tokio::test]
async fn component_pattern_gives_parameters() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));
    let bot = start(builder(&transport).component(
        "vote:{poll}:{choice}",
        |ctx: ComponentContext<()>| async move {
            let poll: u64 = ctx.param("poll")?;
            let choice = ctx.params().get("choice").unwrap_or_default().to_owned();
            ctx.update_message(format!("{poll}/{choice}")).await?;
            Ok::<(), crate::BoxError>(())
        },
    ));

    dispatch(
        &bot,
        interaction(3, &json!({"custom_id": "vote:7:yes", "component_type": 2})),
    );

    let requests = requests(&transport, 1).await;
    assert_eq!(
        requests[0].json(),
        json!({"type": 7, "data": {"content": "7/yes"}})
    );
}

#[tokio::test]
async fn slow_handler_gets_an_automatic_defer() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));
    transport.push(FakeResponse::json(200, &message_json()));
    let bot = start(
        builder(&transport)
            .slow_response(SlowResponse::Defer(Duration::from_millis(20)))
            .command::<Ping, _, _, _>(|ctx: CommandContext<()>, _| async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                ctx.reply("Late").await
            }),
    );

    dispatch(&bot, command("ping"));

    let requests = requests(&transport, 2).await;
    assert_eq!(requests[0].json(), json!({"type": 5}));
    assert_eq!(requests[1].method, "PATCH");
}

#[tokio::test]
async fn collector_gets_matching_events() {
    let transport = FakeTransport::new();
    let bot = start(builder(&transport));
    let context = bot.context();

    let waiter = tokio::spawn(async move {
        context
            .wait_for(
                |message: &MessageCreate| message.content == "hello",
                Duration::from_secs(1),
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(10)).await;
    dispatch(&bot, event("MESSAGE_CREATE", &message_json()));

    let message = waiter.await.unwrap().unwrap();
    assert_eq!(message.content, "hello");
}

#[tokio::test]
async fn state_after_handlers_is_an_error() {
    let transport = FakeTransport::new();
    let builder = builder(&transport)
        .on::<MessageCreate, _, _, _>(|_: Context<()>, _| async {})
        .state(5_u8);

    assert!(builder.build().is_err());
}

fn message_json() -> Value {
    json!({
        "id": "9", "channel_id": "5", "author": {"id": "6", "username": "user", "discriminator": "0", "avatar": null},
        "content": "hello", "timestamp": "2026-01-01T00:00:00+00:00", "edited_timestamp": null,
        "tts": false, "mention_everyone": false, "mentions": [], "mention_roles": [],
        "attachments": [], "embeds": [], "pinned": false, "type": 0
    })
}
