//! Embeds.
//!
//! [`Embed`] has chainable setters, so a request can make an embed in one expression:
//!
//! ```
//! use cordial_model::Color;
//! use cordial_model::embed::Embed;
//!
//! let embed = Embed::new()
//!     .title("Server status")
//!     .description("All systems work.")
//!     .color(Color(0x57F287))
//!     .field("Latency", "42 ms", true);
//! assert_eq!(embed.fields.len(), 1);
//! ```

use serde::{Deserialize, Serialize};

use crate::component::Component;
use crate::{Color, Timestamp};

/// The maximum number of characters in a title.
pub const TITLE_LIMIT: usize = 256;
/// The maximum number of characters in a description.
pub const DESCRIPTION_LIMIT: usize = 4096;
/// The maximum number of fields.
pub const FIELD_COUNT_LIMIT: usize = 25;
/// The maximum number of characters in a field name.
pub const FIELD_NAME_LIMIT: usize = 256;
/// The maximum number of characters in a field value.
pub const FIELD_VALUE_LIMIT: usize = 1024;
/// The maximum number of characters in a footer text.
pub const FOOTER_TEXT_LIMIT: usize = 2048;
/// The maximum number of characters in an author name.
pub const AUTHOR_NAME_LIMIT: usize = 256;
/// The maximum number of characters in all embeds of a message.
pub const TOTAL_LIMIT: usize = 6000;

/// An embed: rich content in a message.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Embed {
    /// The title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The embed type. Always `rich` for embeds that bots send.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<EmbedType>,
    /// The description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The URL of the title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The time of the content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<Timestamp>,
    /// The color of the left border.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// The footer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer: Option<EmbedFooter>,
    /// The image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<EmbedMedia>,
    /// The thumbnail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<EmbedMedia>,
    /// The video. Bots cannot set it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<EmbedMedia>,
    /// The provider. Bots cannot set it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<EmbedProvider>,
    /// The author.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<EmbedAuthor>,
    /// The fields, up to 25.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<EmbedField>,
    /// The components of a `components` type embed (a link preview).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<Component>>,
    /// The embed flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<EmbedFlags>,
}

impl Embed {
    /// Makes an empty embed.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the title.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets the description.
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the URL of the title.
    #[must_use]
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Sets the time of the content.
    #[must_use]
    pub fn timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Sets the color of the left border.
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Sets the footer text.
    #[must_use]
    pub fn footer(mut self, text: impl Into<String>) -> Self {
        self.footer = Some(EmbedFooter::new(text));
        self
    }

    /// Sets the footer text and icon.
    #[must_use]
    pub fn footer_with_icon(
        mut self,
        text: impl Into<String>,
        icon_url: impl Into<String>,
    ) -> Self {
        let mut footer = EmbedFooter::new(text);
        footer.icon_url = Some(icon_url.into());
        self.footer = Some(footer);
        self
    }

    /// Sets the image from a URL or an `attachment://` reference.
    #[must_use]
    pub fn image(mut self, url: impl Into<String>) -> Self {
        self.image = Some(EmbedMedia::new(url));
        self
    }

    /// Sets the thumbnail from a URL or an `attachment://` reference.
    #[must_use]
    pub fn thumbnail(mut self, url: impl Into<String>) -> Self {
        self.thumbnail = Some(EmbedMedia::new(url));
        self
    }

    /// Sets the author name.
    #[must_use]
    pub fn author(mut self, name: impl Into<String>) -> Self {
        self.author = Some(EmbedAuthor::new(name));
        self
    }

    /// Sets the author.
    #[must_use]
    pub fn author_full(mut self, author: EmbedAuthor) -> Self {
        self.author = Some(author);
        self
    }

    /// Adds a field.
    #[must_use]
    pub fn field(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
        inline: bool,
    ) -> Self {
        self.fields.push(EmbedField {
            name: name.into(),
            value: value.into(),
            inline: inline.then_some(true),
        });
        self
    }

    /// Counts the characters that count toward the limit of 6000 for all embeds of a message.
    #[must_use]
    pub fn character_count(&self) -> usize {
        fn count(text: Option<&str>) -> usize {
            text.map_or(0, |text| text.trim().chars().count())
        }

        let fields: usize = self
            .fields
            .iter()
            .map(|field| count(Some(&field.name)) + count(Some(&field.value)))
            .sum();

        count(self.title.as_deref())
            + count(self.description.as_deref())
            + fields
            + count(self.footer.as_ref().map(|footer| footer.text.as_str()))
            + count(self.author.as_ref().map(|author| author.name.as_str()))
    }
}

crate::discord_string_enum! {
    /// The type of an embed.
    pub enum EmbedType {
        /// A generic embed. Embeds that bots send have this type.
        Rich = "rich",
        /// An image.
        Image = "image",
        /// A video.
        Video = "video",
        /// An animated GIF that shows as a video.
        Gifv = "gifv",
        /// An article.
        Article = "article",
        /// A link.
        Link = "link",
        /// The result of a poll.
        PollResult = "poll_result",
        /// A link preview made from components.
        Components = "components",
    }
}

crate::discord_flags! {
    /// Flags of an embed.
    pub struct EmbedFlags: u64 as integer {
        /// Content scanning flagged the media as explicit.
        const CONTAINS_EXPLICIT_CONTENT = 1 << 4;
        /// The embed is a fallback for a reply to an activity card.
        const IS_CONTENT_INVENTORY_ENTRY = 1 << 5;
        /// Content scanning flagged the media as gore.
        const CONTAINS_GORE_CONTENT = 1 << 6;
        /// Content scanning flagged the media as self-harm.
        const CONTAINS_SELF_HARM_CONTENT = 1 << 7;
    }
}

crate::discord_flags! {
    /// Flags of embed media.
    pub struct EmbedMediaFlags: u64 as integer {
        /// The image is animated.
        const IS_ANIMATED = 1 << 5;
    }
}

/// An image, a thumbnail, or a video of an embed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct EmbedMedia {
    /// The source URL. Only `http(s)` and `attachment://` are valid in requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The proxied URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_url: Option<String>,
    /// The height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// The width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// The media type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// The thumbhash placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// The version of the placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder_version: Option<u32>,
    /// The alt text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The media flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<EmbedMediaFlags>,
}

impl EmbedMedia {
    /// Makes media from a URL.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: Some(url.into()),
            ..Self::default()
        }
    }
}

/// The provider of an embed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct EmbedProvider {
    /// The name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// The author of an embed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct EmbedAuthor {
    /// The name.
    pub name: String,
    /// The URL. Only `http(s)` is valid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The icon URL. Only `http(s)` and `attachment://` are valid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// The proxied icon URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_icon_url: Option<String>,
}

impl EmbedAuthor {
    /// Makes an author with a name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Sets the URL.
    #[must_use]
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Sets the icon URL.
    #[must_use]
    pub fn icon_url(mut self, icon_url: impl Into<String>) -> Self {
        self.icon_url = Some(icon_url.into());
        self
    }
}

/// The footer of an embed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct EmbedFooter {
    /// The text.
    pub text: String,
    /// The icon URL. Only `http(s)` and `attachment://` are valid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// The proxied icon URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_icon_url: Option<String>,
}

impl EmbedFooter {
    /// Makes a footer with text.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }
}

/// A field of an embed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct EmbedField {
    /// The name.
    pub name: String,
    /// The value.
    pub value: String,
    /// Tells if the field shows inline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline: Option<bool>,
}

impl EmbedField {
    /// Makes a field.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>, inline: bool) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            inline: inline.then_some(true),
        }
    }
}
