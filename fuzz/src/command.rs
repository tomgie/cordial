//! The framework: command option parsing and `custom_id` patterns.

use std::fmt::Display;

use cordial_framework::command::{Mentionable, OptionsInput, ResolvedUser};
use cordial_framework::{Command, CommandArg, CommandTarget, Pattern};
use cordial_model::channel::Channel;
use cordial_model::guild::Role;
use cordial_model::id::{ChannelId, GenericId, RoleId, UserId};
use cordial_model::interaction::{CommandData, CommandDataOption, Resolved};
use cordial_model::message::{Attachment, Message};

use crate::reader::Reader;

/// A command with one option of each type.
#[derive(Command, Debug)]
#[command(name = "fuzz", description = "A command for the fuzz target")]
#[allow(dead_code)]
struct AllTypes {
    #[option(description = "A user")]
    user: ResolvedUser,
    #[option(description = "A role")]
    role: Option<Role>,
    #[option(description = "A channel")]
    channel: Option<Channel>,
    #[option(description = "An attachment")]
    file: Option<Attachment>,
    #[option(description = "A text", min_length = 1, max_length = 100)]
    text: Option<String>,
    #[option(description = "An integer")]
    integer: Option<i64>,
    #[option(description = "A small integer")]
    small: Option<u8>,
    #[option(description = "A number")]
    number: Option<f64>,
    #[option(description = "A flag")]
    flag: Option<bool>,
    #[option(description = "A mentionable")]
    target: Option<Mentionable>,
}

/// The deepest nesting of options that the body follows. Discord nests at most 3 levels.
const MAX_DEPTH: usize = 4;

/// Command data or a `custom_id` pattern.
///
/// With selector bit 0 clear, the rest is the JSON of command data, which goes to the option
/// parsers. With the bit set, the first line of the rest is a pattern and the second line is a
/// `custom_id`.
pub fn command_parse(data: &[u8]) {
    let mut reader = Reader::new(data);
    let selector = reader.byte();
    if selector & 1 == 0 {
        parse_command(reader.rest());
    } else {
        match_pattern(reader.rest());
    }
}

fn parse_command(body: &[u8]) {
    let Ok(data) = serde_json::from_slice::<CommandData>(body) else {
        return;
    };

    let _ = data.leaf_options();
    let _ = AllTypes::parse(&data);
    let _ = ResolvedUser::from_data(&data);
    let _ = Message::from_data(&data);
    parse_level(&data.options, data.resolved.as_ref(), 0);
}

fn parse_level(options: &[CommandDataOption], resolved: Option<&Resolved>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    let input = OptionsInput::new(options, resolved);
    let _ = input.focused();

    for option in options {
        parse_all_types(input, &option.name);
        parse_level(&option.options, resolved, depth + 1);
    }
    if let Ok((_, sub_input)) = input.subcommand() {
        let _ = sub_input.focused();
    }
}

fn parse_all_types(input: OptionsInput<'_>, name: &str) {
    parse_as::<String>(input, name);
    parse_as::<bool>(input, name);
    parse_as::<f64>(input, name);
    parse_as::<i64>(input, name);
    parse_as::<i8>(input, name);
    parse_as::<u64>(input, name);
    parse_as::<UserId>(input, name);
    parse_as::<ChannelId>(input, name);
    parse_as::<RoleId>(input, name);
    parse_as::<GenericId>(input, name);
    parse_as::<ResolvedUser>(input, name);
    parse_as::<cordial_model::user::User>(input, name);
    parse_as::<Role>(input, name);
    parse_as::<Channel>(input, name);
    parse_as::<Attachment>(input, name);
    parse_as::<Mentionable>(input, name);
}

fn parse_as<T: CommandArg>(input: OptionsInput<'_>, name: &str) {
    let _ = input.optional::<T>(name);
    let _ = input.required::<T>(name);
}

fn match_pattern(rest: &[u8]) {
    let text = String::from_utf8_lossy(rest);
    let (source, custom_id) = text.split_once('\n').unwrap_or((&text, ""));
    let Ok(pattern) = Pattern::parse(source) else {
        return;
    };
    assert_eq!(pattern.as_str(), source);
    let Some(params) = pattern.matches(custom_id) else {
        return;
    };

    // A match must give each parameter, and the values must make the same `custom_id`.
    let values: Vec<(&str, String)> = parameter_names(source)
        .into_iter()
        .map(|name| {
            let value = params.get(name).expect("a matched parameter has a value");
            (name, value.to_owned())
        })
        .collect();
    let references: Vec<(&str, &dyn Display)> = values
        .iter()
        .map(|(name, value)| (*name, value as &dyn Display))
        .collect();
    assert_eq!(pattern.format(&references).as_deref(), Some(custom_id));
}

fn parameter_names(source: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            break;
        };
        names.push(&after[..end]);
        rest = &after[end + 1..];
    }
    names
}
