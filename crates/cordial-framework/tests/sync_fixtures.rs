//! Tests of the sync normal form against real responses from Discord.
//!
//! The fixtures are command lists from `GET .../commands?with_localizations=true`, captured
//! on 2026-11-06. Placeholder IDs replace the application ID and the guild ID.
//! - `guild_commands.json`: the `apply` set of `crates/cordial/examples/live_sync.rs`,
//!   registered in a guild. [`guild_definitions`] repeats that set.
//! - `global_commands.json`: global commands of a bot that another library registered.

use cordial_framework::sync::{Action, SyncCommand, SyncOptions, plan};
use cordial_model::channel::ChannelType;
use cordial_model::command::{
    Command, CommandDefinition, CommandOption, CommandOptionType, NumberLimit,
};
use cordial_model::interaction::InteractionContextType;
use cordial_model::{Locale, Permissions};
use serde_json::Value;

const GUILD_COMMANDS: &str = include_str!("fixtures/guild_commands.json");
const GLOBAL_COMMANDS: &str = include_str!("fixtures/global_commands.json");

fn remote(fixture: &str) -> Vec<Command> {
    serde_json::from_str(fixture).expect("the fixture is a list of commands")
}

fn changes(local: &[SyncCommand], remote: &[Command]) -> Vec<Action> {
    let plan = plan(local, remote, &SyncOptions::new()).expect("the local names are unique");
    plan.changes().cloned().collect()
}

#[test]
fn guild_fixture_matches_the_definitions_that_made_it() {
    let remote = remote(GUILD_COMMANDS);

    let changes = changes(&guild_definitions(), &remote);

    assert!(changes.is_empty(), "unexpected changes: {changes:#?}");
}

/// Discord drops `contexts` from a guild command. Before the fix, each sync updated it.
#[test]
fn guild_command_ignores_contexts() {
    let remote = remote(GUILD_COMMANDS);
    let with_contexts = remote
        .iter()
        .find(|command| command.name == "ct-contexts")
        .expect("the fixture has the command");
    assert!(with_contexts.contexts.is_none());

    let local = [SyncCommand::new(
        CommandDefinition::chat_input("ct-contexts", "A command with contexts")
            .contexts([InteractionContextType::BotDm]),
    )];
    let plan = plan(&local, &remote, &SyncOptions::new()).unwrap();

    let action = plan
        .actions()
        .iter()
        .find(|action| action.name() == "ct-contexts")
        .unwrap();
    assert!(!action.is_change());
}

#[test]
fn guild_fixture_finds_a_changed_description() {
    let remote = remote(GUILD_COMMANDS);
    let mut local = guild_definitions();
    local[0].definition.description = "A basic test command, changed".to_owned();

    let changes = changes(&local, &remote);

    assert_eq!(changes.len(), 1);
    assert!(
        matches!(&changes[0], Action::Update { definition, .. } if definition.name == "ct-basic")
    );
}

/// A local definition does not repeat the defaults: no null localizations, no `false` flags,
/// no Discord fields. Each real command must equal its short form.
#[test]
fn global_fixture_equals_its_short_form() {
    let remote = remote(GLOBAL_COMMANDS);
    let raw: Vec<Value> = serde_json::from_str(GLOBAL_COMMANDS).unwrap();
    let local: Vec<SyncCommand> = raw.into_iter().map(short_form).collect();

    let changes = changes(&local, &remote);

    assert!(changes.is_empty(), "unexpected changes: {changes:#?}");
}

#[test]
fn global_fixture_finds_a_changed_option() {
    let remote = remote(GLOBAL_COMMANDS);
    let raw: Vec<Value> = serde_json::from_str(GLOBAL_COMMANDS).unwrap();
    let mut local: Vec<SyncCommand> = raw.into_iter().map(short_form).collect();
    let option = local[0]
        .definition
        .options
        .first_mut()
        .expect("the first command has options");
    option.description.push_str(" (changed)");

    let changes = changes(&local, &remote);

    assert_eq!(changes.len(), 1);
}

/// Makes the definition that a user writes for a remote command.
fn short_form(mut command: Value) -> SyncCommand {
    let object = command.as_object_mut().unwrap();
    for field in [
        "id",
        "application_id",
        "guild_id",
        "version",
        "dm_permission",
    ] {
        object.remove(field);
    }
    remove_defaults(&mut command);

    SyncCommand::new(serde_json::from_value(command).expect("the short form is a definition"))
}

fn remove_defaults(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|key, item| {
                // `default_member_permissions: null` means "everyone"; keep that value.
                let default = item.is_null() && key != "default_member_permissions";
                !default && *item != Value::Bool(false)
            });
            map.values_mut().for_each(remove_defaults);
        }
        Value::Array(items) => items.iter_mut().for_each(remove_defaults),
        _ => {}
    }
}

/// The `apply` set of `crates/cordial/examples/live_sync.rs`.
fn guild_definitions() -> Vec<SyncCommand> {
    let basic = CommandDefinition::chat_input("ct-basic", "A basic test command")
        .option(CommandOption::new(CommandOptionType::String, "text", "Some text").required(true));

    let mut word = CommandOption::new(CommandOptionType::String, "word", "A word")
        .choice("alpha", "a")
        .choice("beta", "b");
    word.choices.as_mut().unwrap()[0].name_localizations =
        Some([(Locale::German, "Alpha".to_owned())].into());
    let mut name = CommandOption::new(CommandOptionType::String, "name", "A name");
    name.min_length = Some(2);
    name.max_length = Some(20);
    let mut count = CommandOption::new(CommandOptionType::Integer, "count", "A count");
    count.min_value = Some(NumberLimit::Integer(1));
    count.max_value = Some(NumberLimit::Integer(100));
    let mut ratio = CommandOption::new(CommandOptionType::Number, "ratio", "A ratio");
    ratio.min_value = Some(NumberLimit::Number(0.5));
    ratio.max_value = Some(NumberLimit::Number(10.0));
    let mut channel = CommandOption::new(CommandOptionType::Channel, "channel", "A channel");
    channel.channel_types = Some(vec![ChannelType::GuildText, ChannelType::GuildAnnouncement]);

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
        .option(word)
        .option(name)
        .option(count)
        .option(ratio)
        .option(channel)
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

    vec![
        basic.into(),
        rich.into(),
        group.into(),
        CommandDefinition::chat_input("ct-contexts", "A command with contexts")
            .contexts([InteractionContextType::Guild])
            .into(),
        CommandDefinition::chat_input("ct-admin", "For administrators")
            .default_member_permissions(Permissions::empty())
            .into(),
        CommandDefinition::chat_input("ct-everyone", "For everyone")
            .everyone_can_use()
            .into(),
        CommandDefinition::user("CT Profile").into(),
        CommandDefinition::message("CT Quote").into(),
    ]
}
