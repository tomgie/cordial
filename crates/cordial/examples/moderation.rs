//! A moderation bot: `/ban`, `/timeout`, and `/warn add|list|clear`.
//!
//! It shows options, choices, a command group, permission defaults, checks, a cooldown, and
//! typed state. Run it with `DISCORD_TOKEN=... cargo run --example moderation`.

use std::collections::HashMap;
use std::time::Duration;

use cordial::framework::{
    Bot, BoxError, Check, Choice, Command, CommandContext, CommandGroup, Cooldown, CooldownScope,
    command::ResolvedUser,
};
use cordial::http::Token;
use cordial::model::Permissions;
use cordial::model::Timestamp;
use cordial::model::id::{GuildId, UserId};
use tokio::sync::Mutex;

/// The bot state: the warnings of each member.
#[derive(Default)]
struct State {
    warnings: Mutex<HashMap<(GuildId, UserId), Vec<String>>>,
}

type Ctx = CommandContext<State>;

// ANCHOR: choice
/// How many messages of the member to delete.
#[derive(Choice)]
enum DeleteMessages {
    #[choice(name = "None", value = 0)]
    None,
    #[choice(name = "Last hour", value = 3600)]
    Hour,
    #[choice(name = "Last day", value = 86400)]
    Day,
    #[choice(name = "Last week", value = 604_800)]
    Week,
}
// ANCHOR_END: choice

// ANCHOR: ban
/// Bans a member.
#[derive(Command)]
#[command(
    name = "ban",
    description = "Ban a member",
    default_member_permissions = "BAN_MEMBERS",
    contexts = "guild"
)]
struct Ban {
    #[option(description = "The member to ban")]
    user: ResolvedUser,
    #[option(description = "The reason, for the audit log", max_length = 512)]
    reason: Option<String>,
    #[option(description = "Which messages to delete")]
    delete_messages: Option<DeleteMessages>,
}

async fn ban(ctx: Ctx, args: Ban) -> Result<(), BoxError> {
    let guild_id = ctx.guild_id()?;
    let seconds = args
        .delete_messages
        .map_or(0, |choice| choice.choice_value());

    let mut request = ctx
        .http()
        .ban_member(guild_id, args.user.user.id)
        .delete_message_seconds(u32::try_from(seconds)?);
    if let Some(reason) = &args.reason {
        request = request.reason(reason.clone());
    }
    request.await?;

    ctx.reply(format!("{} is banned.", args.user.user.username))
        .ephemeral()
        .await?;
    Ok(())
}
// ANCHOR_END: ban

/// Times out a member.
#[derive(Command)]
#[command(
    name = "timeout",
    description = "Time out a member",
    default_member_permissions = "MODERATE_MEMBERS",
    contexts = "guild"
)]
struct Timeout {
    #[option(description = "The member")]
    user: ResolvedUser,
    #[option(
        description = "The length in minutes",
        min_value = 1,
        max_value = 40320
    )]
    minutes: u16,
}

async fn timeout(ctx: Ctx, args: Timeout) -> Result<(), BoxError> {
    let guild_id = ctx.guild_id()?;
    let until = Timestamp::now() + Duration::from_secs(u64::from(args.minutes) * 60);

    ctx.http()
        .edit_guild_member(guild_id, args.user.user.id)
        .communication_disabled_until(until)
        .reason(format!("Timeout by {}", ctx.user().username))
        .await?;

    ctx.reply(format!(
        "{} is timed out for {} minutes.",
        args.user.user.username, args.minutes
    ))
    .await?;
    Ok(())
}

/// The options of `/warn add`.
#[derive(Command)]
struct WarnAdd {
    #[option(description = "The member")]
    user: ResolvedUser,
    #[option(description = "The reason", max_length = 200)]
    reason: String,
}

/// The options of `/warn list` and `/warn clear`.
#[derive(Command)]
struct WarnTarget {
    #[option(description = "The member")]
    user: ResolvedUser,
}

/// Manages warnings.
#[derive(CommandGroup)]
#[command(
    name = "warn",
    description = "Manage warnings",
    default_member_permissions = "MODERATE_MEMBERS",
    contexts = "guild"
)]
enum Warn {
    #[subcommand(description = "Warn a member")]
    Add(WarnAdd),
    #[subcommand(description = "Show the warnings of a member")]
    List(WarnTarget),
    #[subcommand(description = "Remove all warnings of a member")]
    Clear(WarnTarget),
}

async fn warn(ctx: Ctx, args: Warn) -> Result<(), BoxError> {
    let guild_id = ctx.guild_id()?;
    let mut warnings = ctx.state().warnings.lock().await;

    let message = match args {
        Warn::Add(add) => {
            let list = warnings.entry((guild_id, add.user.user.id)).or_default();
            list.push(add.reason);
            format!("{} has {} warnings.", add.user.user.username, list.len())
        }
        Warn::List(target) => match warnings.get(&(guild_id, target.user.user.id)) {
            Some(list) if !list.is_empty() => list
                .iter()
                .enumerate()
                .map(|(index, reason)| format!("{}. {reason}", index + 1))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => format!("{} has no warnings.", target.user.user.username),
        },
        Warn::Clear(target) => {
            warnings.remove(&(guild_id, target.user.user.id));
            format!("The warnings of {} are removed.", target.user.user.username)
        }
    };
    drop(warnings);

    ctx.reply(message).ephemeral().await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    // ANCHOR: builder
    let bot = Bot::builder(Token::from_env("DISCORD_TOKEN")?)
        .state(State::default())
        // Discord hides the command from members without the permission. The check is a
        // second guard, for servers that changed the command permissions.
        .command_with(ban, |settings| {
            settings.check(Check::permissions(Permissions::BAN_MEMBERS))
        })
        .command_with(timeout, |settings| {
            settings.cooldown(Cooldown::new(CooldownScope::User, Duration::from_secs(5)))
        })
        .command(warn)
        .build()?;
    // ANCHOR_END: builder

    bot.run().await?;
    Ok(())
}
