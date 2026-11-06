use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cordial::framework::error::HandlerErrorKind;
use cordial::framework::{
    Bot, BoxError, Check, Command, CommandContext, Context, EventOrder, HandlerError,
    InteractionContext, ResponseState,
};
use cordial::http::Token;
use cordial::model::gateway::Event;
use cordial::model::gateway::event::{GuildCreate, MessageCreate};
use cordial::model::id::UserId;
use cordial::model::interaction::InteractionMessage;

// ANCHOR: state
/// The bot state. Each handler gets it with `ctx.state()`.
#[derive(Default)]
struct State {
    events: AtomicU64,
    messages: AtomicU64,
}

async fn count_all(ctx: Context<State>, _: Arc<Event>) {
    ctx.state().events.fetch_add(1, Ordering::Relaxed);
}

async fn count_message(ctx: Context<State>, _: MessageCreate) {
    ctx.state().messages.fetch_add(1, Ordering::Relaxed);
}

async fn guild_create(_: Context<State>, guild: GuildCreate) -> Result<(), BoxError> {
    tracing::info!(guild_id = %guild.id(), "a guild is available");
    Ok(())
}
// ANCHOR_END: state

// ANCHOR: error_hook
async fn on_error(_: Context<State>, error: HandlerError) {
    match &error.kind {
        HandlerErrorKind::Panic(message) => {
            tracing::error!(source = %error.source, message, "a handler panicked");
        }
        _ => tracing::error!(%error, "a handler failed"),
    }
}
// ANCHOR_END: error_hook

// ANCHOR: interaction_error
/// Answers the user of a failed interaction. The context knows the response state.
async fn on_interaction_error(ctx: InteractionContext<State>, error: HandlerError) {
    tracing::error!(%error, "an interaction handler failed");

    let text = "Something went wrong. Try again later.";
    let result = match ctx.response_state().await {
        ResponseState::Pending | ResponseState::Deferred { .. } => {
            ctx.reply(text).ephemeral().await
        }
        _ => ctx
            .followup(InteractionMessage::text(text).ephemeral())
            .await
            .map(drop),
    };
    if let Err(error) = result {
        tracing::warn!(%error, "the error message failed");
    }
}
// ANCHOR_END: interaction_error

// ANCHOR: shutdown
/// Stops the bot.
#[derive(Command)]
#[command(
    name = "stop",
    description = "Stop the bot",
    default_member_permissions = ""
)]
struct Stop;

async fn stop(ctx: CommandContext<State>, _: Stop) -> Result<(), BoxError> {
    ctx.reply("The bot stops.").ephemeral().await?;
    // `Bot::run` returns after the shards close.
    ctx.shutdown();
    Ok(())
}
// ANCHOR_END: shutdown

// ANCHOR: builder
async fn run(token: Token) -> Result<(), BoxError> {
    let bot = Bot::builder(token)
        // Call `state` first: the handlers use the state type.
        .state(State::default())
        .event_order(EventOrder::PerShard)
        .on_any(count_all)
        .on(count_message)
        .on(guild_create)
        .on_error(on_error)
        .on_interaction_error(on_interaction_error)
        .command_with(stop, |settings| {
            settings.check(Check::users([UserId::new_const(1234)]))
        })
        .build()?;

    let context = bot.context();
    bot.run().await?;
    let count = context.state().events.load(Ordering::Relaxed);
    tracing::info!(count, "the bot stopped");
    Ok(())
}
// ANCHOR_END: builder
