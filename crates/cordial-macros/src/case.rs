//! Conversions between the cases of identifiers.

/// Splits a Rust identifier into lowercase words.
///
/// A new word starts at an underscore, at an uppercase letter after a lowercase letter or a
/// digit, and at the last uppercase letter of an acronym (`HTTPServer` gives `http server`).
fn words(identifier: &str) -> Vec<String> {
    let characters: Vec<char> = identifier.chars().collect();
    let mut words = Vec::new();
    let mut word = String::new();

    for (index, &character) in characters.iter().enumerate() {
        if character == '_' {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }

        let previous = index.checked_sub(1).map(|before| characters[before]);
        let next = characters.get(index + 1).copied();
        let after_lowercase = previous.is_some_and(|c| c.is_lowercase() || c.is_numeric());
        let ends_acronym =
            previous.is_some_and(char::is_uppercase) && next.is_some_and(char::is_lowercase);
        if character.is_uppercase() && (after_lowercase || ends_acronym) && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }

        word.extend(character.to_lowercase());
    }

    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// `ShowAll` gives `show-all`.
pub(crate) fn kebab_case(identifier: &str) -> String {
    words(identifier).join("-")
}

/// `ShowAll` gives `show_all`.
pub(crate) fn snake_case(identifier: &str) -> String {
    words(identifier).join("_")
}

/// `bot_dm` gives `BotDm`.
pub(crate) fn pascal_case(identifier: &str) -> String {
    words(identifier)
        .iter()
        .map(|word| {
            let mut characters = word.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(characters).collect()
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{kebab_case, pascal_case, snake_case};

    #[test]
    fn converts_cases() {
        assert_eq!(kebab_case("ShowAll"), "show-all");
        assert_eq!(kebab_case("Add"), "add");
        assert_eq!(kebab_case("HTTPServer"), "http-server");
        assert_eq!(kebab_case("Level2Up"), "level2-up");
        assert_eq!(snake_case("ShowAll"), "show_all");
        assert_eq!(pascal_case("bot_dm"), "BotDm");
        assert_eq!(pascal_case("guild"), "Guild");
    }
}
