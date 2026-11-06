//! A live test of the command sync in one guild. It changes only the guild commands of that
//! guild. Global commands stay untouched.
//!
//! Run it with `DISCORD_TOKEN=... cargo run --example live_sync -- <guild id> <mode>`.
//!
//! | Mode | What it does |
//! |---|---|
//! | `apply` | Creates the test commands with the granular strategy. |
//! | `modify` | Changes one description and renames one command (granular). |
//! | `bulk` | Goes back to the `apply` set with one bulk overwrite. |
//! | `clear` | Deletes all guild commands of the application in the guild. |
//!
//! After each change, a second sync in dry-run mode must find no changes. Else the program
//! fails, because the normal form of the sync does not match the response of Discord.

#![expect(clippy::print_stdout, reason = "the program prints its report")]

use cordial::framework::sync::{SyncCommand, SyncOptions, SyncScope, SyncStrategy, sync};
use cordial::http::{Client, Token};
use cordial::model::Locale;
use cordial::model::Permissions;
use cordial::model::channel::ChannelType;
use cordial::model::command::{CommandDefinition, CommandOption, CommandOptionType, NumberLimit};
use cordial::model::id::GuildId;
use cordial::model::interaction::InteractionContextType;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let mut args = std::env::args().skip(1);
    let guild_id: GuildId = args.next().ok_or("give a guild id")?.parse()?;
    let mode = args
        .next()
        .ok_or("give a mode: apply, modify, bulk, or clear")?;

    let (commands, strategy) = match mode.as_str() {
        "apply" => (test_commands(false), SyncStrategy::Granular),
        "modify" => (test_commands(true), SyncStrategy::Granular),
        "bulk" => (test_commands(false), SyncStrategy::Bulk),
        "clear" => (Vec::new(), SyncStrategy::Granular),
        _ => return Err(format!("unknown mode {mode}").into()),
    };

    let http = Client::new(Token::from_env("DISCORD_TOKEN")?);
    let application_id = http.current_application().await?.id;
    let scope = SyncScope::Guild(guild_id);

    let options = SyncOptions::new().strategy(strategy);
    let report = sync(&http, application_id, scope, &commands, &options).await?;
    println!("{mode}: {} ({:?})", report.plan, report.outcome);
    for action in report.plan.changes() {
        println!("  change: {action:?}");
    }

    let check = sync(
        &http,
        application_id,
        scope,
        &commands,
        &options.dry_run(true),
    )
    .await?;
    println!("check: {}", check.plan);
    for action in check.plan.changes() {
        println!("  unexpected change: {action:?}");
    }
    if check.plan.has_changes() {
        return Err("the second sync found changes".into());
    }
    Ok(())
}

/// The test commands. `modified` changes one description and renames one command.
fn test_commands(modified: bool) -> Vec<SyncCommand> {
    let basic_description = if modified {
        "A basic test command, changed"
    } else {
        "A basic test command"
    };
    let basic = CommandDefinition::chat_input("ct-basic", basic_description)
        .option(CommandOption::new(CommandOptionType::String, "text", "Some text").required(true));

    let rich = CommandDefinition::chat_input("ct-rich", "A command with all option kinds")
        .localize(
            Locale::German,
            "ct-reich",
            "Ein Befehl mit allen Optionsarten",
        )
        .localize(
            Locale::French,
            "ct-riche",
            "Une commande avec tous les types",
        )
        .default_member_permissions(Permissions::MANAGE_GUILD)
        .nsfw(true)
        .option(word_choices())
        .option(limited_text())
        .option(limited_integer())
        .option(limited_number())
        .option(text_channel())
        .option(CommandOption::new(
            CommandOptionType::Boolean,
            "flag",
            "A flag",
        ))
        .option(CommandOption::new(
            CommandOptionType::User,
            "user",
            "A user",
        ))
        .option(CommandOption::new(
            CommandOptionType::Role,
            "role",
            "A role",
        ))
        .option(CommandOption::new(
            CommandOptionType::Mentionable,
            "target",
            "A target",
        ))
        .option(CommandOption::new(
            CommandOptionType::Attachment,
            "file",
            "A file",
        ))
        .option(
            CommandOption::new(CommandOptionType::String, "search", "A search").autocomplete(true),
        );

    let group = CommandDefinition::chat_input("ct-group", "A command with a group").option(
        CommandOption::new(CommandOptionType::SubCommandGroup, "config", "Settings")
            .option(CommandOption::new(
                CommandOptionType::SubCommand,
                "show",
                "Show",
            ))
            .option(
                CommandOption::new(CommandOptionType::SubCommand, "set", "Set a value").option(
                    CommandOption::new(CommandOptionType::Integer, "value", "The value")
                        .required(true)
                        .choice("one", 1)
                        .choice("two", 2),
                ),
            ),
    );

    let contexts = CommandDefinition::chat_input("ct-contexts", "A command with contexts")
        .contexts([InteractionContextType::Guild]);
    let admin = CommandDefinition::chat_input("ct-admin", "For administrators")
        .default_member_permissions(Permissions::empty());
    let everyone = CommandDefinition::chat_input("ct-everyone", "For everyone").everyone_can_use();

    let renamed = if modified {
        SyncCommand::new(CommandDefinition::user("CT Profile 2")).previous_name("CT Profile")
    } else {
        SyncCommand::new(CommandDefinition::user("CT Profile")).previous_name("CT Profile 2")
    };

    vec![
        basic.into(),
        rich.into(),
        group.into(),
        contexts.into(),
        admin.into(),
        everyone.into(),
        renamed,
        CommandDefinition::message("CT Quote").into(),
    ]
}

fn limited_text() -> CommandOption {
    let mut option = CommandOption::new(CommandOptionType::String, "name", "A name");
    option.min_length = Some(2);
    option.max_length = Some(20);
    option
}

fn word_choices() -> CommandOption {
    let mut option = CommandOption::new(CommandOptionType::String, "word", "A word")
        .choice("alpha", "a")
        .choice("beta", "b");
    let choice = &mut option.choices.as_mut().expect("the option has choices")[0];
    choice.name_localizations = Some([(Locale::German, "Alpha".to_owned())].into());
    option
}

fn limited_integer() -> CommandOption {
    let mut option = CommandOption::new(CommandOptionType::Integer, "count", "A count");
    option.min_value = Some(NumberLimit::Integer(1));
    option.max_value = Some(NumberLimit::Integer(100));
    option
}

fn limited_number() -> CommandOption {
    let mut option = CommandOption::new(CommandOptionType::Number, "ratio", "A ratio");
    option.min_value = Some(NumberLimit::Number(0.5));
    option.max_value = Some(NumberLimit::Number(10.0));
    option
}

fn text_channel() -> CommandOption {
    let mut option = CommandOption::new(CommandOptionType::Channel, "channel", "A channel");
    option.channel_types = Some(vec![ChannelType::GuildText, ChannelType::GuildAnnouncement]);
    option
}
