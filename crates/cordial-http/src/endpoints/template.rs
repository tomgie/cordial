//! Guild template endpoints.

use cordial_model::id::{GuildId, UserId};
use cordial_model::user::User;
use cordial_model::{Patch, Timestamp};
use serde::{Deserialize, Serialize};

use super::limits;
use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, RequestBody, check};
use crate::route::{MajorParameter, Method, Route, encode_segment};

/// The maximum number of characters in a template name.
const NAME_LIMIT: usize = 100;
/// The maximum number of characters in a template description.
const DESCRIPTION_LIMIT: usize = 120;

/// A guild template: a code that makes a new guild from a snapshot of a guild.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildTemplate {
    /// The template code. It is the unique ID.
    pub code: String,
    /// The name.
    pub name: String,
    /// The description.
    #[serde(default)]
    pub description: Option<String>,
    /// The number of guilds that were made from the template.
    #[serde(default)]
    pub usage_count: u64,
    /// The user that made the template.
    pub creator_id: UserId,
    /// The user that made the template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator: Option<User>,
    /// When the template was made.
    pub created_at: Timestamp,
    /// When the template was last synced with the source guild.
    pub updated_at: Timestamp,
    /// The guild that the template is a snapshot of.
    pub source_guild_id: GuildId,
    /// The snapshot: a partial guild. IDs in it are placeholder integers, not snowflakes, so it
    /// stays as raw JSON.
    pub serialized_source_guild: serde_json::Value,
    /// Tells if the source guild has changes that are not synced.
    #[serde(default)]
    pub is_dirty: Option<bool>,
}

/// `GET /guilds/templates/{template.code}`: a guild template.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildTemplate {
    client: Client,
    code: String,
}

impl Endpoint for GetGuildTemplate {
    type Response = GuildTemplate;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/templates/{code}",
            format!("/guilds/templates/{}", encode_segment(&self.code)),
            MajorParameter::None,
        )
    }
}

/// `GET /guilds/{guild.id}/templates`: the templates of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildTemplates {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildTemplates {
    type Response = Vec<GuildTemplate>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/templates",
            format!("/guilds/{}/templates", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

#[derive(Clone, Debug, Default, Serialize)]
struct TemplateBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    description: Patch<String>,
}

impl TemplateBody {
    fn check(&self) -> Result<(), cordial_model::validate::ValidationError> {
        limits::optional_length("name", self.name.as_deref(), 1, NAME_LIMIT)?;
        limits::optional_length(
            "description",
            limits::patch_text(&self.description),
            0,
            DESCRIPTION_LIMIT,
        )
    }
}

/// `POST /guilds/{guild.id}/templates`: makes a template from a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateGuildTemplate {
    client: Client,
    guild_id: GuildId,
    body: TemplateBody,
}

impl CreateGuildTemplate {
    /// Sets the description, 0 to 120 characters.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.body.description = Patch::Set(description.into());
        self
    }
}

impl Endpoint for CreateGuildTemplate {
    type Response = GuildTemplate;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/guilds/{guild_id}/templates",
            format!("/guilds/{}/templates", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || self.body.check())?;
        RequestBody::json(&self.body)
    }
}

/// `PUT /guilds/{guild.id}/templates/{template.code}`: syncs a template with the current state
/// of the guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct SyncGuildTemplate {
    client: Client,
    guild_id: GuildId,
    code: String,
}

impl Endpoint for SyncGuildTemplate {
    type Response = GuildTemplate;

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/guilds/{guild_id}/templates/{code}",
            format!(
                "/guilds/{}/templates/{}",
                self.guild_id,
                encode_segment(&self.code)
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `PATCH /guilds/{guild.id}/templates/{template.code}`: changes the name or the description
/// of a template.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuildTemplate {
    client: Client,
    guild_id: GuildId,
    code: String,
    body: TemplateBody,
}

impl EditGuildTemplate {
    /// Sets the name, 1 to 100 characters.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.body.name = Some(name.into());
        self
    }

    /// Sets the description, 0 to 120 characters.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.body.description = Patch::Set(description.into());
        self
    }

    /// Removes the description.
    pub fn clear_description(mut self) -> Self {
        self.body.description = Patch::Clear;
        self
    }
}

impl Endpoint for EditGuildTemplate {
    type Response = GuildTemplate;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/templates/{code}",
            format!(
                "/guilds/{}/templates/{}",
                self.guild_id,
                encode_segment(&self.code)
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || self.body.check())?;
        RequestBody::json(&self.body)
    }
}

/// `DELETE /guilds/{guild.id}/templates/{template.code}`: deletes a template.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteGuildTemplate {
    client: Client,
    guild_id: GuildId,
    code: String,
}

impl Endpoint for DeleteGuildTemplate {
    /// The deleted template.
    type Response = GuildTemplate;

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/guilds/{guild_id}/templates/{code}",
            format!(
                "/guilds/{}/templates/{}",
                self.guild_id,
                encode_segment(&self.code)
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

super::into_future!(
    GetGuildTemplate,
    GetGuildTemplates,
    CreateGuildTemplate,
    SyncGuildTemplate,
    EditGuildTemplate,
    DeleteGuildTemplate,
);

impl Client {
    /// Gets a guild template from its code.
    pub fn guild_template(&self, code: impl Into<String>) -> GetGuildTemplate {
        GetGuildTemplate {
            client: self.clone(),
            code: code.into(),
        }
    }

    /// Gets the templates of a guild. Needs `MANAGE_GUILD`.
    pub fn guild_templates(&self, guild_id: GuildId) -> GetGuildTemplates {
        GetGuildTemplates {
            client: self.clone(),
            guild_id,
        }
    }

    /// Makes a template from a guild. The name has 1 to 100 characters. Needs `MANAGE_GUILD`.
    pub fn create_guild_template(
        &self,
        guild_id: GuildId,
        name: impl Into<String>,
    ) -> CreateGuildTemplate {
        CreateGuildTemplate {
            client: self.clone(),
            guild_id,
            body: TemplateBody {
                name: Some(name.into()),
                description: Patch::Keep,
            },
        }
    }

    /// Syncs a template with the current state of the guild. Needs `MANAGE_GUILD`.
    pub fn sync_guild_template(
        &self,
        guild_id: GuildId,
        code: impl Into<String>,
    ) -> SyncGuildTemplate {
        SyncGuildTemplate {
            client: self.clone(),
            guild_id,
            code: code.into(),
        }
    }

    /// Changes the name or the description of a template. Needs `MANAGE_GUILD`.
    pub fn edit_guild_template(
        &self,
        guild_id: GuildId,
        code: impl Into<String>,
    ) -> EditGuildTemplate {
        EditGuildTemplate {
            client: self.clone(),
            guild_id,
            code: code.into(),
            body: TemplateBody::default(),
        }
    }

    /// Deletes a template. Needs `MANAGE_GUILD`.
    pub fn delete_guild_template(
        &self,
        guild_id: GuildId,
        code: impl Into<String>,
    ) -> DeleteGuildTemplate {
        DeleteGuildTemplate {
            client: self.clone(),
            guild_id,
            code: code.into(),
        }
    }
}
