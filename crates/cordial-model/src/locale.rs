//! Locales that Discord supports.

crate::discord_string_enum! {
    /// A locale that Discord supports for users and for command localization.
    pub enum Locale {
        /// Indonesian.
        Indonesian = "id",
        /// Danish.
        Danish = "da",
        /// German.
        German = "de",
        /// English, UK.
        EnglishUk = "en-GB",
        /// English, US.
        EnglishUs = "en-US",
        /// Spanish.
        Spanish = "es-ES",
        /// Spanish, Latin America.
        SpanishLatinAmerica = "es-419",
        /// French.
        French = "fr",
        /// Croatian.
        Croatian = "hr",
        /// Italian.
        Italian = "it",
        /// Lithuanian.
        Lithuanian = "lt",
        /// Hungarian.
        Hungarian = "hu",
        /// Dutch.
        Dutch = "nl",
        /// Norwegian.
        Norwegian = "no",
        /// Polish.
        Polish = "pl",
        /// Portuguese, Brazilian.
        PortugueseBrazilian = "pt-BR",
        /// Romanian.
        Romanian = "ro",
        /// Finnish.
        Finnish = "fi",
        /// Swedish.
        Swedish = "sv-SE",
        /// Vietnamese.
        Vietnamese = "vi",
        /// Turkish.
        Turkish = "tr",
        /// Czech.
        Czech = "cs",
        /// Greek.
        Greek = "el",
        /// Bulgarian.
        Bulgarian = "bg",
        /// Russian.
        Russian = "ru",
        /// Ukrainian.
        Ukrainian = "uk",
        /// Hindi.
        Hindi = "hi",
        /// Thai.
        Thai = "th",
        /// Chinese, China.
        ChineseChina = "zh-CN",
        /// Japanese.
        Japanese = "ja",
        /// Chinese, Taiwan.
        ChineseTaiwan = "zh-TW",
        /// Korean.
        Korean = "ko",
    }
}

/// The default is US English, as Discord uses it.
impl Default for Locale {
    fn default() -> Self {
        Self::EnglishUs
    }
}

/// A map from locale to localized text, as in `name_localizations`.
pub type Localizations = std::collections::HashMap<Locale, String>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_known_and_unknown() {
        let known: Locale = serde_json::from_str("\"en-US\"").unwrap();
        let unknown: Locale = serde_json::from_str("\"xx-YY\"").unwrap();

        assert_eq!(known, Locale::EnglishUs);
        assert_eq!(unknown, Locale::Unknown("xx-YY".to_owned()));
        assert!(unknown.is_unknown());
        assert_eq!(serde_json::to_string(&unknown).unwrap(), "\"xx-YY\"");
    }
}
