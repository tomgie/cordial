//! Guild setting endpoints: the widget, the welcome screen, onboarding, and the new member
//! welcome.

use bytes::Bytes;
use cordial_model::Patch;
use cordial_model::guild::{
    GuildWidget, GuildWidgetSettings, Onboarding, OnboardingMode, OnboardingPrompt, WelcomeScreen,
    WelcomeScreenChannel,
};
use cordial_model::id::{ChannelId, EmojiId, GuildId, UserId};
use serde::{Deserialize, Serialize};

use super::limits;
use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, RequestBody, check};
use crate::route::{MajorParameter, Method, Query, Route};

/// The maximum number of channels in a welcome screen.
const WELCOME_CHANNEL_LIMIT: usize = 5;

/// `GET /guilds/{guild.id}/widget`: the widget settings of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildWidgetSettings {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildWidgetSettings {
    type Response = GuildWidgetSettings;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/widget",
            format!("/guilds/{}/widget", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `PATCH /guilds/{guild.id}/widget`: changes the widget settings of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuildWidget {
    client: Client,
    guild_id: GuildId,
    body: EditGuildWidgetBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditGuildWidgetBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    channel_id: Patch<ChannelId>,
}

impl EditGuildWidget {
    /// Turns the widget on or off.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.body.enabled = Some(enabled);
        self
    }

    /// Sets the channel that the widget invite points to.
    pub fn channel_id(mut self, channel_id: ChannelId) -> Self {
        self.body.channel_id = Patch::Set(channel_id);
        self
    }

    /// Removes the widget channel.
    pub fn clear_channel_id(mut self) -> Self {
        self.body.channel_id = Patch::Clear;
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditGuildWidget {
    type Response = GuildWidgetSettings;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/widget",
            format!("/guilds/{}/widget", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `GET /guilds/{guild.id}/widget.json`: the public widget of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildWidget {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildWidget {
    type Response = GuildWidget;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/widget.json",
            format!("/guilds/{}/widget.json", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// The style of a widget image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WidgetStyle {
    /// A shield with the Discord icon and the number of online members (the default).
    #[default]
    Shield,
    /// A large image with the guild icon, the name, and the online count. The footer says
    /// "POWERED BY DISCORD".
    Banner1,
    /// A smaller image with the guild icon, the name, and the online count. The Discord logo
    /// is on the right.
    Banner2,
    /// A large image with the guild icon, the name, and the online count. The footer has the
    /// Discord logo and "Chat Now".
    Banner3,
    /// A large Discord logo at the top, the guild data in the middle, and a "JOIN MY SERVER"
    /// button at the bottom.
    Banner4,
}

impl WidgetStyle {
    /// Gives the value of the `style` query parameter.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Shield => "shield",
            Self::Banner1 => "banner1",
            Self::Banner2 => "banner2",
            Self::Banner3 => "banner3",
            Self::Banner4 => "banner4",
        }
    }
}

/// `GET /guilds/{guild.id}/widget.png`: the widget of a guild as a PNG image.
///
/// The response is the PNG bytes, not JSON.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildWidgetImage {
    client: Client,
    guild_id: GuildId,
    style: Option<WidgetStyle>,
}

impl GetGuildWidgetImage {
    /// Sets the style of the image. The default is [`WidgetStyle::Shield`].
    pub fn style(mut self, style: WidgetStyle) -> Self {
        self.style = Some(style);
        self
    }
}

impl Endpoint for GetGuildWidgetImage {
    /// Not used: `.await` gives the raw bytes through [`Client::request_raw`].
    type Response = serde::de::IgnoredAny;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("style", self.style.map(WidgetStyle::as_str));
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/widget.png",
            format!("/guilds/{}/widget.png{}", self.guild_id, query.finish()),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

impl std::future::IntoFuture for GetGuildWidgetImage {
    type Output = Result<Bytes, Error>;
    type IntoFuture = crate::transport::BoxFuture<'static, Self::Output>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.client.request_raw(&self).await })
    }
}

/// `GET /guilds/{guild.id}/welcome-screen`: the welcome screen of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildWelcomeScreen {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildWelcomeScreen {
    type Response = WelcomeScreen;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/welcome-screen",
            format!("/guilds/{}/welcome-screen", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `PATCH /guilds/{guild.id}/welcome-screen`: changes the welcome screen of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuildWelcomeScreen {
    client: Client,
    guild_id: GuildId,
    body: EditGuildWelcomeScreenBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditGuildWelcomeScreenBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    welcome_channels: Patch<Vec<WelcomeScreenChannel>>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    description: Patch<String>,
}

impl EditGuildWelcomeScreen {
    /// Turns the welcome screen on or off.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.body.enabled = Some(enabled);
        self
    }

    /// Sets the channels in the welcome screen, up to 5.
    pub fn welcome_channels(mut self, channels: Vec<WelcomeScreenChannel>) -> Self {
        self.body.welcome_channels = Patch::Set(channels);
        self
    }

    /// Removes all channels from the welcome screen.
    pub fn clear_welcome_channels(mut self) -> Self {
        self.body.welcome_channels = Patch::Clear;
        self
    }

    /// Sets the description of the guild in the welcome screen.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.body.description = Patch::Set(description.into());
        self
    }

    /// Removes the description.
    pub fn clear_description(mut self) -> Self {
        self.body.description = Patch::Clear;
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditGuildWelcomeScreen {
    type Response = WelcomeScreen;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/welcome-screen",
            format!("/guilds/{}/welcome-screen", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            if let Patch::Set(channels) = &self.body.welcome_channels {
                limits::count("welcome_channels", channels.len(), WELCOME_CHANNEL_LIMIT)?;
            }
            Ok(())
        })?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `GET /guilds/{guild.id}/onboarding`: the onboarding flow of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildOnboarding {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildOnboarding {
    type Response = Onboarding;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/onboarding",
            format!("/guilds/{}/onboarding", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `PUT /guilds/{guild.id}/onboarding`: changes the onboarding flow of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuildOnboarding {
    client: Client,
    guild_id: GuildId,
    body: EditGuildOnboardingBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditGuildOnboardingBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    prompts: Option<Vec<OnboardingPrompt>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_channel_ids: Option<Vec<ChannelId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<OnboardingMode>,
}

impl EditGuildOnboarding {
    /// Sets the prompts.
    pub fn prompts(mut self, prompts: Vec<OnboardingPrompt>) -> Self {
        self.body.prompts = Some(prompts);
        self
    }

    /// Sets the channels that members join automatically.
    pub fn default_channel_ids(mut self, channel_ids: Vec<ChannelId>) -> Self {
        self.body.default_channel_ids = Some(channel_ids);
        self
    }

    /// Turns onboarding on or off.
    ///
    /// When it is on, Discord needs at least 7 default channels. At least 5 of them must let
    /// `@everyone` send messages.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.body.enabled = Some(enabled);
        self
    }

    /// Sets the mode. It sets what counts toward the constraints.
    pub fn mode(mut self, mode: OnboardingMode) -> Self {
        self.body.mode = Some(mode);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditGuildOnboarding {
    type Response = Onboarding;

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/guilds/{guild_id}/onboarding",
            format!("/guilds/{}/onboarding", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// The new member welcome of a guild (the Server Guide).
///
/// The Discord docs do not describe it. The fields come from the OpenAPI specification.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct NewMemberWelcome {
    /// The guild ID.
    pub guild_id: GuildId,
    /// Tells if the new member welcome is on.
    pub enabled: bool,
    /// The welcome message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub welcome_message: Option<WelcomeMessage>,
    /// The actions for new members.
    #[serde(default)]
    pub new_member_actions: Vec<NewMemberAction>,
    /// The resource channels.
    #[serde(default)]
    pub resource_channels: Vec<ResourceChannel>,
}

/// The welcome message of a new member welcome.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct WelcomeMessage {
    /// The users that the message shows as authors.
    #[serde(default)]
    pub author_ids: Vec<UserId>,
    /// The text.
    pub message: String,
}

/// An action for new members in a new member welcome.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct NewMemberAction {
    /// The channel of the action.
    pub channel_id: ChannelId,
    /// What the member does in the channel.
    pub action_type: NewMemberActionType,
    /// The title.
    pub title: String,
    /// The description.
    #[serde(default)]
    pub description: String,
    /// The emoji.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<WelcomeEmoji>,
    /// The icon hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// What a new member does for a new member action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(transparent)]
pub struct NewMemberActionType(pub u8);

impl NewMemberActionType {
    /// The member views the channel.
    pub const VIEW: Self = Self(0);
    /// The member sends a message in the channel.
    pub const TALK: Self = Self(1);
}

/// A resource channel in a new member welcome.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ResourceChannel {
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The title.
    pub title: String,
    /// The description.
    #[serde(default)]
    pub description: String,
    /// The emoji.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<WelcomeEmoji>,
    /// The icon hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// An emoji in a new member welcome.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct WelcomeEmoji {
    /// The ID of a custom emoji.
    #[serde(default)]
    pub id: Option<EmojiId>,
    /// The name of a custom emoji, or a Unicode emoji.
    #[serde(default)]
    pub name: Option<String>,
    /// Tells if the emoji is animated.
    #[serde(default)]
    pub animated: bool,
}

/// `GET /guilds/{guild.id}/new-member-welcome`: the new member welcome of a guild.
///
/// The Discord docs do not describe this endpoint. It comes from the OpenAPI specification.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildNewMemberWelcome {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildNewMemberWelcome {
    /// `None` when the guild has no new member welcome (status 204).
    type Response = Option<NewMemberWelcome>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/new-member-welcome",
            format!("/guilds/{}/new-member-welcome", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

super::into_future!(
    GetGuildWidgetSettings,
    EditGuildWidget,
    GetGuildWidget,
    GetGuildWelcomeScreen,
    EditGuildWelcomeScreen,
    GetGuildOnboarding,
    EditGuildOnboarding,
    GetGuildNewMemberWelcome,
);

impl Client {
    /// Gets the widget settings of a guild. Needs `MANAGE_GUILD`.
    pub fn guild_widget_settings(&self, guild_id: GuildId) -> GetGuildWidgetSettings {
        GetGuildWidgetSettings {
            client: self.clone(),
            guild_id,
        }
    }

    /// Changes the widget settings of a guild. Needs `MANAGE_GUILD`.
    pub fn edit_guild_widget(&self, guild_id: GuildId) -> EditGuildWidget {
        EditGuildWidget {
            client: self.clone(),
            guild_id,
            body: EditGuildWidgetBody::default(),
            reason: None,
        }
    }

    /// Gets the public widget of a guild.
    pub fn guild_widget(&self, guild_id: GuildId) -> GetGuildWidget {
        GetGuildWidget {
            client: self.clone(),
            guild_id,
        }
    }

    /// Gets the widget of a guild as PNG bytes.
    pub fn guild_widget_image(&self, guild_id: GuildId) -> GetGuildWidgetImage {
        GetGuildWidgetImage {
            client: self.clone(),
            guild_id,
            style: None,
        }
    }

    /// Gets the welcome screen of a guild. When it is off, this needs `MANAGE_GUILD`.
    pub fn guild_welcome_screen(&self, guild_id: GuildId) -> GetGuildWelcomeScreen {
        GetGuildWelcomeScreen {
            client: self.clone(),
            guild_id,
        }
    }

    /// Changes the welcome screen of a guild. Needs `MANAGE_GUILD`.
    pub fn edit_guild_welcome_screen(&self, guild_id: GuildId) -> EditGuildWelcomeScreen {
        EditGuildWelcomeScreen {
            client: self.clone(),
            guild_id,
            body: EditGuildWelcomeScreenBody::default(),
            reason: None,
        }
    }

    /// Gets the onboarding flow of a guild.
    pub fn guild_onboarding(&self, guild_id: GuildId) -> GetGuildOnboarding {
        GetGuildOnboarding {
            client: self.clone(),
            guild_id,
        }
    }

    /// Changes the onboarding flow of a guild. Needs `MANAGE_GUILD` and `MANAGE_ROLES`.
    pub fn edit_guild_onboarding(&self, guild_id: GuildId) -> EditGuildOnboarding {
        EditGuildOnboarding {
            client: self.clone(),
            guild_id,
            body: EditGuildOnboardingBody::default(),
            reason: None,
        }
    }

    /// Gets the new member welcome (Server Guide) of a guild.
    pub fn guild_new_member_welcome(&self, guild_id: GuildId) -> GetGuildNewMemberWelcome {
        GetGuildNewMemberWelcome {
            client: self.clone(),
            guild_id,
        }
    }
}
