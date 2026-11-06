//! Client-side checks of the documented limits.
//!
//! Discord counts some failed requests against a limit of 10,000 in 10 minutes.
//! A check here finds the error before the request goes out.

use std::fmt;

use crate::command::{
    CHOICE_COUNT_LIMIT, CommandDefinition, CommandOption, CommandOptionType, CommandType,
    OPTION_COUNT_LIMIT, TOTAL_CHARACTER_LIMIT,
};
use crate::component::{Component, ComponentType};
use crate::embed::{self, Embed};
use crate::message::{self, MessageFlags};

/// The maximum number of components in a Components V2 message, nested ones included.
pub const COMPONENTS_V2_LIMIT: usize = 40;
/// The maximum number of action rows in a message without Components V2.
pub const ACTION_ROW_LIMIT: usize = 5;
/// The maximum number of buttons in an action row.
pub const BUTTONS_PER_ROW_LIMIT: usize = 5;
/// The maximum number of characters in a custom ID.
pub const CUSTOM_ID_LIMIT: usize = 100;
/// The maximum number of top-level components in a modal.
pub const MODAL_COMPONENT_LIMIT: usize = 5;
/// The maximum number of characters in a modal title.
pub const MODAL_TITLE_LIMIT: usize = 45;

/// A value that breaks a documented limit.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ValidationError {
    /// The field, for example `embeds[0].title`.
    pub field: String,
    /// What is wrong.
    pub reason: String,
}

impl ValidationError {
    /// Makes an error.
    pub fn new(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.field, self.reason)
    }
}

impl std::error::Error for ValidationError {}

fn char_count(text: &str) -> usize {
    text.chars().count()
}

fn check_length(field: &str, text: &str, min: usize, max: usize) -> Result<(), ValidationError> {
    let count = char_count(text);
    if count < min || count > max {
        return Err(ValidationError::new(
            field,
            format!("has {count} characters; it must have {min} to {max}"),
        ));
    }
    Ok(())
}

fn check_count(field: &str, count: usize, max: usize) -> Result<(), ValidationError> {
    if count > max {
        return Err(ValidationError::new(
            field,
            format!("has {count} items; the maximum is {max}"),
        ));
    }
    Ok(())
}

/// Checks the content of a message.
///
/// # Errors
///
/// Gives an error when the content has more than 2000 characters.
pub fn message_content(content: &str) -> Result<(), ValidationError> {
    check_length("content", content, 0, message::CONTENT_LIMIT)
}

/// Checks the embeds of a message: the count, each field, and the total of 6000 characters.
///
/// # Errors
///
/// Gives an error for the first limit that an embed breaks.
pub fn embeds(embeds: &[Embed]) -> Result<(), ValidationError> {
    check_count("embeds", embeds.len(), message::EMBED_COUNT_LIMIT)?;

    let mut total = 0;
    for (index, embed) in embeds.iter().enumerate() {
        let prefix = format!("embeds[{index}]");
        if let Some(title) = &embed.title {
            check_length(&format!("{prefix}.title"), title, 0, embed::TITLE_LIMIT)?;
        }
        if let Some(description) = &embed.description {
            check_length(
                &format!("{prefix}.description"),
                description,
                0,
                embed::DESCRIPTION_LIMIT,
            )?;
        }
        check_count(
            &format!("{prefix}.fields"),
            embed.fields.len(),
            embed::FIELD_COUNT_LIMIT,
        )?;
        for (field_index, field) in embed.fields.iter().enumerate() {
            check_length(
                &format!("{prefix}.fields[{field_index}].name"),
                &field.name,
                1,
                embed::FIELD_NAME_LIMIT,
            )?;
            check_length(
                &format!("{prefix}.fields[{field_index}].value"),
                &field.value,
                1,
                embed::FIELD_VALUE_LIMIT,
            )?;
        }
        if let Some(footer) = &embed.footer {
            check_length(
                &format!("{prefix}.footer.text"),
                &footer.text,
                0,
                embed::FOOTER_TEXT_LIMIT,
            )?;
        }
        if let Some(author) = &embed.author {
            check_length(
                &format!("{prefix}.author.name"),
                &author.name,
                0,
                embed::AUTHOR_NAME_LIMIT,
            )?;
        }
        total += embed.character_count();
    }

    if total > embed::TOTAL_LIMIT {
        return Err(ValidationError::new(
            "embeds",
            format!(
                "have {total} characters in total; the maximum is {}",
                embed::TOTAL_LIMIT
            ),
        ));
    }
    Ok(())
}

/// Checks the parts of a message that depend on the Components V2 flag.
///
/// # Errors
///
/// Gives an error when a field is not valid with the flag, or a component limit is broken.
pub fn message_components(
    flags: MessageFlags,
    components: &[Component],
    has_content: bool,
    has_embeds: bool,
    has_poll: bool,
) -> Result<(), ValidationError> {
    if flags.contains(MessageFlags::IS_COMPONENTS_V2) {
        if has_content || has_embeds || has_poll {
            return Err(ValidationError::new(
                "flags",
                "IS_COMPONENTS_V2 does not permit content, embeds, or a poll",
            ));
        }
        let count: usize = components.iter().map(Component::count_recursive).sum();
        check_count("components (nested included)", count, COMPONENTS_V2_LIMIT)?;
    } else {
        check_count("components", components.len(), ACTION_ROW_LIMIT)?;
        for (index, component) in components.iter().enumerate() {
            if component.kind() != ComponentType::ActionRow {
                return Err(ValidationError::new(
                    format!("components[{index}]"),
                    "without IS_COMPONENTS_V2, top-level components must be action rows",
                ));
            }
        }
    }

    components_tree(components, "components")
}

fn components_tree(components: &[Component], path: &str) -> Result<(), ValidationError> {
    for (index, component) in components.iter().enumerate() {
        let field = format!("{path}[{index}]");
        if let Some(custom_id) = component.custom_id() {
            check_length(&format!("{field}.custom_id"), custom_id, 1, CUSTOM_ID_LIMIT)?;
        }
        if let Component::ActionRow(row) = component {
            let buttons = row
                .components
                .iter()
                .filter(|child| child.kind() == ComponentType::Button)
                .count();
            let selects = row.components.len() - buttons;
            if buttons > BUTTONS_PER_ROW_LIMIT || (selects > 0 && row.components.len() > 1) {
                return Err(ValidationError::new(
                    field.clone(),
                    "an action row holds up to 5 buttons or one select menu",
                ));
            }
        }
        let children: Vec<Component> = component.children().into_iter().cloned().collect();
        components_tree(&children, &format!("{field}.components"))?;
    }
    Ok(())
}

/// Checks a modal.
///
/// # Errors
///
/// Gives an error when the custom ID, the title, or the component count is not valid.
pub fn modal(
    custom_id: &str,
    title: &str,
    components: &[Component],
) -> Result<(), ValidationError> {
    check_length("custom_id", custom_id, 1, CUSTOM_ID_LIMIT)?;
    check_length("title", title, 1, MODAL_TITLE_LIMIT)?;
    if components.is_empty() {
        return Err(ValidationError::new(
            "components",
            "a modal needs 1 to 5 components",
        ));
    }
    check_count("components", components.len(), MODAL_COMPONENT_LIMIT)?;
    components_tree(components, "components")
}

/// Tells if a name is valid for a slash command or an option.
///
/// The rule is `^[-_ʼ\p{L}\p{N}\p{sc=Deva}\p{sc=Thai}]{1,32}$`, and letters that have a
/// lowercase form must be lowercase.
#[must_use]
pub fn is_valid_chat_input_name(name: &str) -> bool {
    let count = char_count(name);
    if !(1..=32).contains(&count) {
        return false;
    }

    name.chars().all(|character| {
        let allowed = character == '-'
            || character == '_'
            || character == '\u{02BC}'
            || character.is_alphanumeric()
            || is_devanagari_or_thai(character);
        let lowercase_ok = !character.is_uppercase();
        allowed && lowercase_ok
    })
}

fn is_devanagari_or_thai(character: char) -> bool {
    matches!(character, '\u{0900}'..='\u{097F}' | '\u{A8E0}'..='\u{A8FF}' | '\u{0E00}'..='\u{0E7F}')
}

/// Checks a command definition: names, descriptions, option counts, nesting, and the total
/// size of 8000 characters.
///
/// # Errors
///
/// Gives an error for the first rule that the command breaks.
pub fn command(definition: &CommandDefinition) -> Result<(), ValidationError> {
    match definition.kind {
        CommandType::ChatInput => {
            if !is_valid_chat_input_name(&definition.name) {
                return Err(ValidationError::new(
                    "name",
                    "must be 1 to 32 lowercase letters, digits, '-', or '_'",
                ));
            }
            check_length("description", &definition.description, 1, 100)?;
        }
        CommandType::User | CommandType::Message => {
            check_length("name", &definition.name, 1, 32)?;
            if !definition.description.is_empty() {
                return Err(ValidationError::new(
                    "description",
                    "must be empty for context menu commands",
                ));
            }
            if !definition.options.is_empty() {
                return Err(ValidationError::new(
                    "options",
                    "context menu commands cannot have options",
                ));
            }
        }
        _ => check_length("name", &definition.name, 1, 32)?,
    }

    check_options(&definition.options, "options", 0)?;

    let total = command_character_count(definition);
    if total > TOTAL_CHARACTER_LIMIT {
        return Err(ValidationError::new(
            "command",
            format!(
                "has {total} characters in names, descriptions, and values; the maximum is {TOTAL_CHARACTER_LIMIT}"
            ),
        ));
    }
    Ok(())
}

fn check_options(options: &[CommandOption], path: &str, depth: u8) -> Result<(), ValidationError> {
    check_count(path, options.len(), OPTION_COUNT_LIMIT)?;

    let mut seen_optional = false;
    let mut names = std::collections::HashSet::new();
    for (index, option) in options.iter().enumerate() {
        let field = format!("{path}[{index}]");
        if !is_valid_chat_input_name(&option.name) {
            return Err(ValidationError::new(
                format!("{field}.name"),
                "is not a valid option name",
            ));
        }
        if !names.insert(option.name.as_str()) {
            return Err(ValidationError::new(
                format!("{field}.name"),
                "must be unique among its siblings",
            ));
        }
        check_length(&format!("{field}.description"), &option.description, 1, 100)?;

        let is_group = option.kind == CommandOptionType::SubCommandGroup;
        let is_subcommand = option.kind == CommandOptionType::SubCommand;
        if is_group && depth > 0 {
            return Err(ValidationError::new(
                field,
                "a subcommand group can only be at the top level",
            ));
        }
        if is_subcommand && depth > 1 {
            return Err(ValidationError::new(
                field,
                "a subcommand cannot be nested this deep",
            ));
        }

        if !is_group && !is_subcommand {
            if option.required && seen_optional {
                return Err(ValidationError::new(
                    field,
                    "required options must come before optional options",
                ));
            }
            seen_optional |= !option.required;
        }

        if let Some(choices) = &option.choices {
            check_count(
                &format!("{field}.choices"),
                choices.len(),
                CHOICE_COUNT_LIMIT,
            )?;
            if option.autocomplete {
                return Err(ValidationError::new(
                    field,
                    "autocomplete cannot be used with choices",
                ));
            }
        }

        if is_group || is_subcommand {
            check_options(&option.options, &format!("{field}.options"), depth + 1)?;
        }
    }
    Ok(())
}

fn command_character_count(definition: &CommandDefinition) -> usize {
    fn longest(default: &str, localized: Option<&crate::locale::Localizations>) -> usize {
        let localized_max = localized
            .into_iter()
            .flat_map(|map| map.values())
            .map(|text| char_count(text))
            .max()
            .unwrap_or(0);
        char_count(default).max(localized_max)
    }

    fn option_count(option: &CommandOption) -> usize {
        let own = longest(&option.name, option.name_localizations.as_ref())
            + longest(
                &option.description,
                option.description_localizations.as_ref(),
            );
        let choices: usize = option
            .choices
            .iter()
            .flatten()
            .map(|choice| {
                let value = match &choice.value {
                    crate::command::CommandOptionChoiceValue::String(text) => char_count(text),
                    other => serde_json::to_string(other).map_or(0, |text| text.len()),
                };
                longest(&choice.name, choice.name_localizations.as_ref()) + value
            })
            .sum();
        let nested: usize = option.options.iter().map(option_count).sum();
        own + choices + nested
    }

    longest(&definition.name, definition.name_localizations.as_ref())
        + longest(
            &definition.description,
            definition.description_localizations.as_ref(),
        )
        + definition.options.iter().map(option_count).sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{ActionRow, Button, ButtonStyle, TextDisplay};

    #[test]
    fn content_limit() {
        assert!(message_content(&"a".repeat(2000)).is_ok());
        assert!(message_content(&"a".repeat(2001)).is_err());
    }

    #[test]
    fn embed_total_limit() {
        let embed = Embed::new().description("a".repeat(4000));
        let two = vec![embed.clone(), embed];

        let error = embeds(&two).unwrap_err();

        assert_eq!(error.field, "embeds");
    }

    #[test]
    fn components_v2_rules() {
        let text: Component = TextDisplay::new("hi").into();

        assert!(
            message_components(
                MessageFlags::IS_COMPONENTS_V2,
                std::slice::from_ref(&text),
                false,
                false,
                false
            )
            .is_ok()
        );
        assert!(
            message_components(
                MessageFlags::IS_COMPONENTS_V2,
                std::slice::from_ref(&text),
                true,
                false,
                false
            )
            .is_err()
        );
        assert!(message_components(MessageFlags::empty(), &[text], false, false, false).is_err());
    }

    #[test]
    fn action_row_limits() {
        let buttons: Vec<Button> = (0..6)
            .map(|index| Button::new(ButtonStyle::Primary, format!("b{index}"), "x"))
            .collect();
        let row: Component = ActionRow::new(buttons).into();

        assert!(message_components(MessageFlags::empty(), &[row], false, false, false).is_err());
    }

    #[test]
    fn command_names() {
        assert!(is_valid_chat_input_name("ban"));
        assert!(is_valid_chat_input_name("user-info_2"));
        assert!(is_valid_chat_input_name("ユーザー"));
        assert!(!is_valid_chat_input_name("Ban"));
        assert!(!is_valid_chat_input_name("two words"));
        assert!(!is_valid_chat_input_name(""));
    }

    #[test]
    fn command_option_order() {
        let definition = CommandDefinition::chat_input("ban", "Ban a member")
            .option(CommandOption::new(
                CommandOptionType::String,
                "reason",
                "Why",
            ))
            .option(CommandOption::new(CommandOptionType::User, "user", "Who").required(true));

        let error = command(&definition).unwrap_err();

        assert!(error.reason.contains("required options"));
    }
}
