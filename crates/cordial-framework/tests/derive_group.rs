//! Tests of `#[derive(CommandGroup)]`: subcommands, groups, and unit subcommands.

use cordial_framework::command::{OptionsInput, ParseErrorKind};
use cordial_framework::{Command, CommandGroup, CommandOptions};
use cordial_model::interaction::CommandData;
use serde_json::{Value, json};

#[derive(Command, Debug, PartialEq)]
struct Add {
    #[option(description = "The tag name")]
    name: String,
    #[option(description = "The tag text", max_length = 2000)]
    content: Option<String>,
}

#[derive(Command, Debug, PartialEq)]
struct Remove {
    #[option(description = "The tag name", autocomplete)]
    name: String,
}

#[derive(CommandGroup, Debug, PartialEq)]
enum Settings {
    #[subcommand(description = "Show the settings")]
    Show,
    #[subcommand(name = "set-limit", description = "Set the tag limit")]
    Limit(Limit),
}

#[derive(Command, Debug, PartialEq)]
struct Limit {
    #[option(
        description = "The maximum number of tags",
        min_value = 1,
        max_value = 100
    )]
    value: i64,
}

#[derive(CommandGroup, Debug, PartialEq)]
#[command(
    name = "tag",
    description = "Manage tags",
    default_member_permissions = "MANAGE_GUILD",
    contexts = "guild"
)]
enum Tag {
    #[subcommand(description = "Add a tag")]
    Add(Add),
    #[subcommand(description = "Remove a tag")]
    Remove(Remove),
    #[subcommand(description = "Show all tags")]
    ShowAll,
    #[group(description = "Change the settings")]
    Settings(Settings),
}

/// A group without `#[command]`. It gives only the subcommand options.
#[derive(CommandGroup, Debug, PartialEq)]
enum Unit {
    #[subcommand(description = "Start")]
    Start,
    #[subcommand(description = "Stop")]
    Stop,
}

fn tag_data(options: Value) -> CommandData {
    let mut data = json!({
        "id": "100000000000000001",
        "name": "tag",
        "type": 1,
        "guild_id": "700000000000000001"
    });
    data["options"] = options;
    serde_json::from_value(data).unwrap()
}

#[test]
fn group_definition() {
    assert_eq!(Tag::NAME, "tag");

    let expected = json!({
        "type": 1,
        "name": "tag",
        "description": "Manage tags",
        "default_member_permissions": "32",
        "contexts": [0],
        "options": [
            {
                "type": 1,
                "name": "add",
                "description": "Add a tag",
                "options": [
                    { "type": 3, "name": "name", "description": "The tag name", "required": true },
                    {
                        "type": 3,
                        "name": "content",
                        "description": "The tag text",
                        "max_length": 2000
                    }
                ]
            },
            {
                "type": 1,
                "name": "remove",
                "description": "Remove a tag",
                "options": [{
                    "type": 3,
                    "name": "name",
                    "description": "The tag name",
                    "required": true,
                    "autocomplete": true
                }]
            },
            { "type": 1, "name": "show-all", "description": "Show all tags" },
            {
                "type": 2,
                "name": "settings",
                "description": "Change the settings",
                "options": [
                    { "type": 1, "name": "show", "description": "Show the settings" },
                    {
                        "type": 1,
                        "name": "set-limit",
                        "description": "Set the tag limit",
                        "options": [{
                            "type": 4,
                            "name": "value",
                            "description": "The maximum number of tags",
                            "required": true,
                            "min_value": 1,
                            "max_value": 100
                        }]
                    }
                ]
            }
        ]
    });
    assert_eq!(serde_json::to_value(Tag::definition()).unwrap(), expected);
}

#[test]
fn unit_group_options() {
    let expected = json!([
        { "type": 1, "name": "start", "description": "Start" },
        { "type": 1, "name": "stop", "description": "Stop" }
    ]);
    assert_eq!(serde_json::to_value(Unit::options()).unwrap(), expected);
}

#[test]
fn parse_subcommands() {
    let data = tag_data(json!([{
        "name": "add",
        "type": 1,
        "options": [
            { "name": "name", "type": 3, "value": "rules" },
            { "name": "content", "type": 3, "value": "Be nice." }
        ]
    }]));
    let expected = Tag::Add(Add {
        name: "rules".to_owned(),
        content: Some("Be nice.".to_owned()),
    });
    assert_eq!(<Tag as Command>::parse(&data).unwrap(), expected);

    let data = tag_data(json!([{ "name": "show-all", "type": 1 }]));
    assert_eq!(<Tag as Command>::parse(&data).unwrap(), Tag::ShowAll);

    let data = tag_data(json!([{
        "name": "settings",
        "type": 2,
        "options": [{
            "name": "set-limit",
            "type": 1,
            "options": [{ "name": "value", "type": 4, "value": 10 }]
        }]
    }]));
    let expected = Tag::Settings(Settings::Limit(Limit { value: 10 }));
    assert_eq!(<Tag as Command>::parse(&data).unwrap(), expected);

    let data = tag_data(json!([{
        "name": "settings",
        "type": 2,
        "options": [{ "name": "show", "type": 1 }]
    }]));
    let expected = Tag::Settings(Settings::Show);
    assert_eq!(<Tag as Command>::parse(&data).unwrap(), expected);
}

#[test]
fn parse_errors() {
    let data = tag_data(json!([{ "name": "rename", "type": 1 }]));
    let error = <Tag as Command>::parse(&data).unwrap_err();
    assert!(error.path.is_empty());
    assert_eq!(
        error.kind,
        ParseErrorKind::UnknownSubcommand("rename".to_owned())
    );

    let data = tag_data(json!([]));
    let error = <Tag as Command>::parse(&data).unwrap_err();
    assert_eq!(error.kind, ParseErrorKind::MissingSubcommand);

    let data = tag_data(json!([{
        "name": "settings",
        "type": 2,
        "options": [{ "name": "set-limit", "type": 1, "options": [] }]
    }]));
    let error = <Tag as Command>::parse(&data).unwrap_err();
    assert_eq!(error.path, ["settings", "set-limit", "value"]);
    assert_eq!(error.kind, ParseErrorKind::Missing);

    let data = tag_data(json!([{
        "name": "settings",
        "type": 2,
        "options": [{ "name": "reset", "type": 1 }]
    }]));
    let error = <Tag as Command>::parse(&data).unwrap_err();
    assert_eq!(error.path, ["settings"]);
    assert_eq!(
        error.kind,
        ParseErrorKind::UnknownSubcommand("reset".to_owned())
    );
}

#[test]
fn parse_unit_group() {
    let data = tag_data(json!([{ "name": "stop", "type": 1 }]));
    let input = OptionsInput::from_data(&data);
    assert_eq!(Unit::parse(input).unwrap(), Unit::Stop);
}
