use std::time::Duration;

use cordial::framework::command::{Mentionable, ResolvedUser};
use cordial::framework::localize::LocalizationKey;
use cordial::framework::sync::{SyncOptions, SyncStrategy};
use cordial::framework::{
    Bot, BoxError, Check, CheckFailure, Choice, Command, CommandContext, CommandGroup,
    CommandSync, Cooldown, CooldownScope, Error, Focused, InteractionContext,
};
use cordial::http::Token;
use cordial::model::channel::Channel;
use cordial::model::command::CommandOptionChoice;
use cordial::model::id::{GuildId, UserId};
use cordial::model::locale::{Locale, Localizations};
use cordial::model::message::Attachment;

// ANCHOR: options
/// Sets a reminder.
#[derive(Command)]
#[command(name = "remind", description = "Set a reminder")]
struct Remind {
    #[option(description = "What to remind you of", min_length = 1, max_length = 200)]
    text: String,
    #[option(description = "Minutes from now", min_value = 1, max_value = 1440)]
    minutes: u16,
    #[option(description = "The unit of the time")]
    unit: Option<Unit>,
    #[option(
        description = "The channel for the reminder",
        channel_types(GuildText, GuildAnnouncement)
    )]
    channel: Option<Channel>,
    #[option(description = "Mention this user or role too")]
    mention: Option<Mentionable>,
    #[option(description = "A file to attach")]
    file: Option<Attachment>,
    #[option(description = "Send it as a direct message")]
    private: Option<bool>,
}

/// A fixed list of values. The values are strings: "minutes" and "hours".
#[derive(Choice)]
enum Unit {
    Minutes,
    #[choice(name = "Hours (long reminders)")]
    Hours,
}

async fn remind(ctx: CommandContext<()>, args: Remind) -> Result<(), BoxError> {
    let minutes = match args.unit.unwrap_or(Unit::Minutes) {
        Unit::Minutes => u64::from(args.minutes),
        Unit::Hours => u64::from(args.minutes) * 60,
    };
    ctx.reply(format!("I remind you in {minutes} minutes: {}", args.text))
        .ephemeral()
        .await?;
    Ok(())
}
// ANCHOR_END: options

// ANCHOR: group
/// `/settings show`, `/settings reset`, and `/settings role add|remove`.
#[derive(CommandGroup)]
#[command(
    name = "settings",
    description = "Change the bot settings",
    default_member_permissions = "MANAGE_GUILD",
    contexts = "guild"
)]
enum Settings {
    #[subcommand(description = "Show the settings")]
    Show,
    #[subcommand(description = "Reset all settings")]
    Reset,
    #[group(description = "Manage the roles of the bot")]
    Role(RoleSettings),
}

/// A subcommand group. It has no `#[command]` attribute, because it is not a top-level
/// command.
#[derive(CommandGroup)]
enum RoleSettings {
    #[subcommand(description = "Add a role")]
    Add(RoleTarget),
    #[subcommand(description = "Remove a role")]
    Remove(RoleTarget),
}

/// The options of `/settings role add` and `/settings role remove`.
#[derive(Command)]
struct RoleTarget {
    #[option(description = "The role")]
    role: cordial::model::guild::Role,
}

async fn settings(ctx: CommandContext<()>, args: Settings) -> Result<(), BoxError> {
    let text = match args {
        Settings::Show => "No settings yet.".to_owned(),
        Settings::Reset => "The settings are reset.".to_owned(),
        Settings::Role(RoleSettings::Add(target)) => format!("Added {}.", target.role.name),
        Settings::Role(RoleSettings::Remove(target)) => format!("Removed {}.", target.role.name),
    };
    ctx.reply(text).ephemeral().await?;
    Ok(())
}
// ANCHOR_END: group

// ANCHOR: checks
const OWNER: UserId = UserId::new_const(1234);

/// A check is an async function. An error stops the command and the user sees the message.
fn in_business_hours() -> Check<()> {
    Check::new(|_ctx: InteractionContext<()>| async move {
        let hour = cordial::model::Timestamp::now().unix_seconds() / 3600 % 24;
        if (8..18).contains(&hour) {
            Ok(())
        } else {
            Err(CheckFailure::new("Use this command from 08:00 to 18:00 UTC."))
        }
    })
}

/// Shows details about a user.
#[derive(Command)]
#[command(name = "whois", description = "Show details about a user")]
struct Whois {
    #[option(description = "The user")]
    user: ResolvedUser,
}

async fn whois(ctx: CommandContext<()>, args: Whois) -> Result<(), BoxError> {
    let nickname = args
        .user
        .member
        .and_then(|member| member.nick)
        .unwrap_or_else(|| "(no nickname)".to_owned());
    ctx.reply(format!("{}: {nickname}", args.user.user.username))
        .await?;
    Ok(())
}

fn with_checks(token: Token) -> Result<Bot<()>, Error> {
    Bot::builder(token)
        .command_with(whois, |settings| {
            settings
                .check(Check::users([OWNER]))
                .check(in_business_hours())
                .cooldown(Cooldown::new(CooldownScope::User, Duration::from_secs(10)))
        })
        .build()
}
// ANCHOR_END: checks

// ANCHOR: autocomplete
/// Picks a color.
#[derive(Command)]
#[command(name = "color", description = "Pick a color")]
struct PickColor {
    #[option(description = "The color name", autocomplete)]
    name: String,
}

const COLORS: [&str; 6] = ["red", "orange", "yellow", "green", "blue", "violet"];

/// Gives the choices that start with the text that the user typed so far.
async fn color_choices(_: CommandContext<()>, focused: Focused) -> Vec<CommandOptionChoice> {
    let typed = focused.value.to_lowercase();
    COLORS
        .iter()
        .filter(|color| color.starts_with(&typed))
        .map(|color| CommandOptionChoice::new(*color, *color))
        .collect()
}

async fn pick_color(ctx: CommandContext<()>, args: PickColor) -> Result<(), BoxError> {
    ctx.reply(format!("You picked {}.", args.name)).await?;
    Ok(())
}

fn with_autocomplete(token: Token) -> Result<Bot<()>, Error> {
    Bot::builder(token)
        .command(pick_color)
        // The command type cannot come from the handler, so name it.
        .autocomplete::<PickColor, _, _>(color_choices)
        .build()
}
// ANCHOR_END: autocomplete

// ANCHOR: localize
/// Gives German texts by the dotted key, for example `color.options.name.description`.
fn german(key: &LocalizationKey<'_>) -> Localizations {
    let text = match key.dotted().as_str() {
        "color.name" => "farbe",
        "color.description" => "Wähle eine Farbe",
        "color.options.name.name" => "name",
        "color.options.name.description" => "Der Name der Farbe",
        _ => return Localizations::new(),
    };
    Localizations::from([(Locale::German, text.to_owned())])
}

fn with_localizer(token: Token) -> Result<Bot<()>, Error> {
    Bot::builder(token)
        .localizer(german)
        .command(pick_color)
        .build()
}
// ANCHOR_END: localize

// ANCHOR: sync
const TEST_GUILD: GuildId = GuildId::new_const(5678);

fn with_sync_settings(token: Token) -> Result<Bot<()>, Error> {
    let options = SyncOptions::new()
        .strategy(SyncStrategy::Auto { threshold: 5 })
        .dry_run(true);
    Bot::builder(token)
        .command_sync(CommandSync::OnStart(options))
        // The command was `/colour` before. The sync renames it and keeps its ID.
        .command_with(pick_color, |settings| {
            settings.previous_name("colour").guilds([TEST_GUILD])
        })
        .build()
}
// ANCHOR_END: sync
