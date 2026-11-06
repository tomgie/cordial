//! Message and modal components, including Components V2.
//!
//! A [`Component`] is a tagged union on the integer field `type`. Unknown component types
//! decode to [`Component::Unknown`] and encode back without loss.

use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Color;
use crate::channel::ChannelType;
use crate::emoji::PartialEmoji;
use crate::id::{AttachmentId, GenericId, SkuId};

crate::discord_enum! {
    /// The type of a component.
    pub enum ComponentType: u8 {
        /// A row of interactive components.
        ActionRow = 1,
        /// A button.
        Button = 2,
        /// A select menu with text options.
        StringSelect = 3,
        /// A text input (modals).
        TextInput = 4,
        /// A select menu for users.
        UserSelect = 5,
        /// A select menu for roles.
        RoleSelect = 6,
        /// A select menu for users and roles.
        MentionableSelect = 7,
        /// A select menu for channels.
        ChannelSelect = 8,
        /// Text with an accessory.
        Section = 9,
        /// Markdown text.
        TextDisplay = 10,
        /// A small image.
        Thumbnail = 11,
        /// A gallery of media.
        MediaGallery = 12,
        /// An attached file.
        File = 13,
        /// Vertical space between components.
        Separator = 14,
        /// A group of components.
        Container = 17,
        /// A label for a component in a modal.
        Label = 18,
        /// A file upload (modals).
        FileUpload = 19,
        /// A group of options with one choice (modals).
        RadioGroup = 21,
        /// A group of checkboxes (modals).
        CheckboxGroup = 22,
        /// A checkbox (modals).
        Checkbox = 23,
    }
}

/// A component in a message or a modal.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Component {
    /// A row of up to 5 buttons or one select menu.
    ActionRow(ActionRow),
    /// A button.
    Button(Button),
    /// A select menu with text options.
    StringSelect(StringSelect),
    /// A text input. Deprecated directly in an action row: put it in a [`Label`].
    TextInput(TextInput),
    /// A select menu for users.
    UserSelect(SelectMenu),
    /// A select menu for roles.
    RoleSelect(SelectMenu),
    /// A select menu for users and roles.
    MentionableSelect(SelectMenu),
    /// A select menu for channels.
    ChannelSelect(SelectMenu),
    /// Text with an accessory.
    Section(Section),
    /// Markdown text.
    TextDisplay(TextDisplay),
    /// A small image.
    Thumbnail(Thumbnail),
    /// A gallery of media.
    MediaGallery(MediaGallery),
    /// An attached file.
    File(FileComponent),
    /// Vertical space between components.
    Separator(Separator),
    /// A group of components.
    Container(Container),
    /// A label for a component in a modal.
    Label(Label),
    /// A file upload.
    FileUpload(FileUpload),
    /// A group of options with one choice.
    RadioGroup(RadioGroup),
    /// A group of checkboxes.
    CheckboxGroup(CheckboxGroup),
    /// A checkbox.
    Checkbox(Checkbox),
    /// A component type that this version of the library does not know.
    Unknown(UnknownComponent),
}

/// A component of a type that this version of the library does not know.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct UnknownComponent {
    /// The component type.
    pub kind: ComponentType,
    /// The full JSON object of the component, with the `type` field.
    pub raw: Value,
}

impl Component {
    /// Gives the component type.
    #[must_use]
    pub fn kind(&self) -> ComponentType {
        match self {
            Self::ActionRow(_) => ComponentType::ActionRow,
            Self::Button(_) => ComponentType::Button,
            Self::StringSelect(_) => ComponentType::StringSelect,
            Self::TextInput(_) => ComponentType::TextInput,
            Self::UserSelect(_) => ComponentType::UserSelect,
            Self::RoleSelect(_) => ComponentType::RoleSelect,
            Self::MentionableSelect(_) => ComponentType::MentionableSelect,
            Self::ChannelSelect(_) => ComponentType::ChannelSelect,
            Self::Section(_) => ComponentType::Section,
            Self::TextDisplay(_) => ComponentType::TextDisplay,
            Self::Thumbnail(_) => ComponentType::Thumbnail,
            Self::MediaGallery(_) => ComponentType::MediaGallery,
            Self::File(_) => ComponentType::File,
            Self::Separator(_) => ComponentType::Separator,
            Self::Container(_) => ComponentType::Container,
            Self::Label(_) => ComponentType::Label,
            Self::FileUpload(_) => ComponentType::FileUpload,
            Self::RadioGroup(_) => ComponentType::RadioGroup,
            Self::CheckboxGroup(_) => ComponentType::CheckboxGroup,
            Self::Checkbox(_) => ComponentType::Checkbox,
            Self::Unknown(unknown) => unknown.kind,
        }
    }

    /// Gives the custom ID of an interactive component.
    #[must_use]
    pub fn custom_id(&self) -> Option<&str> {
        match self {
            Self::Button(button) => button.custom_id.as_deref(),
            Self::StringSelect(select) => Some(&select.custom_id),
            Self::TextInput(input) => Some(&input.custom_id),
            Self::UserSelect(select)
            | Self::RoleSelect(select)
            | Self::MentionableSelect(select)
            | Self::ChannelSelect(select) => Some(&select.custom_id),
            Self::FileUpload(upload) => Some(&upload.custom_id),
            Self::RadioGroup(group) => Some(&group.custom_id),
            Self::CheckboxGroup(group) => Some(&group.custom_id),
            Self::Checkbox(checkbox) => Some(&checkbox.custom_id),
            _ => None,
        }
    }

    /// Gives the direct child components.
    #[must_use]
    pub fn children(&self) -> Vec<&Component> {
        match self {
            Self::ActionRow(row) => row.components.iter().collect(),
            Self::Section(section) => section
                .components
                .iter()
                .chain(std::iter::once(section.accessory.as_ref()))
                .collect(),
            Self::Container(container) => container.components.iter().collect(),
            Self::Label(label) => vec![label.component.as_ref()],
            _ => Vec::new(),
        }
    }

    /// Counts this component and all nested components.
    ///
    /// Discord permits at most 40 in a Components V2 message.
    #[must_use]
    pub fn count_recursive(&self) -> usize {
        1 + self
            .children()
            .into_iter()
            .map(Component::count_recursive)
            .sum::<usize>()
    }
}

/// Encodes a component struct with its `type` field.
#[derive(Serialize)]
struct Tagged<'a, T: Serialize> {
    #[serde(rename = "type")]
    kind: ComponentType,
    #[serde(flatten)]
    inner: &'a T,
}

impl Serialize for Component {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let kind = self.kind();
        match self {
            Self::ActionRow(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::Button(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::StringSelect(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::TextInput(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::UserSelect(inner)
            | Self::RoleSelect(inner)
            | Self::MentionableSelect(inner)
            | Self::ChannelSelect(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::Section(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::TextDisplay(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::Thumbnail(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::MediaGallery(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::File(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::Separator(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::Container(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::Label(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::FileUpload(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::RadioGroup(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::CheckboxGroup(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::Checkbox(inner) => Tagged { kind, inner }.serialize(serializer),
            Self::Unknown(unknown) => unknown.raw.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Component {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let kind = value
            .get("type")
            .and_then(Value::as_u64)
            .and_then(|kind| u8::try_from(kind).ok())
            .map(ComponentType::from_value)
            .ok_or_else(|| de::Error::missing_field("type"))?;

        decode_component(kind, value).map_err(de::Error::custom)
    }
}

fn decode_component(kind: ComponentType, value: Value) -> Result<Component, serde_json::Error> {
    use serde_json::from_value;

    Ok(match kind {
        ComponentType::ActionRow => Component::ActionRow(from_value(value)?),
        ComponentType::Button => Component::Button(from_value(value)?),
        ComponentType::StringSelect => Component::StringSelect(from_value(value)?),
        ComponentType::TextInput => Component::TextInput(from_value(value)?),
        ComponentType::UserSelect => Component::UserSelect(from_value(value)?),
        ComponentType::RoleSelect => Component::RoleSelect(from_value(value)?),
        ComponentType::MentionableSelect => Component::MentionableSelect(from_value(value)?),
        ComponentType::ChannelSelect => Component::ChannelSelect(from_value(value)?),
        ComponentType::Section => Component::Section(from_value(value)?),
        ComponentType::TextDisplay => Component::TextDisplay(from_value(value)?),
        ComponentType::Thumbnail => Component::Thumbnail(from_value(value)?),
        ComponentType::MediaGallery => Component::MediaGallery(from_value(value)?),
        ComponentType::File => Component::File(from_value(value)?),
        ComponentType::Separator => Component::Separator(from_value(value)?),
        ComponentType::Container => Component::Container(from_value(value)?),
        ComponentType::Label => Component::Label(from_value(value)?),
        ComponentType::FileUpload => Component::FileUpload(from_value(value)?),
        ComponentType::RadioGroup => Component::RadioGroup(from_value(value)?),
        ComponentType::CheckboxGroup => Component::CheckboxGroup(from_value(value)?),
        ComponentType::Checkbox => Component::Checkbox(from_value(value)?),
        ComponentType::Unknown(_) => Component::Unknown(UnknownComponent { kind, raw: value }),
    })
}

macro_rules! component_from {
    ($($variant:ident($inner:ty),)*) => {
        $(
            impl From<$inner> for Component {
                fn from(inner: $inner) -> Self {
                    Self::$variant(inner)
                }
            }
        )*
    };
}

component_from! {
    ActionRow(ActionRow),
    Button(Button),
    StringSelect(StringSelect),
    TextInput(TextInput),
    Section(Section),
    TextDisplay(TextDisplay),
    Thumbnail(Thumbnail),
    MediaGallery(MediaGallery),
    File(FileComponent),
    Separator(Separator),
    Container(Container),
    Label(Label),
    FileUpload(FileUpload),
    RadioGroup(RadioGroup),
    CheckboxGroup(CheckboxGroup),
    Checkbox(Checkbox),
}

/// A row of up to 5 buttons or one select menu.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ActionRow {
    /// The component identifier, unique in the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The child components.
    #[serde(default)]
    pub components: Vec<Component>,
}

impl ActionRow {
    /// Makes a row with child components.
    #[must_use]
    pub fn new(components: impl IntoIterator<Item = impl Into<Component>>) -> Self {
        Self {
            id: None,
            components: components.into_iter().map(Into::into).collect(),
        }
    }
}

/// A button.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Button {
    /// The component identifier, unique in the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The style.
    pub style: ButtonStyle,
    /// The text, up to 80 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The emoji.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<PartialEmoji>,
    /// The custom ID, 1 to 100 characters. Required except for link and premium buttons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_id: Option<String>,
    /// The SKU of a premium button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sku_id: Option<SkuId>,
    /// The URL of a link button, up to 512 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Tells if the button is disabled.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub disabled: bool,
}

impl Button {
    /// Makes a button that sends an interaction.
    #[must_use]
    pub fn new(style: ButtonStyle, custom_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: None,
            style,
            label: Some(label.into()),
            emoji: None,
            custom_id: Some(custom_id.into()),
            sku_id: None,
            url: None,
            disabled: false,
        }
    }

    /// Makes a link button.
    #[must_use]
    pub fn link(url: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: None,
            style: ButtonStyle::Link,
            label: Some(label.into()),
            emoji: None,
            custom_id: None,
            sku_id: None,
            url: Some(url.into()),
            disabled: false,
        }
    }

    /// Makes a premium button that shows a SKU and its price.
    #[must_use]
    pub fn premium(sku_id: SkuId) -> Self {
        Self {
            id: None,
            style: ButtonStyle::Premium,
            label: None,
            emoji: None,
            custom_id: None,
            sku_id: Some(sku_id),
            url: None,
            disabled: false,
        }
    }

    /// Sets the emoji.
    #[must_use]
    pub fn emoji(mut self, emoji: PartialEmoji) -> Self {
        self.emoji = Some(emoji);
        self
    }

    /// Sets whether the button is disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

crate::discord_enum! {
    /// The style of a button.
    pub enum ButtonStyle: u8 {
        /// The most important action. Needs a custom ID.
        Primary = 1,
        /// A secondary action. Needs a custom ID.
        Secondary = 2,
        /// A positive action. Needs a custom ID.
        Success = 3,
        /// An action that cannot be undone. Needs a custom ID.
        Danger = 4,
        /// A link to a URL. Needs a URL.
        Link = 5,
        /// A purchase. Needs a SKU ID.
        Premium = 6,
    }
}

/// A select menu with text options.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct StringSelect {
    /// The component identifier, unique in the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The custom ID, 1 to 100 characters.
    pub custom_id: String,
    /// The options, up to 25.
    #[serde(default)]
    pub options: Vec<SelectOption>,
    /// The placeholder text, up to 150 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// The minimum number of choices, 0 to 25. The default is 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_values: Option<u8>,
    /// The maximum number of choices, up to 25. The default is 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_values: Option<u8>,
    /// In a modal: tells if a choice is required. The default is `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// In a message: tells if the menu is disabled.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub disabled: bool,
}

impl StringSelect {
    /// Makes a select menu with options.
    #[must_use]
    pub fn new(custom_id: impl Into<String>, options: Vec<SelectOption>) -> Self {
        Self {
            id: None,
            custom_id: custom_id.into(),
            options,
            placeholder: None,
            min_values: None,
            max_values: None,
            required: None,
            disabled: false,
        }
    }
}

/// An option of a string select menu.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct SelectOption {
    /// The text that users see, up to 100 characters.
    pub label: String,
    /// The value that the bot gets, up to 100 characters.
    pub value: String,
    /// The description, up to 100 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The emoji.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<PartialEmoji>,
    /// Tells if the option is selected by default.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub default: bool,
}

impl SelectOption {
    /// Makes an option.
    #[must_use]
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            description: None,
            emoji: None,
            default: false,
        }
    }
}

/// A select menu for users, roles, mentionables, or channels.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct SelectMenu {
    /// The component identifier, unique in the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The custom ID, 1 to 100 characters.
    pub custom_id: String,
    /// For a channel select: the channel types to show.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_types: Option<Vec<ChannelType>>,
    /// The placeholder text, up to 150 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// The values that are selected by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_values: Option<Vec<SelectDefaultValue>>,
    /// The minimum number of choices, 0 to 25. The default is 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_values: Option<u8>,
    /// The maximum number of choices, up to 25. The default is 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_values: Option<u8>,
    /// In a modal: tells if a choice is required. The default is `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// In a message: tells if the menu is disabled.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub disabled: bool,
}

impl SelectMenu {
    /// Makes a select menu.
    #[must_use]
    pub fn new(custom_id: impl Into<String>) -> Self {
        Self {
            id: None,
            custom_id: custom_id.into(),
            channel_types: None,
            placeholder: None,
            default_values: None,
            min_values: None,
            max_values: None,
            required: None,
            disabled: false,
        }
    }
}

/// A value that is selected by default in an auto-filled select menu.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct SelectDefaultValue {
    /// The ID of the user, role, or channel.
    pub id: GenericId,
    /// The kind of the ID.
    #[serde(rename = "type")]
    pub kind: SelectDefaultValueType,
}

crate::discord_string_enum! {
    /// The kind of a default value of a select menu.
    pub enum SelectDefaultValueType {
        /// A user.
        User = "user",
        /// A role.
        Role = "role",
        /// A channel.
        Channel = "channel",
    }
}

/// A text input in a modal.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct TextInput {
    /// The component identifier, unique in the modal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The custom ID, 1 to 100 characters.
    pub custom_id: String,
    /// The style.
    pub style: TextInputStyle,
    /// The label. Deprecated: put the text input in a [`Label`] instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The minimum length, 0 to 4000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u16>,
    /// The maximum length, 1 to 4000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u16>,
    /// Tells if the input is required. The default is `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// The initial value, up to 4000 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// The placeholder text, up to 100 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

impl TextInput {
    /// Makes a text input.
    #[must_use]
    pub fn new(custom_id: impl Into<String>, style: TextInputStyle) -> Self {
        Self {
            id: None,
            custom_id: custom_id.into(),
            style,
            label: None,
            min_length: None,
            max_length: None,
            required: None,
            value: None,
            placeholder: None,
        }
    }
}

crate::discord_enum! {
    /// The style of a text input.
    pub enum TextInputStyle: u8 {
        /// One line.
        Short = 1,
        /// Many lines.
        Paragraph = 2,
    }
}

/// Text with an accessory (Components V2).
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Section {
    /// The component identifier, unique in the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// One to three text displays.
    pub components: Vec<Component>,
    /// The accessory: a button or a thumbnail.
    pub accessory: Box<Component>,
}

impl Section {
    /// Makes a section.
    #[must_use]
    pub fn new(components: Vec<Component>, accessory: impl Into<Component>) -> Self {
        Self {
            id: None,
            components,
            accessory: Box::new(accessory.into()),
        }
    }
}

/// Markdown text (Components V2).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct TextDisplay {
    /// The component identifier, unique in the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The markdown text.
    pub content: String,
}

impl TextDisplay {
    /// Makes a text display.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            id: None,
            content: content.into(),
        }
    }
}

/// A small image (Components V2).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Thumbnail {
    /// The component identifier, unique in the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The image.
    pub media: UnfurledMediaItem,
    /// The alt text, up to 1024 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tells if the image is a spoiler.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub spoiler: bool,
}

impl Thumbnail {
    /// Makes a thumbnail from a URL or an `attachment://` reference.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            id: None,
            media: UnfurledMediaItem::new(url),
            description: None,
            spoiler: false,
        }
    }
}

/// A gallery of 1 to 10 media items (Components V2).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MediaGallery {
    /// The component identifier, unique in the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The items.
    pub items: Vec<MediaGalleryItem>,
}

impl MediaGallery {
    /// Makes a gallery.
    #[must_use]
    pub fn new(items: Vec<MediaGalleryItem>) -> Self {
        Self { id: None, items }
    }
}

/// An item of a media gallery.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MediaGalleryItem {
    /// The media.
    pub media: UnfurledMediaItem,
    /// The alt text, up to 1024 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tells if the media is a spoiler.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub spoiler: bool,
}

impl MediaGalleryItem {
    /// Makes an item from a URL or an `attachment://` reference.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            media: UnfurledMediaItem::new(url),
            description: None,
            spoiler: false,
        }
    }
}

/// An attached file (Components V2).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct FileComponent {
    /// The component identifier, unique in the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The file. Only `attachment://<filename>` references are valid.
    pub file: UnfurledMediaItem,
    /// Tells if the file is a spoiler.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub spoiler: bool,
    /// The file name. Discord sets it in responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The file size in bytes. Discord sets it in responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

impl FileComponent {
    /// Makes a file component for an uploaded file, for example `attachment://report.pdf`.
    #[must_use]
    pub fn attachment(filename: &str) -> Self {
        Self {
            id: None,
            file: UnfurledMediaItem::new(format!("attachment://{filename}")),
            spoiler: false,
            name: None,
            size: None,
        }
    }
}

/// Vertical space between components (Components V2).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Separator {
    /// The component identifier, unique in the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// Tells if a visible line shows. The default is `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divider: Option<bool>,
    /// The size of the space.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacing: Option<SeparatorSpacing>,
}

crate::discord_enum! {
    /// The size of the space of a separator.
    pub enum SeparatorSpacing: u8 {
        /// Small space.
        Small = 1,
        /// Large space.
        Large = 2,
    }
}

/// A group of components with an optional accent color (Components V2).
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Container {
    /// The component identifier, unique in the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The child components: action rows, text displays, sections, galleries, separators, files.
    pub components: Vec<Component>,
    /// The accent color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent_color: Option<Color>,
    /// Tells if the container is a spoiler.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub spoiler: bool,
}

impl Container {
    /// Makes a container.
    #[must_use]
    pub fn new(components: Vec<Component>) -> Self {
        Self {
            id: None,
            components,
            accent_color: None,
            spoiler: false,
        }
    }
}

/// A label and a description for one component in a modal.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Label {
    /// The component identifier, unique in the modal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The label text, up to 45 characters.
    pub label: String,
    /// The description, up to 100 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The component in the label.
    pub component: Box<Component>,
}

impl Label {
    /// Makes a label for a component.
    #[must_use]
    pub fn new(label: impl Into<String>, component: impl Into<Component>) -> Self {
        Self {
            id: None,
            label: label.into(),
            description: None,
            component: Box::new(component.into()),
        }
    }
}

/// A file upload in a modal.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct FileUpload {
    /// The component identifier, unique in the modal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The custom ID, 1 to 100 characters.
    pub custom_id: String,
    /// The minimum number of files, 0 to 10. The default is 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_values: Option<u8>,
    /// The maximum number of files, up to 10. The default is 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_values: Option<u8>,
    /// Tells if files are required. The default is `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// The file types: `image`, `video`, `audio`, or extensions such as `.pdf`. Up to 10.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_types: Option<Vec<String>>,
}

/// A group of 2 to 10 options with one choice, in a modal.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct RadioGroup {
    /// The component identifier, unique in the modal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The custom ID, 1 to 100 characters.
    pub custom_id: String,
    /// The options.
    pub options: Vec<ChoiceOption>,
    /// Tells if a choice is required. The default is `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

/// A group of 1 to 10 checkboxes, in a modal.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct CheckboxGroup {
    /// The component identifier, unique in the modal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The custom ID, 1 to 100 characters.
    pub custom_id: String,
    /// The options.
    pub options: Vec<ChoiceOption>,
    /// The minimum number of choices, 0 to 10. The default is 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_values: Option<u8>,
    /// The maximum number of choices, 1 to 10. The default is the number of options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_values: Option<u8>,
    /// Tells if a choice is required. The default is `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

/// An option of a radio group or a checkbox group.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ChoiceOption {
    /// The value that the bot gets, up to 100 characters.
    pub value: String,
    /// The text that users see, up to 100 characters.
    pub label: String,
    /// The description, up to 100 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tells if the option is selected by default.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub default: bool,
}

impl ChoiceOption {
    /// Makes an option.
    #[must_use]
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            description: None,
            default: false,
        }
    }
}

/// A checkbox in a modal.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Checkbox {
    /// The component identifier, unique in the modal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The custom ID, 1 to 100 characters.
    pub custom_id: String,
    /// Tells if the checkbox is checked by default.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub default: bool,
}

/// A media item: a URL or an `attachment://` reference, with data that Discord adds.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct UnfurledMediaItem {
    /// The URL or the `attachment://<filename>` reference. The only field that requests set.
    pub url: String,
    /// The proxied URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_url: Option<String>,
    /// The height, for images and videos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// The width, for images and videos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// The thumbhash placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// The version of the placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder_version: Option<u32>,
    /// The media type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// The media flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<UnfurledMediaFlags>,
    /// The results of content scanning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_scan_metadata: Option<ContentScanMetadata>,
    /// The ID of the uploaded attachment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment_id: Option<AttachmentId>,
}

impl UnfurledMediaItem {
    /// Makes a media item for a request.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            proxy_url: None,
            height: None,
            width: None,
            placeholder: None,
            placeholder_version: None,
            content_type: None,
            flags: None,
            content_scan_metadata: None,
            attachment_id: None,
        }
    }
}

crate::discord_flags! {
    /// Flags of an unfurled media item.
    pub struct UnfurledMediaFlags: u64 as integer {
        /// The image is animated.
        const IS_ANIMATED = 1 << 0;
    }
}

/// The results of content scanning of a media item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ContentScanMetadata {
    /// The scan flags.
    #[serde(default)]
    pub flags: ContentScanFlags,
}

crate::discord_flags! {
    /// Flags of content scanning. The bits differ from the attachment and embed flags.
    pub struct ContentScanFlags: u64 as integer {
        /// The media is explicit.
        const IS_EXPLICIT_CONTENT = 1 << 0;
        /// The media is gore.
        const IS_GORE_CONTENT = 1 << 1;
        /// The media is self-harm.
        const IS_SELF_HARM_CONTENT = 1 << 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_unknown_component() {
        let json = r#"{"type":99,"id":4,"secret":"field"}"#;

        let component: Component = serde_json::from_str(json).unwrap();

        assert_eq!(component.kind(), ComponentType::Unknown(99));
        let original: Value = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_value(&component).unwrap(), original);
    }

    #[test]
    fn encodes_type_field() {
        let row = ActionRow::new([Button::new(ButtonStyle::Primary, "yes", "Yes")]);

        let json = serde_json::to_value(Component::from(row)).unwrap();

        assert_eq!(json["type"], 1);
        assert_eq!(json["components"][0]["type"], 2);
        assert_eq!(json["components"][0]["custom_id"], "yes");
    }

    #[test]
    fn counts_nested_components() {
        let container = Container::new(vec![
            TextDisplay::new("a").into(),
            ActionRow::new([
                Button::new(ButtonStyle::Primary, "one", "One"),
                Button::new(ButtonStyle::Danger, "two", "Two"),
            ])
            .into(),
        ]);

        assert_eq!(Component::from(container).count_recursive(), 5);
    }
}
