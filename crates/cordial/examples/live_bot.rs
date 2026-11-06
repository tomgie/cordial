//! A live test of the interactions. A person uses the test commands in one guild.
//!
//! Run it with `DISCORD_TOKEN=... cargo run --example live_bot -- <mode> <guild id> <keep file>`.
//!
//! | Mode | What it does |
//! |---|---|
//! | `check` | Shows the sync plan of `run` and changes nothing. |
//! | `run` | Registers the test commands in the guild and runs the bot. |
//! | `clear` | Removes the test commands from the guild. |
//!
//! The keep file is a JSON list of guild commands (`GET .../commands?with_localizations=true`)
//! that must stay in the guild. The sync includes them, so they stay unchanged. The program
//! does not change the global commands: the bot sync is off.

use std::time::Duration;

use cordial::framework::command::ResolvedUser;
use cordial::framework::sync::{SyncCommand, SyncOptions, SyncScope, sync};
use cordial::framework::{
    Bot, BoxError, Command, CommandContext, CommandSync, ComponentContext, Cooldown, CooldownScope,
    Focused, Modal, SlowResponse,
};
use cordial::http::{FileUpload, Token};
use cordial::model::Color;
use cordial::model::command::{CommandDefinition, CommandOptionChoice};
use cordial::model::component::{
    ActionRow, Button, ButtonStyle, Container, SelectOption, Separator, StringSelect, TextDisplay,
};
use cordial::model::embed::Embed;
use cordial::model::gateway::event::MessageCreate;
use cordial::model::gateway::{EventType, Intents};
use cordial::model::id::GuildId;
use cordial::model::interaction::InteractionMessage;
use cordial::model::message::Message;

/// Checks that the bot answers.
#[derive(Command)]
#[command(name = "ct-ping", description = "Test: check that the bot answers")]
struct CtPing;

async fn ct_ping(ctx: CommandContext<()>, _: CtPing) -> Result<(), BoxError> {
    let latency = ctx.shard(0).and_then(|shard| shard.latency()).map_or_else(
        || "unknown".to_owned(),
        |latency| format!("{} ms", latency.as_millis()),
    );
    ctx.reply(format!("Pong! Gateway latency: {latency}."))
        .await?;
    Ok(())
}

/// Sends a channel message with a file, then a reply with a file.
#[derive(Command)]
#[command(name = "ct-attach", description = "Test: send messages with files")]
struct CtAttach;

async fn ct_attach(ctx: CommandContext<()>, _: CtAttach) -> Result<(), BoxError> {
    ctx.defer().await?;

    let channel_id = ctx.channel_id()?;
    let file = FileUpload::new("channel.txt", b"A file in a channel message.".as_slice())
        .content_type("text/plain");
    let embed = Embed::new()
        .title("Channel message")
        .description("This message has a file.");
    ctx.http()
        .create_message(channel_id)
        .content("Test: a channel message with an attachment.")
        .embed(embed)
        .file(file)
        .await?;

    let reply_file = FileUpload::new("reply.txt", b"A file in an interaction reply.".as_slice())
        .content_type("text/plain");
    ctx.reply("Test: an interaction reply with an attachment.")
        .file(reply_file)
        .await?;
    Ok(())
}

/// Defers, then replies, sends a follow-up, and edits the first reply.
#[derive(Command)]
#[command(
    name = "ct-defer",
    description = "Test: defer, reply, follow-up, and edit"
)]
struct CtDefer;

async fn ct_defer(ctx: CommandContext<()>, _: CtDefer) -> Result<(), BoxError> {
    ctx.defer_ephemeral().await?;
    tokio::time::sleep(Duration::from_secs(4)).await;

    ctx.reply("First reply, after 4 seconds.")
        .ephemeral()
        .await?;
    ctx.reply("A follow-up message.").ephemeral().await?;
    ctx.edit_reply(InteractionMessage::text("First reply, edited."))
        .await?;
    Ok(())
}

/// Takes 5 seconds without a defer. The bot defers it after 2 seconds.
#[derive(Command)]
#[command(name = "ct-slow", description = "Test: the automatic defer")]
struct CtSlow;

async fn ct_slow(ctx: CommandContext<()>, _: CtSlow) -> Result<(), BoxError> {
    tokio::time::sleep(Duration::from_secs(5)).await;
    ctx.reply("This reply came after 5 seconds. The bot deferred it.")
        .await?;
    Ok(())
}

/// Shows buttons and a select menu.
#[derive(Command)]
#[command(
    name = "ct-panel",
    description = "Test: buttons, a select menu, and a modal"
)]
struct CtPanel;

fn panel(count: u32) -> InteractionMessage {
    let options = ["red", "green", "blue"]
        .map(|color| SelectOption::new(color, color))
        .to_vec();
    InteractionMessage::text(format!("Count: {count}"))
        .component(ActionRow::new([
            Button::new(ButtonStyle::Primary, format!("ct:count:{count}"), "Add one"),
            Button::new(ButtonStyle::Secondary, "ct:form", "Open a form"),
        ]))
        .component(ActionRow::new([StringSelect::new("ct:pick", options)]))
}

async fn ct_panel(ctx: CommandContext<()>, _: CtPanel) -> Result<(), BoxError> {
    ctx.reply(panel(0)).await?;
    Ok(())
}

async fn ct_count(ctx: ComponentContext<()>) -> Result<(), BoxError> {
    let count: u32 = ctx.param("count")?;
    ctx.update_message(panel(count + 1)).await?;
    Ok(())
}

async fn ct_pick(ctx: ComponentContext<()>) -> Result<(), BoxError> {
    let picked = ctx
        .interaction()
        .component_data()
        .map(|data| data.values.join(", "))
        .unwrap_or_default();
    ctx.reply(format!("You picked {picked}."))
        .ephemeral()
        .await?;
    Ok(())
}

/// The test form.
#[derive(Modal)]
#[modal(title = "Test form")]
struct CtForm {
    #[field(label = "Name", max_length = 50)]
    name: String,
    #[field(label = "Comment", style = "paragraph", max_length = 500)]
    comment: Option<String>,
}

async fn ct_open_form(ctx: ComponentContext<()>) -> Result<(), BoxError> {
    ctx.modal(CtForm::modal("ct:form-submit")).await?;
    Ok(())
}

async fn ct_submit_form(ctx: ComponentContext<()>, form: CtForm) -> Result<(), BoxError> {
    let comment = form.comment.unwrap_or_else(|| "(no comment)".to_owned());
    ctx.reply(format!("Form: name {}, comment {comment}.", form.name))
        .ephemeral()
        .await?;
    Ok(())
}

/// Picks a color with autocomplete.
#[derive(Command)]
#[command(name = "ct-color", description = "Test: autocomplete")]
struct CtColor {
    #[option(description = "Type a color", autocomplete)]
    name: String,
}

const COLORS: [&str; 6] = ["red", "orange", "yellow", "green", "blue", "violet"];

async fn ct_color_choices(_: CommandContext<()>, focused: Focused) -> Vec<CommandOptionChoice> {
    let typed = focused.value.to_lowercase();
    COLORS
        .iter()
        .filter(|color| color.starts_with(&typed))
        .map(|color| CommandOptionChoice::new(*color, *color))
        .collect()
}

async fn ct_color(ctx: CommandContext<()>, args: CtColor) -> Result<(), BoxError> {
    ctx.reply(format!("You picked {}.", args.name)).await?;
    Ok(())
}

/// Has a cooldown of 15 seconds for each user.
#[derive(Command)]
#[command(name = "ct-cooldown", description = "Test: a 15 second cooldown")]
struct CtCooldown;

async fn ct_cooldown(ctx: CommandContext<()>, _: CtCooldown) -> Result<(), BoxError> {
    ctx.reply("OK. Use the command again within 15 seconds to see the cooldown.")
        .ephemeral()
        .await?;
    Ok(())
}

/// Waits for the next message of the user in the channel.
#[derive(Command)]
#[command(name = "ct-wait", description = "Test: wait for your next message")]
struct CtWait;

async fn ct_wait(ctx: CommandContext<()>, _: CtWait) -> Result<(), BoxError> {
    let user_id = ctx.user_id();
    let channel_id = ctx.channel_id()?;
    ctx.reply("Send a message in this channel within 30 seconds.")
        .await?;

    let message = ctx
        .wait_for::<MessageCreate>(
            move |message| message.author.id == user_id && message.channel_id == channel_id,
            Duration::from_secs(30),
        )
        .await;
    let answer = match message {
        Some(message) => format!("Got your message (ID {}).", message.id),
        None => "No message within 30 seconds.".to_owned(),
    };
    ctx.reply(answer).await?;
    Ok(())
}

/// Fails on purpose, so the error response shows.
#[derive(Command)]
#[command(name = "ct-error", description = "Test: a handler error")]
struct CtError;

async fn ct_error(_: CommandContext<()>, _: CtError) -> Result<(), BoxError> {
    Err("this error is part of the test".into())
}

/// Shows a Components V2 message.
#[derive(Command)]
#[command(name = "ct-v2", description = "Test: a Components V2 message")]
struct CtV2;

async fn ct_v2(ctx: CommandContext<()>, _: CtV2) -> Result<(), BoxError> {
    let mut container = Container::new(vec![
        TextDisplay::new("## Test panel").into(),
        Separator::default().into(),
        TextDisplay::new("This is a Components V2 message.").into(),
    ]);
    container.accent_color = Some(Color::from_rgb(0x2E, 0xCC, 0x71));
    let message = InteractionMessage::new()
        .component(container)
        .components_v2();
    ctx.reply(message).await?;
    Ok(())
}

/// Shows details of a user.
#[derive(Command)]
#[command(name = "CT Info", kind = "user")]
struct CtInfo {
    target: ResolvedUser,
}

async fn ct_info(ctx: CommandContext<()>, args: CtInfo) -> Result<(), BoxError> {
    let nickname = args
        .target
        .member
        .and_then(|member| member.nick)
        .unwrap_or_else(|| "(no nickname)".to_owned());
    ctx.reply(format!(
        "User {} (ID {}), nickname {nickname}.",
        args.target.user.username, args.target.user.id
    ))
    .ephemeral()
    .await?;
    Ok(())
}

/// Quotes a message.
#[derive(Command)]
#[command(name = "CT Quote", kind = "message")]
struct CtQuote(Message);

async fn ct_quote(ctx: CommandContext<()>, CtQuote(message): CtQuote) -> Result<(), BoxError> {
    let length = message.content.chars().count();
    ctx.reply(format!(
        "Message {} by {} has {length} characters and {} attachment(s).",
        message.id,
        message.author.username,
        message.attachments.len()
    ))
    .ephemeral()
    .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let mut args = std::env::args().skip(1);
    let mode = args.next().ok_or("give a mode: run or clear")?;
    let guild_id: GuildId = args.next().ok_or("give a guild id")?.parse()?;
    let keep_file = args.next().ok_or("give the keep file")?;
    let keep = read_keep_file(&keep_file)?;

    let bot = Bot::builder(Token::from_env("DISCORD_TOKEN")?)
        .intents(Intents::GUILDS | Intents::GUILD_MESSAGES)
        .command_sync(CommandSync::Off)
        // `ct-wait` waits for MESSAGE_CREATE, but no handler uses it.
        .extra_events([EventType::MessageCreate])
        .slow_response(SlowResponse::Defer(Duration::from_secs(2)))
        .command(ct_ping)
        .command(ct_attach)
        .command(ct_defer)
        .command(ct_slow)
        .command(ct_panel)
        .command(ct_color)
        .autocomplete::<CtColor>(ct_color_choices)
        .command_with(ct_cooldown, |settings| {
            settings.cooldown(Cooldown::new(CooldownScope::User, Duration::from_secs(15)))
        })
        .command(ct_wait)
        .command(ct_error)
        .command(ct_v2)
        .command(ct_info)
        .command(ct_quote)
        .component("ct:count:{count}", ct_count)
        .component("ct:pick", ct_pick)
        .component("ct:form", ct_open_form)
        .modal("ct:form-submit", ct_submit_form)
        .build()?;

    let mut commands = keep;
    if mode == "run" || mode == "check" {
        let definitions = bot.command_definitions();
        commands.extend(definitions.into_values().flatten());
    } else if mode != "clear" {
        return Err(format!("unknown mode {mode}").into());
    }

    let http = bot.context().http().clone();
    let application_id = http.current_application().await?.id;
    let scope = SyncScope::Guild(guild_id);
    let options = SyncOptions::new().dry_run(mode == "check");
    let report = sync(&http, application_id, scope, &commands, &options).await?;
    for action in report.plan.actions() {
        tracing::info!(
            name = action.name(),
            change = action.is_change(),
            "sync action"
        );
    }
    if mode != "run" {
        return Ok(());
    }

    bot.run().await?;
    Ok(())
}

/// Reads the guild commands that must stay. A sync that leaves them out would delete them.
fn read_keep_file(path: &str) -> Result<Vec<SyncCommand>, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    let remote: Vec<cordial::model::command::Command> = serde_json::from_str(&text)?;
    Ok(remote
        .iter()
        .map(|command| {
            let mut definition = CommandDefinition::from(command);
            definition.id = None;
            SyncCommand::new(definition)
        })
        .collect())
}
