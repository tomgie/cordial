//! Emojis.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::id::{EmojiId, RoleId};
use crate::user::User;

/// A custom emoji of a guild or an application, or a Unicode emoji.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Emoji {
    /// The emoji ID. `None` for a Unicode emoji.
    #[serde(default)]
    pub id: Option<EmojiId>,
    /// The name. For a Unicode emoji, the emoji itself. `None` only in reactions,
    /// when the data of a custom emoji is not available.
    #[serde(default)]
    pub name: Option<String>,
    /// The roles that can use the emoji.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<RoleId>,
    /// The user that made the emoji.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// Tells if the emoji must be wrapped in colons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_colons: Option<bool>,
    /// Tells if an integration manages the emoji.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managed: Option<bool>,
    /// Tells if the emoji is animated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animated: Option<bool>,
    /// Tells if the emoji can be used. It can be `false` after the guild loses Server Boosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available: Option<bool>,
}

impl Emoji {
    /// Gives the reaction form of the emoji, for example `name:123` or `👍`.
    #[must_use]
    pub fn reaction(&self) -> Option<ReactionType> {
        match (self.id, &self.name) {
            (Some(id), name) => Some(ReactionType::Custom {
                id,
                name: name.clone(),
                animated: self.animated.unwrap_or(false),
            }),
            (None, Some(name)) => Some(ReactionType::Unicode(name.clone())),
            (None, None) => None,
        }
    }
}

/// A partial emoji, as in components, reactions, and polls.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct PartialEmoji {
    /// The emoji ID. `None` for a Unicode emoji.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<EmojiId>,
    /// The name, or the Unicode emoji itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Tells if the emoji is animated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animated: Option<bool>,
}

impl PartialEmoji {
    /// Makes a Unicode emoji.
    #[must_use]
    pub fn unicode(emoji: impl Into<String>) -> Self {
        Self {
            id: None,
            name: Some(emoji.into()),
            animated: None,
        }
    }

    /// Makes a custom emoji.
    #[must_use]
    pub fn custom(id: EmojiId, name: impl Into<String>, animated: bool) -> Self {
        Self {
            id: Some(id),
            name: Some(name.into()),
            animated: animated.then_some(true),
        }
    }
}

/// An emoji for a reaction request.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ReactionType {
    /// A Unicode emoji, for example `👍`.
    Unicode(String),
    /// A custom emoji.
    Custom {
        /// The emoji ID.
        id: EmojiId,
        /// The emoji name.
        name: Option<String>,
        /// Tells if the emoji is animated.
        animated: bool,
    },
}

impl ReactionType {
    /// Makes a custom emoji reaction.
    #[must_use]
    pub fn custom(id: EmojiId, name: impl Into<String>) -> Self {
        Self::Custom {
            id,
            name: Some(name.into()),
            animated: false,
        }
    }
}

/// Formats the emoji as the HTTP API needs it in a path, before URL encoding:
/// `name:id` for a custom emoji, or the Unicode emoji itself.
impl fmt::Display for ReactionType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unicode(emoji) => formatter.write_str(emoji),
            Self::Custom { id, name, .. } => {
                write!(formatter, "{}:{id}", name.as_deref().unwrap_or("_"))
            }
        }
    }
}

impl From<&str> for ReactionType {
    fn from(emoji: &str) -> Self {
        Self::Unicode(emoji.to_owned())
    }
}

impl From<PartialEmoji> for Option<ReactionType> {
    fn from(emoji: PartialEmoji) -> Self {
        match (emoji.id, emoji.name) {
            (Some(id), name) => Some(ReactionType::Custom {
                id,
                name,
                animated: emoji.animated.unwrap_or(false),
            }),
            (None, Some(name)) => Some(ReactionType::Unicode(name)),
            (None, None) => None,
        }
    }
}

/// The emojis of an application, as `GET /applications/{id}/emojis` gives them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ApplicationEmojis {
    /// The emojis.
    pub items: Vec<Emoji>,
}
