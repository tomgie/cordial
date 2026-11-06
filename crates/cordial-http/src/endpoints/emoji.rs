//! Emoji endpoints: guild emojis and application emojis.
//!
//! Discord limits the guild emoji routes for each guild in a special way. The rate limit
//! headers can be wrong for these routes, and a 429 response can come without a warning.

use cordial_model::Patch;
use cordial_model::emoji::{ApplicationEmojis, Emoji};
use cordial_model::id::{ApplicationId, EmojiId, GuildId, RoleId};
use cordial_model::validate::ValidationError;
use serde::Serialize;

use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, ImageData, RequestBody, check};
use crate::route::{MajorParameter, Method, Route};

/// The maximum size of an emoji image in bytes (256 KiB).
const IMAGE_SIZE_LIMIT: usize = 256 * 1024;

/// Checks the size of an emoji image. Discord gives no JSON error code for a large image.
fn check_image(image: &ImageData) -> Result<(), ValidationError> {
    let encoded = image.as_str().split_once(',').map_or("", |(_, data)| data);
    let padding = encoded
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'=')
        .count();
    let size = (encoded.len() / 4 * 3).saturating_sub(padding);
    if size > IMAGE_SIZE_LIMIT {
        return Err(ValidationError::new(
            "image",
            format!("has {size} bytes; the maximum is {IMAGE_SIZE_LIMIT}"),
        ));
    }
    Ok(())
}

/// `GET /guilds/{guild.id}/emojis`: the emojis of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildEmojis {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildEmojis {
    type Response = Vec<Emoji>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/emojis",
            format!("/guilds/{}/emojis", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/emojis/{emoji.id}`: one emoji of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildEmoji {
    client: Client,
    guild_id: GuildId,
    emoji_id: EmojiId,
}

impl Endpoint for GetGuildEmoji {
    type Response = Emoji;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/emojis/{emoji_id}",
            format!("/guilds/{}/emojis/{}", self.guild_id, self.emoji_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `POST /guilds/{guild.id}/emojis`: creates an emoji in a guild.
///
/// The image is 128x128 pixels and at most 256 KiB.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateGuildEmoji {
    client: Client,
    guild_id: GuildId,
    body: CreateGuildEmojiBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct CreateGuildEmojiBody {
    name: String,
    image: ImageData,
    #[serde(skip_serializing_if = "Option::is_none")]
    roles: Option<Vec<RoleId>>,
}

impl CreateGuildEmoji {
    /// Sets the roles that can use the emoji.
    pub fn roles(mut self, roles: Vec<RoleId>) -> Self {
        self.body.roles = Some(roles);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for CreateGuildEmoji {
    type Response = Emoji;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/guilds/{guild_id}/emojis",
            format!("/guilds/{}/emojis", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || check_image(&self.body.image))?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `PATCH /guilds/{guild.id}/emojis/{emoji.id}`: edits an emoji of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuildEmoji {
    client: Client,
    guild_id: GuildId,
    emoji_id: EmojiId,
    body: EditGuildEmojiBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditGuildEmojiBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    roles: Patch<Vec<RoleId>>,
}

impl EditGuildEmoji {
    /// Sets the name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.body.name = Some(name.into());
        self
    }

    /// Sets the roles that can use the emoji.
    pub fn roles(mut self, roles: Vec<RoleId>) -> Self {
        self.body.roles = Patch::Set(roles);
        self
    }

    /// Removes the role limit. All members can then use the emoji.
    pub fn clear_roles(mut self) -> Self {
        self.body.roles = Patch::Clear;
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditGuildEmoji {
    type Response = Emoji;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/emojis/{emoji_id}",
            format!("/guilds/{}/emojis/{}", self.guild_id, self.emoji_id),
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

/// `DELETE /guilds/{guild.id}/emojis/{emoji.id}`: deletes an emoji of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteGuildEmoji {
    client: Client,
    guild_id: GuildId,
    emoji_id: EmojiId,
    reason: Option<String>,
}

impl DeleteGuildEmoji {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for DeleteGuildEmoji {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/guilds/{guild_id}/emojis/{emoji_id}",
            format!("/guilds/{}/emojis/{}", self.guild_id, self.emoji_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `GET /applications/{application.id}/emojis`: the emojis of an application.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetApplicationEmojis {
    client: Client,
    application_id: ApplicationId,
}

impl Endpoint for GetApplicationEmojis {
    type Response = ApplicationEmojis;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/applications/{application_id}/emojis",
            format!("/applications/{}/emojis", self.application_id),
            MajorParameter::None,
        )
    }
}

/// `GET /applications/{application.id}/emojis/{emoji.id}`: one emoji of an application.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetApplicationEmoji {
    client: Client,
    application_id: ApplicationId,
    emoji_id: EmojiId,
}

impl Endpoint for GetApplicationEmoji {
    type Response = Emoji;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/applications/{application_id}/emojis/{emoji_id}",
            format!(
                "/applications/{}/emojis/{}",
                self.application_id, self.emoji_id
            ),
            MajorParameter::None,
        )
    }
}

/// `POST /applications/{application.id}/emojis`: creates an emoji of an application.
///
/// The image is 128x128 pixels and at most 256 KiB. An application can have up to 2000
/// emojis.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateApplicationEmoji {
    client: Client,
    application_id: ApplicationId,
    body: CreateApplicationEmojiBody,
}

#[derive(Clone, Debug, Serialize)]
struct CreateApplicationEmojiBody {
    name: String,
    image: ImageData,
}

impl Endpoint for CreateApplicationEmoji {
    type Response = Emoji;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/applications/{application_id}/emojis",
            format!("/applications/{}/emojis", self.application_id),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || check_image(&self.body.image))?;
        RequestBody::json(&self.body)
    }
}

/// `PATCH /applications/{application.id}/emojis/{emoji.id}`: edits an emoji of an
/// application.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditApplicationEmoji {
    client: Client,
    application_id: ApplicationId,
    emoji_id: EmojiId,
    body: EditApplicationEmojiBody,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditApplicationEmojiBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

impl EditApplicationEmoji {
    /// Sets the name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.body.name = Some(name.into());
        self
    }
}

impl Endpoint for EditApplicationEmoji {
    type Response = Emoji;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/applications/{application_id}/emojis/{emoji_id}",
            format!(
                "/applications/{}/emojis/{}",
                self.application_id, self.emoji_id
            ),
            MajorParameter::None,
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }
}

/// `DELETE /applications/{application.id}/emojis/{emoji.id}`: deletes an emoji of an
/// application.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteApplicationEmoji {
    client: Client,
    application_id: ApplicationId,
    emoji_id: EmojiId,
}

impl Endpoint for DeleteApplicationEmoji {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/applications/{application_id}/emojis/{emoji_id}",
            format!(
                "/applications/{}/emojis/{}",
                self.application_id, self.emoji_id
            ),
            MajorParameter::None,
        )
    }
}

super::into_future!(
    GetGuildEmojis,
    GetGuildEmoji,
    CreateGuildEmoji,
    EditGuildEmoji,
    DeleteGuildEmoji,
    GetApplicationEmojis,
    GetApplicationEmoji,
    CreateApplicationEmoji,
    EditApplicationEmoji,
    DeleteApplicationEmoji
);

impl Client {
    /// Gets the emojis of a guild.
    pub fn guild_emojis(&self, guild_id: GuildId) -> GetGuildEmojis {
        GetGuildEmojis {
            client: self.clone(),
            guild_id,
        }
    }

    /// Gets one emoji of a guild.
    pub fn guild_emoji(&self, guild_id: GuildId, emoji_id: EmojiId) -> GetGuildEmoji {
        GetGuildEmoji {
            client: self.clone(),
            guild_id,
            emoji_id,
        }
    }

    /// Creates an emoji in a guild.
    pub fn create_guild_emoji(
        &self,
        guild_id: GuildId,
        name: impl Into<String>,
        image: ImageData,
    ) -> CreateGuildEmoji {
        CreateGuildEmoji {
            client: self.clone(),
            guild_id,
            body: CreateGuildEmojiBody {
                name: name.into(),
                image,
                roles: None,
            },
            reason: None,
        }
    }

    /// Edits an emoji of a guild.
    pub fn edit_guild_emoji(&self, guild_id: GuildId, emoji_id: EmojiId) -> EditGuildEmoji {
        EditGuildEmoji {
            client: self.clone(),
            guild_id,
            emoji_id,
            body: EditGuildEmojiBody::default(),
            reason: None,
        }
    }

    /// Deletes an emoji of a guild.
    pub fn delete_guild_emoji(&self, guild_id: GuildId, emoji_id: EmojiId) -> DeleteGuildEmoji {
        DeleteGuildEmoji {
            client: self.clone(),
            guild_id,
            emoji_id,
            reason: None,
        }
    }

    /// Gets the emojis of an application.
    pub fn application_emojis(&self, application_id: ApplicationId) -> GetApplicationEmojis {
        GetApplicationEmojis {
            client: self.clone(),
            application_id,
        }
    }

    /// Gets one emoji of an application.
    pub fn application_emoji(
        &self,
        application_id: ApplicationId,
        emoji_id: EmojiId,
    ) -> GetApplicationEmoji {
        GetApplicationEmoji {
            client: self.clone(),
            application_id,
            emoji_id,
        }
    }

    /// Creates an emoji of an application.
    pub fn create_application_emoji(
        &self,
        application_id: ApplicationId,
        name: impl Into<String>,
        image: ImageData,
    ) -> CreateApplicationEmoji {
        CreateApplicationEmoji {
            client: self.clone(),
            application_id,
            body: CreateApplicationEmojiBody {
                name: name.into(),
                image,
            },
        }
    }

    /// Edits an emoji of an application.
    pub fn edit_application_emoji(
        &self,
        application_id: ApplicationId,
        emoji_id: EmojiId,
    ) -> EditApplicationEmoji {
        EditApplicationEmoji {
            client: self.clone(),
            application_id,
            emoji_id,
            body: EditApplicationEmojiBody::default(),
        }
    }

    /// Deletes an emoji of an application.
    pub fn delete_application_emoji(
        &self,
        application_id: ApplicationId,
        emoji_id: EmojiId,
    ) -> DeleteApplicationEmoji {
        DeleteApplicationEmoji {
            client: self.clone(),
            application_id,
            emoji_id,
        }
    }
}
