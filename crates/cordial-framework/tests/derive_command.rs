//! Tests of `#[derive(Command)]`: the definitions and the parsers.

use cordial_framework::command::{Mentionable, ParseErrorKind, ResolvedUser};
use cordial_framework::{Command, CommandOptions, ParseError};
use cordial_model::channel::Channel;
use cordial_model::command::{CommandOptionType, CommandType};
use cordial_model::guild::Role;
use cordial_model::interaction::CommandData;
use cordial_model::message::{Attachment, Message};
use serde_json::{Value, json};

/// Moderation command with all option types and limits.
#[derive(Command, Debug)]
#[command(
    name = "moderate",
    description = "Moderate a member",
    default_member_permissions = "BAN_MEMBERS | KICK_MEMBERS",
    contexts = "guild",
    integration_types = "guild_install"
)]
struct Moderate {
    #[option(description = "The member")]
    user: ResolvedUser,
    #[option(description = "The role to give")]
    role: Role,
    #[option(
        description = "The log channel",
        channel_types(GuildText, GuildAnnouncement)
    )]
    channel: Channel,
    #[option(description = "The evidence")]
    evidence: Attachment,
    #[option(description = "The reason", min_length = 3, max_length = 512)]
    reason: String,
    #[option(description = "The number of days", min_value = 0, max_value = 7)]
    days: i64,
    #[option(description = "The level")]
    level: u8,
    #[option(description = "The weight", min_value = -1.5, max_value = 2.5)]
    weight: f64,
    #[option(description = "Tells if the action is silent")]
    silent: bool,
    #[option(description = "The target", name = "target")]
    mentionable: Mentionable,
    #[option(description = "The kind of the action")]
    r#type: Option<String>,
    #[option(description = "The priority", min_value = 1)]
    priority: Option<u8>,
    #[option(description = "A note", autocomplete)]
    note: Option<String>,
}

/// A command with no options.
#[derive(Command, Debug, PartialEq)]
#[command(
    name = "ping",
    description = "Check the latency",
    nsfw,
    contexts = "guild, bot_dm, private_channel",
    integration_types = "guild_install, user_install"
)]
struct Ping;

/// A command for administrators only.
#[derive(Command, Debug)]
#[command(name = "reset", description = "Reset", default_member_permissions = "")]
struct Reset {}

#[derive(Command, Debug)]
#[command(name = "User Info", kind = "user", contexts = "guild")]
struct UserInfo {
    target: ResolvedUser,
}

#[derive(Command, Debug)]
#[command(name = "Quote message", kind = "message")]
struct QuoteMessage(Message);

/// A set of options only. It has no `#[command]` attribute.
#[derive(Command, Debug, PartialEq)]
struct Pagination {
    #[option(description = "The page")]
    page: Option<u32>,
}

fn definition_json<T: Command>() -> Value {
    serde_json::to_value(T::definition()).unwrap()
}

fn command_data(value: Value) -> CommandData {
    serde_json::from_value(value).unwrap()
}

#[test]
fn chat_input_definition() {
    assert_eq!(Moderate::NAME, "moderate");
    assert!(matches!(Moderate::KIND, CommandType::ChatInput));

    let expected = json!({
        "type": 1,
        "name": "moderate",
        "description": "Moderate a member",
        "default_member_permissions": "6",
        "contexts": [0],
        "integration_types": [0],
        "options": [
            { "type": 6, "name": "user", "description": "The member", "required": true },
            { "type": 8, "name": "role", "description": "The role to give", "required": true },
            {
                "type": 7,
                "name": "channel",
                "description": "The log channel",
                "required": true,
                "channel_types": [0, 5]
            },
            { "type": 11, "name": "evidence", "description": "The evidence", "required": true },
            {
                "type": 3,
                "name": "reason",
                "description": "The reason",
                "required": true,
                "min_length": 3,
                "max_length": 512
            },
            {
                "type": 4,
                "name": "days",
                "description": "The number of days",
                "required": true,
                "min_value": 0,
                "max_value": 7
            },
            {
                "type": 4,
                "name": "level",
                "description": "The level",
                "required": true,
                "min_value": 0,
                "max_value": 255
            },
            {
                "type": 10,
                "name": "weight",
                "description": "The weight",
                "required": true,
                "min_value": -1.5,
                "max_value": 2.5
            },
            {
                "type": 5,
                "name": "silent",
                "description": "Tells if the action is silent",
                "required": true
            },
            { "type": 9, "name": "target", "description": "The target", "required": true },
            { "type": 3, "name": "type", "description": "The kind of the action" },
            {
                "type": 4,
                "name": "priority",
                "description": "The priority",
                "min_value": 1,
                "max_value": 255
            },
            { "type": 3, "name": "note", "description": "A note", "autocomplete": true }
        ]
    });
    assert_eq!(definition_json::<Moderate>(), expected);
}

#[test]
fn definition_without_options() {
    let expected = json!({
        "type": 1,
        "name": "ping",
        "description": "Check the latency",
        "nsfw": true,
        "contexts": [0, 1, 2],
        "integration_types": [0, 1]
    });
    assert_eq!(definition_json::<Ping>(), expected);

    let expected = json!({
        "type": 1,
        "name": "reset",
        "description": "Reset",
        "default_member_permissions": "0"
    });
    assert_eq!(definition_json::<Reset>(), expected);
}

#[test]
fn context_menu_definitions() {
    assert_eq!(UserInfo::NAME, "User Info");
    assert!(matches!(UserInfo::KIND, CommandType::User));
    assert_eq!(
        definition_json::<UserInfo>(),
        json!({ "type": 2, "name": "User Info", "contexts": [0] })
    );

    assert!(matches!(QuoteMessage::KIND, CommandType::Message));
    assert_eq!(
        definition_json::<QuoteMessage>(),
        json!({ "type": 3, "name": "Quote message" })
    );
}

#[test]
fn options_without_command() {
    let options = serde_json::to_value(Pagination::options()).unwrap();
    let expected = json!([{
        "type": 4,
        "name": "page",
        "description": "The page",
        "min_value": 0,
        "max_value": 4_294_967_295_i64
    }]);
    assert_eq!(options, expected);
}

const USER_ID: &str = "200000000000000001";
const ROLE_ID: &str = "300000000000000001";
const CHANNEL_ID: &str = "400000000000000001";
const ATTACHMENT_ID: &str = "500000000000000001";
const MESSAGE_ID: &str = "600000000000000001";

fn resolved() -> Value {
    json!({
        "users": {
            USER_ID: {
                "id": USER_ID,
                "username": "alice",
                "discriminator": "0",
                "global_name": "Alice",
                "avatar": null,
                "public_flags": 0
            }
        },
        "members": {
            USER_ID: {
                "nick": "Ally",
                "avatar": null,
                "roles": [ROLE_ID],
                "joined_at": "2024-01-01T00:00:00.000000+00:00",
                "premium_since": null,
                "pending": false,
                "flags": 0,
                "permissions": "2248473465835073",
                "communication_disabled_until": null
            }
        },
        "roles": {
            ROLE_ID: {
                "id": ROLE_ID,
                "name": "Moderators",
                "color": 0,
                "hoist": false,
                "icon": null,
                "unicode_emoji": null,
                "position": 1,
                "permissions": "0",
                "managed": false,
                "mentionable": true,
                "flags": 0
            }
        },
        "channels": {
            CHANNEL_ID: {
                "id": CHANNEL_ID,
                "name": "mod-log",
                "type": 0,
                "parent_id": null,
                "permissions": "2248473465835073",
                "nsfw": false,
                "last_message_id": null,
                "flags": 0,
                "topic": null,
                "rate_limit_per_user": 0
            }
        },
        "attachments": {
            ATTACHMENT_ID: {
                "id": ATTACHMENT_ID,
                "filename": "evidence.png",
                "size": 2048,
                "url": "https://cdn.discordapp.com/ephemeral-attachments/1/2/evidence.png",
                "proxy_url": "https://media.discordapp.net/ephemeral-attachments/1/2/evidence.png",
                "width": 64,
                "height": 64,
                "content_type": "image/png",
                "ephemeral": true
            }
        }
    })
}

fn moderate_data(options: Value) -> CommandData {
    let mut data = json!({
        "id": "100000000000000001",
        "name": "moderate",
        "type": 1,
        "guild_id": "700000000000000001",
        "resolved": resolved()
    });
    data["options"] = options;
    command_data(data)
}

fn moderate_options() -> Vec<Value> {
    vec![
        json!({ "name": "user", "type": 6, "value": USER_ID }),
        json!({ "name": "role", "type": 8, "value": ROLE_ID }),
        json!({ "name": "channel", "type": 7, "value": CHANNEL_ID }),
        json!({ "name": "evidence", "type": 11, "value": ATTACHMENT_ID }),
        json!({ "name": "reason", "type": 3, "value": "spam" }),
        json!({ "name": "days", "type": 4, "value": 3 }),
        json!({ "name": "level", "type": 4, "value": 200 }),
        json!({ "name": "weight", "type": 10, "value": 0.75 }),
        json!({ "name": "silent", "type": 5, "value": true }),
        json!({ "name": "target", "type": 9, "value": ROLE_ID }),
        json!({ "name": "type", "type": 3, "value": "warn" }),
    ]
}

#[test]
fn parse_chat_input() {
    let data = moderate_data(Value::Array(moderate_options()));
    let command = <Moderate as Command>::parse(&data).unwrap();

    assert_eq!(command.user.user.username, "alice");
    let member = command.user.member.expect("the member is resolved");
    assert_eq!(member.nick.as_deref(), Some("Ally"));
    assert_eq!(command.role.name, "Moderators");
    assert_eq!(command.channel.name.as_deref(), Some("mod-log"));
    assert_eq!(command.evidence.filename, "evidence.png");
    assert_eq!(command.reason, "spam");
    assert_eq!(command.days, 3);
    assert_eq!(command.level, 200);
    assert!((command.weight - 0.75).abs() < f64::EPSILON);
    assert!(command.silent);
    assert!(matches!(command.mentionable, Mentionable::Role(role) if role.name == "Moderators"));
    assert_eq!(command.r#type.as_deref(), Some("warn"));
    assert_eq!(command.priority, None);
    assert_eq!(command.note, None);
}

#[test]
fn parse_mentionable_user() {
    let mut options = moderate_options();
    options[9] = json!({ "name": "target", "type": 9, "value": USER_ID });
    let data = moderate_data(Value::Array(options));

    let command = <Moderate as Command>::parse(&data).unwrap();
    assert!(
        matches!(command.mentionable, Mentionable::User(user) if user.user.username == "alice")
    );
}

fn parse_error(options: Vec<Value>) -> ParseError {
    let data = moderate_data(Value::Array(options));
    <Moderate as Command>::parse(&data).unwrap_err()
}

#[test]
fn parse_errors() {
    let mut options = moderate_options();
    options.remove(4);
    let error = parse_error(options);
    assert_eq!(error.path, ["reason"]);
    assert_eq!(error.kind, ParseErrorKind::Missing);

    let mut options = moderate_options();
    options[5] = json!({ "name": "days", "type": 3, "value": "three" });
    let error = parse_error(options);
    assert_eq!(error.path, ["days"]);
    assert_eq!(
        error.kind,
        ParseErrorKind::WrongType {
            expected: CommandOptionType::Integer
        }
    );

    let mut options = moderate_options();
    options[6] = json!({ "name": "level", "type": 4, "value": 300 });
    let error = parse_error(options);
    assert_eq!(error.path, ["level"]);
    assert_eq!(error.kind, ParseErrorKind::OutOfRange);

    let mut options = moderate_options();
    options[1] = json!({ "name": "role", "type": 8, "value": "300000000000000999" });
    let error = parse_error(options);
    assert_eq!(error.path, ["role"]);
    assert_eq!(error.kind, ParseErrorKind::Unresolved);
}

#[test]
fn parse_without_options() {
    let data = command_data(json!({ "id": "100000000000000002", "name": "ping", "type": 1 }));
    assert_eq!(<Ping as Command>::parse(&data).unwrap(), Ping);
}

#[test]
fn parse_user_command() {
    let data = command_data(json!({
        "id": "100000000000000003",
        "name": "User Info",
        "type": 2,
        "target_id": USER_ID,
        "resolved": resolved()
    }));
    let command = UserInfo::parse(&data).unwrap();
    assert_eq!(command.target.user.username, "alice");

    let data = command_data(json!({ "id": "100000000000000003", "name": "User Info", "type": 2 }));
    let error = UserInfo::parse(&data).unwrap_err();
    assert_eq!(error.kind, ParseErrorKind::MissingTarget);
}

#[test]
fn parse_message_command() {
    let author = resolved()["users"][USER_ID].clone();
    let data = command_data(json!({
        "id": "100000000000000004",
        "name": "Quote message",
        "type": 3,
        "target_id": MESSAGE_ID,
        "resolved": {
            "messages": {
                MESSAGE_ID: {
                    "id": MESSAGE_ID,
                    "type": 0,
                    "channel_id": CHANNEL_ID,
                    "author": author,
                    "content": "Hello there",
                    "timestamp": "2024-05-01T12:00:00.000000+00:00",
                    "edited_timestamp": null,
                    "tts": false,
                    "mention_everyone": false,
                    "mentions": [],
                    "mention_roles": [],
                    "attachments": [],
                    "embeds": [],
                    "pinned": false,
                    "flags": 0,
                    "components": []
                }
            }
        }
    }));
    let QuoteMessage(message) = QuoteMessage::parse(&data).unwrap();
    assert_eq!(message.content, "Hello there");
}

#[test]
fn parse_options_only() {
    let data = command_data(json!({
        "id": "100000000000000005",
        "name": "list",
        "type": 1,
        "options": [{ "name": "page", "type": 4, "value": 2 }]
    }));
    let input = cordial_framework::command::OptionsInput::from_data(&data);
    assert_eq!(
        Pagination::parse(input).unwrap(),
        Pagination { page: Some(2) }
    );
}
