//! Message formatting: mentions, timestamps, emojis, and guild navigation.
//!
//! ```
//! use cordial_model::id::UserId;
//! use cordial_model::mention::{self, TimestampStyle};
//!
//! let user = UserId::new_const(80351110224678912);
//! assert_eq!(mention::user(user), "<@80351110224678912>");
//! assert_eq!(mention::timestamp(1618953630, TimestampStyle::Relative), "<t:1618953630:R>");
//! ```

use crate::id::{ChannelId, CommandId, EmojiId, RoleId, UserId};

/// Mentions a user: `<@USER_ID>`.
#[must_use]
pub fn user(user_id: UserId) -> String {
    format!("<@{user_id}>")
}

/// Mentions a channel: `<#CHANNEL_ID>`.
#[must_use]
pub fn channel(channel_id: ChannelId) -> String {
    format!("<#{channel_id}>")
}

/// Mentions a role: `<@&ROLE_ID>`.
#[must_use]
pub fn role(role_id: RoleId) -> String {
    format!("<@&{role_id}>")
}

/// Mentions a slash command: `</NAME:ID>`. `name` can include a subcommand group and a
/// subcommand, separated by spaces.
#[must_use]
pub fn command(name: &str, command_id: CommandId) -> String {
    format!("</{name}:{command_id}>")
}

/// Shows a custom emoji: `<:NAME:ID>`, or `<a:NAME:ID>` when animated.
#[must_use]
pub fn emoji(name: &str, emoji_id: EmojiId, animated: bool) -> String {
    let prefix = if animated { "a" } else { "" };
    format!("<{prefix}:{name}:{emoji_id}>")
}

/// Shows a time that each user sees in the own time zone: `<t:SECONDS:STYLE>`.
#[must_use]
pub fn timestamp(unix_seconds: i64, style: TimestampStyle) -> String {
    format!("<t:{unix_seconds}:{}>", style.code())
}

/// The style of a formatted timestamp.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TimestampStyle {
    /// Short time, for example `16:20`.
    ShortTime,
    /// Medium time, for example `16:20:30`.
    MediumTime,
    /// Short date, for example `20/04/2021`.
    ShortDate,
    /// Long date, for example `April 20, 2021`.
    LongDate,
    /// Long date and short time, for example `April 20, 2021 at 16:20`. The default.
    #[default]
    LongDateShortTime,
    /// Full date and short time, for example `Tuesday, April 20, 2021 at 16:20`.
    FullDateShortTime,
    /// Short date and short time, for example `20/04/2021, 16:20`.
    ShortDateShortTime,
    /// Short date and medium time, for example `20/04/2021, 16:20:30`.
    ShortDateMediumTime,
    /// Relative time, for example `4 years ago`.
    Relative,
}

impl TimestampStyle {
    /// Gives the style letter.
    #[must_use]
    pub const fn code(self) -> char {
        match self {
            Self::ShortTime => 't',
            Self::MediumTime => 'T',
            Self::ShortDate => 'd',
            Self::LongDate => 'D',
            Self::LongDateShortTime => 'f',
            Self::FullDateShortTime => 'F',
            Self::ShortDateShortTime => 's',
            Self::ShortDateMediumTime => 'S',
            Self::Relative => 'R',
        }
    }
}

/// A tab of a guild that a message can link to: `<id:TYPE>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GuildNavigation {
    /// The "Channels & Roles" tab.
    Customize,
    /// The "Browse Channels" tab.
    Browse,
    /// The Server Guide tab.
    Guide,
    /// The "Linked Roles" tab.
    LinkedRoles,
    /// One linked role. A click opens the connection dialog.
    LinkedRole(RoleId),
}

/// Links to a tab of the guild.
#[must_use]
pub fn guild_navigation(target: GuildNavigation) -> String {
    match target {
        GuildNavigation::Customize => "<id:customize>".to_owned(),
        GuildNavigation::Browse => "<id:browse>".to_owned(),
        GuildNavigation::Guide => "<id:guide>".to_owned(),
        GuildNavigation::LinkedRoles => "<id:linked-roles>".to_owned(),
        GuildNavigation::LinkedRole(role_id) => format!("<id:linked-roles:{role_id}>"),
    }
}

/// Escapes Markdown characters, so that Discord shows the text as it is.
#[must_use]
pub fn escape_markdown(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if matches!(
            character,
            '\\' | '*' | '_' | '~' | '`' | '|' | '>' | '#' | '-' | '[' | ']' | '(' | ')'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

/// Breaks `@everyone`, `@here`, and other mentions with a zero-width space.
///
/// Prefer `allowed_mentions` to control notifications. Use this only for display.
#[must_use]
pub fn neutralize_mentions(text: &str) -> String {
    text.replace('@', "@\u{200B}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_the_docs() {
        assert_eq!(
            channel(ChannelId::new_const(103_735_883_630_395_392)),
            "<#103735883630395392>"
        );
        assert_eq!(
            role(RoleId::new_const(165_511_591_545_143_296)),
            "<@&165511591545143296>"
        );
        assert_eq!(
            command(
                "foo group bar",
                CommandId::new_const(123_456_789_012_345_678)
            ),
            "</foo group bar:123456789012345678>"
        );
        assert_eq!(
            emoji("b1nzy", EmojiId::new_const(392_938_283_556_143_104), true),
            "<a:b1nzy:392938283556143104>"
        );
        assert_eq!(
            timestamp(1_618_953_630, TimestampStyle::ShortDate),
            "<t:1618953630:d>"
        );
        assert_eq!(
            guild_navigation(GuildNavigation::LinkedRole(RoleId::new_const(5))),
            "<id:linked-roles:5>"
        );
    }

    #[test]
    fn escapes_markdown() {
        assert_eq!(escape_markdown("*bold* _x_"), "\\*bold\\* \\_x\\_");
    }
}
