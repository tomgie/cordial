//! Localization of command names, descriptions, and choices.

use cordial_model::command::{CommandDefinition, CommandOption};
use cordial_model::locale::Localizations;

/// The text that a localization is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LocalizedField<'a> {
    /// The name of the command or the option.
    Name,
    /// The description of the command or the option.
    Description,
    /// The name of a choice, with the default name.
    Choice(&'a str),
}

/// The place of a localized text in a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct LocalizationKey<'a> {
    /// The command name.
    pub command: &'a str,
    /// The option names from the command to the option. Empty for the command itself.
    pub path: &'a [String],
    /// The text.
    pub field: LocalizedField<'a>,
}

impl LocalizationKey<'_> {
    /// Gives a dotted key, for example `ban.options.reason.description` or
    /// `ban.options.kind.choices.Spam`.
    #[must_use]
    pub fn dotted(&self) -> String {
        let mut key = self.command.to_owned();
        for name in self.path {
            key.push_str(".options.");
            key.push_str(name);
        }
        match self.field {
            LocalizedField::Name => key.push_str(".name"),
            LocalizedField::Description => key.push_str(".description"),
            LocalizedField::Choice(choice) => {
                key.push_str(".choices.");
                key.push_str(choice);
            }
        }
        key
    }
}

/// Gives the translations of command texts.
///
/// A function `Fn(&LocalizationKey) -> Localizations` is a localizer.
pub trait Localizer: Send + Sync + 'static {
    /// Gives the translations of one text. An empty map means no translations.
    fn localize(&self, key: &LocalizationKey<'_>) -> Localizations;
}

impl<F> Localizer for F
where
    F: Fn(&LocalizationKey<'_>) -> Localizations + Send + Sync + 'static,
{
    fn localize(&self, key: &LocalizationKey<'_>) -> Localizations {
        self(key)
    }
}

fn non_empty(localizations: Localizations) -> Option<Localizations> {
    (!localizations.is_empty()).then_some(localizations)
}

/// Adds the translations to a definition. Translations that the definition already has stay.
pub(crate) fn apply(definition: &mut CommandDefinition, localizer: &dyn Localizer) {
    let command = definition.name.clone();
    let key = |field| LocalizationKey {
        command: &command,
        path: &[],
        field,
    };

    if definition.name_localizations.is_none() {
        definition.name_localizations = non_empty(localizer.localize(&key(LocalizedField::Name)));
    }
    // User and message commands have no description.
    if definition.description_localizations.is_none() && !definition.description.is_empty() {
        definition.description_localizations =
            non_empty(localizer.localize(&key(LocalizedField::Description)));
    }

    let mut path = Vec::new();
    for option in &mut definition.options {
        apply_option(&command, &mut path, option, localizer);
    }
}

fn apply_option(
    command: &str,
    path: &mut Vec<String>,
    option: &mut CommandOption,
    localizer: &dyn Localizer,
) {
    path.push(option.name.clone());
    let localize = |field| {
        non_empty(localizer.localize(&LocalizationKey {
            command,
            path,
            field,
        }))
    };

    if option.name_localizations.is_none() {
        option.name_localizations = localize(LocalizedField::Name);
    }
    if option.description_localizations.is_none() {
        option.description_localizations = localize(LocalizedField::Description);
    }
    for choice in option.choices.iter_mut().flatten() {
        if choice.name_localizations.is_none() {
            choice.name_localizations = localize(LocalizedField::Choice(&choice.name));
        }
    }

    for child in &mut option.options {
        apply_option(command, path, child, localizer);
    }
    path.pop();
}

#[cfg(test)]
mod tests {
    use cordial_model::command::CommandOptionType;
    use cordial_model::locale::Locale;

    use super::*;

    #[test]
    fn fills_translations_by_dotted_key() {
        let localizer = |key: &LocalizationKey<'_>| {
            let mut map = Localizations::new();
            map.insert(Locale::German, format!("de:{}", key.dotted()));
            map
        };
        let mut definition = CommandDefinition::chat_input("ban", "Ban").option(
            CommandOption::new(CommandOptionType::String, "kind", "The kind")
                .choice("Spam", "spam"),
        );

        apply(&mut definition, &localizer);

        let german = |map: &Option<Localizations>| map.as_ref().unwrap()[&Locale::German].clone();
        assert_eq!(german(&definition.name_localizations), "de:ban.name");
        let option = &definition.options[0];
        assert_eq!(
            german(&option.description_localizations),
            "de:ban.options.kind.description"
        );
        assert_eq!(
            german(&option.choices.as_ref().unwrap()[0].name_localizations),
            "de:ban.options.kind.choices.Spam"
        );
    }
}
