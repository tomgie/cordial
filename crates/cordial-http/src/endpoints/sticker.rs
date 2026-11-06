//! Sticker endpoints: standard stickers, sticker packs, and guild stickers.

use cordial_model::Patch;
use cordial_model::id::{GuildId, StickerId, StickerPackId};
use cordial_model::sticker::{Sticker, StickerPack, StickerPacks};
use cordial_model::validate::ValidationError;
use serde::Serialize;

use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, FileUpload, Multipart, RequestBody, check};
use crate::route::{MajorParameter, Method, Route};

/// The maximum size of a sticker file in bytes (512 KiB).
const FILE_SIZE_LIMIT: usize = 512 * 1024;
/// The maximum number of characters in the tags of a sticker.
const TAGS_LIMIT: usize = 200;

fn check_name(name: &str) -> Result<(), ValidationError> {
    check_length("name", name, 2, 30)
}

/// Checks a description. An empty description is valid only when `allow_empty` is `true`.
fn check_description(description: &str, allow_empty: bool) -> Result<(), ValidationError> {
    if allow_empty && description.is_empty() {
        return Ok(());
    }
    check_length("description", description, 2, 100)
}

fn check_tags(tags: &str) -> Result<(), ValidationError> {
    check_length("tags", tags, 0, TAGS_LIMIT)
}

fn check_length(field: &str, text: &str, min: usize, max: usize) -> Result<(), ValidationError> {
    let count = text.chars().count();
    if !(min..=max).contains(&count) {
        return Err(ValidationError::new(
            field,
            format!("has {count} characters; it must have {min} to {max}"),
        ));
    }
    Ok(())
}

/// `GET /stickers/{sticker.id}`: a sticker.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetSticker {
    client: Client,
    sticker_id: StickerId,
}

impl Endpoint for GetSticker {
    type Response = Sticker;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/stickers/{sticker_id}",
            format!("/stickers/{}", self.sticker_id),
            MajorParameter::None,
        )
    }
}

/// `GET /sticker-packs`: the packs of standard stickers.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetStickerPacks {
    client: Client,
}

impl Endpoint for GetStickerPacks {
    type Response = StickerPacks;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/sticker-packs",
            "/sticker-packs".to_owned(),
            MajorParameter::None,
        )
    }
}

/// `GET /sticker-packs/{pack.id}`: a pack of standard stickers.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetStickerPack {
    client: Client,
    pack_id: StickerPackId,
}

impl Endpoint for GetStickerPack {
    type Response = StickerPack;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/sticker-packs/{pack_id}",
            format!("/sticker-packs/{}", self.pack_id),
            MajorParameter::None,
        )
    }
}

/// `GET /guilds/{guild.id}/stickers`: the stickers of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildStickers {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildStickers {
    type Response = Vec<Sticker>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/stickers",
            format!("/guilds/{}/stickers", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/stickers/{sticker.id}`: one sticker of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildSticker {
    client: Client,
    guild_id: GuildId,
    sticker_id: StickerId,
}

impl Endpoint for GetGuildSticker {
    type Response = Sticker;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/stickers/{sticker_id}",
            format!("/guilds/{}/stickers/{}", self.guild_id, self.sticker_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `POST /guilds/{guild.id}/stickers`: uploads a sticker to a guild.
///
/// The file is a PNG, APNG, GIF, or Lottie JSON file of at most 512 KiB and 320x320 pixels.
/// Only verified or partnered guilds can upload Lottie stickers.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateGuildSticker {
    client: Client,
    guild_id: GuildId,
    name: String,
    description: String,
    tags: String,
    file: FileUpload,
    reason: Option<String>,
}

impl CreateGuildSticker {
    /// Sets the description: empty, or 2 to 100 characters.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for CreateGuildSticker {
    type Response = Sticker;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/guilds/{guild_id}/stickers",
            format!("/guilds/{}/stickers", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            check_name(&self.name)?;
            check_description(&self.description, true)?;
            check_tags(&self.tags)?;
            let size = self.file.data.len();
            if size > FILE_SIZE_LIMIT {
                return Err(ValidationError::new(
                    "file",
                    format!("has {size} bytes; the maximum is {FILE_SIZE_LIMIT}"),
                ));
            }
            Ok(())
        })?;

        let multipart = Multipart::new()
            .field("name", self.name.clone())
            .field("description", self.description.clone())
            .field("tags", self.tags.clone())
            .named_file("file", self.file.clone());
        Ok(RequestBody::Multipart(multipart))
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `PATCH /guilds/{guild.id}/stickers/{sticker.id}`: edits a sticker of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuildSticker {
    client: Client,
    guild_id: GuildId,
    sticker_id: StickerId,
    body: EditGuildStickerBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditGuildStickerBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    description: Patch<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<String>,
}

impl EditGuildSticker {
    /// Sets the name, 2 to 30 characters.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.body.name = Some(name.into());
        self
    }

    /// Sets the description, 2 to 100 characters.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.body.description = Patch::Set(description.into());
        self
    }

    /// Removes the description.
    pub fn clear_description(mut self) -> Self {
        self.body.description = Patch::Clear;
        self
    }

    /// Sets the autocomplete and suggestion tags, up to 200 characters.
    pub fn tags(mut self, tags: impl Into<String>) -> Self {
        self.body.tags = Some(tags.into());
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditGuildSticker {
    type Response = Sticker;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/stickers/{sticker_id}",
            format!("/guilds/{}/stickers/{}", self.guild_id, self.sticker_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            if let Some(name) = &self.body.name {
                check_name(name)?;
            }
            if let Patch::Set(description) = &self.body.description {
                check_description(description, false)?;
            }
            if let Some(tags) = &self.body.tags {
                check_tags(tags)?;
            }
            Ok(())
        })?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `DELETE /guilds/{guild.id}/stickers/{sticker.id}`: deletes a sticker of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteGuildSticker {
    client: Client,
    guild_id: GuildId,
    sticker_id: StickerId,
    reason: Option<String>,
}

impl DeleteGuildSticker {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for DeleteGuildSticker {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/guilds/{guild_id}/stickers/{sticker_id}",
            format!("/guilds/{}/stickers/{}", self.guild_id, self.sticker_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

super::into_future!(
    GetSticker,
    GetStickerPacks,
    GetStickerPack,
    GetGuildStickers,
    GetGuildSticker,
    CreateGuildSticker,
    EditGuildSticker,
    DeleteGuildSticker
);

impl Client {
    /// Gets a sticker.
    pub fn sticker(&self, sticker_id: StickerId) -> GetSticker {
        GetSticker {
            client: self.clone(),
            sticker_id,
        }
    }

    /// Gets the packs of standard stickers.
    pub fn sticker_packs(&self) -> GetStickerPacks {
        GetStickerPacks {
            client: self.clone(),
        }
    }

    /// Gets a pack of standard stickers.
    pub fn sticker_pack(&self, pack_id: StickerPackId) -> GetStickerPack {
        GetStickerPack {
            client: self.clone(),
            pack_id,
        }
    }

    /// Gets the stickers of a guild.
    pub fn guild_stickers(&self, guild_id: GuildId) -> GetGuildStickers {
        GetGuildStickers {
            client: self.clone(),
            guild_id,
        }
    }

    /// Gets one sticker of a guild.
    pub fn guild_sticker(&self, guild_id: GuildId, sticker_id: StickerId) -> GetGuildSticker {
        GetGuildSticker {
            client: self.clone(),
            guild_id,
            sticker_id,
        }
    }

    /// Uploads a sticker to a guild.
    ///
    /// The name has 2 to 30 characters. The tags have up to 200 characters, for example
    /// the name of a Unicode emoji.
    pub fn create_guild_sticker(
        &self,
        guild_id: GuildId,
        name: impl Into<String>,
        tags: impl Into<String>,
        file: FileUpload,
    ) -> CreateGuildSticker {
        CreateGuildSticker {
            client: self.clone(),
            guild_id,
            name: name.into(),
            description: String::new(),
            tags: tags.into(),
            file,
            reason: None,
        }
    }

    /// Edits a sticker of a guild.
    pub fn edit_guild_sticker(&self, guild_id: GuildId, sticker_id: StickerId) -> EditGuildSticker {
        EditGuildSticker {
            client: self.clone(),
            guild_id,
            sticker_id,
            body: EditGuildStickerBody::default(),
            reason: None,
        }
    }

    /// Deletes a sticker of a guild.
    pub fn delete_guild_sticker(
        &self,
        guild_id: GuildId,
        sticker_id: StickerId,
    ) -> DeleteGuildSticker {
        DeleteGuildSticker {
            client: self.clone(),
            guild_id,
            sticker_id,
            reason: None,
        }
    }
}
