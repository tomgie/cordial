//! Application command endpoints: global commands, guild commands, and command permissions.
//!
//! The Discord endpoint "Batch Edit Application Command Permissions" is not here. Discord
//! disabled it with command permissions v2. Use [`Client::edit_command_permissions`] for
//! each command.

use cordial_model::Permissions;
use cordial_model::application::ApplicationIntegrationType;
use cordial_model::command::{
    Command, CommandDefinition, CommandOption, CommandPermission, CommandType, EntryPointHandler,
    GuildCommandPermissions,
};
use cordial_model::id::{ApplicationId, CommandId, GuildId};
use cordial_model::interaction::InteractionContextType;
use cordial_model::locale::Localizations;
use cordial_model::validate::{self, ValidationError};
use serde::Serialize;

use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, RequestBody, check};
use crate::route::{MajorParameter, Method, Query, Route};

/// The maximum number of permissions of one command in a guild.
const PERMISSION_COUNT_LIMIT: usize = 100;

/// The body of a create or edit request: a command definition without the ID.
#[derive(Serialize)]
struct CommandBody<'a> {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<CommandType>,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    name_localizations: Option<&'a Localizations>,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description_localizations: Option<&'a Localizations>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    options: &'a [CommandOption],
    #[serde(skip_serializing_if = "Option::is_none")]
    default_member_permissions: Option<Permissions>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    nsfw: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    integration_types: Option<&'a [ApplicationIntegrationType]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contexts: Option<&'a [InteractionContextType]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    handler: Option<EntryPointHandler>,
}

impl<'a> CommandBody<'a> {
    /// Makes the body. An edit request does not send the type, because Discord cannot change it.
    fn new(definition: &'a CommandDefinition, with_type: bool) -> Self {
        Self {
            kind: with_type.then_some(definition.kind),
            name: &definition.name,
            name_localizations: definition.name_localizations.as_ref(),
            description: &definition.description,
            description_localizations: definition.description_localizations.as_ref(),
            options: &definition.options,
            default_member_permissions: definition.default_member_permissions,
            nsfw: definition.nsfw,
            integration_types: definition.integration_types.as_deref(),
            contexts: definition.contexts.as_deref(),
            handler: definition.handler,
        }
    }
}

/// Checks a definition and encodes it for a create or edit request.
fn command_body(
    definition: &CommandDefinition,
    with_type: bool,
    validate: bool,
) -> Result<RequestBody, Error> {
    check(validate, || validate::command(definition))?;
    RequestBody::json(&CommandBody::new(definition, with_type))
}

/// Checks all definitions and encodes them for a bulk overwrite request.
fn commands_body(definitions: &[CommandDefinition], validate: bool) -> Result<RequestBody, Error> {
    check(validate, || {
        for (index, definition) in definitions.iter().enumerate() {
            validate::command(definition).map_err(|error| {
                ValidationError::new(format!("commands[{index}].{}", error.field), error.reason)
            })?;
        }
        Ok(())
    })?;
    RequestBody::json(definitions)
}

/// `GET /applications/{application.id}/commands`: the global commands of an application.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGlobalCommands {
    client: Client,
    application_id: ApplicationId,
    with_localizations: Option<bool>,
}

impl GetGlobalCommands {
    /// Asks for the full localization dictionaries instead of the names and descriptions in
    /// the locale of the request.
    pub fn with_localizations(mut self, with_localizations: bool) -> Self {
        self.with_localizations = Some(with_localizations);
        self
    }
}

impl Endpoint for GetGlobalCommands {
    type Response = Vec<Command>;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("with_localizations", self.with_localizations);
        Route::new(
            Method::Get,
            "/applications/{application_id}/commands",
            format!(
                "/applications/{}/commands{}",
                self.application_id,
                query.finish()
            ),
            MajorParameter::None,
        )
    }
}

/// `POST /applications/{application.id}/commands`: creates a global command.
///
/// A command with the same name replaces the old command.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateGlobalCommand {
    client: Client,
    application_id: ApplicationId,
    definition: CommandDefinition,
}

impl Endpoint for CreateGlobalCommand {
    type Response = Command;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/applications/{application_id}/commands",
            format!("/applications/{}/commands", self.application_id),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        command_body(&self.definition, true, validate)
    }
}

/// `GET /applications/{application.id}/commands/{command.id}`: a global command.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGlobalCommand {
    client: Client,
    application_id: ApplicationId,
    command_id: CommandId,
}

impl Endpoint for GetGlobalCommand {
    type Response = Command;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/applications/{application_id}/commands/{command_id}",
            format!(
                "/applications/{}/commands/{}",
                self.application_id, self.command_id
            ),
            MajorParameter::None,
        )
    }
}

/// `PATCH /applications/{application.id}/commands/{command.id}`: edits a global command.
///
/// Discord replaces each field that the definition sends. Fields that the definition does
/// not send (for example `None` values) keep their current value. The type does not change.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGlobalCommand {
    client: Client,
    application_id: ApplicationId,
    command_id: CommandId,
    definition: CommandDefinition,
}

impl Endpoint for EditGlobalCommand {
    type Response = Command;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/applications/{application_id}/commands/{command_id}",
            format!(
                "/applications/{}/commands/{}",
                self.application_id, self.command_id
            ),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        command_body(&self.definition, false, validate)
    }
}

/// `DELETE /applications/{application.id}/commands/{command.id}`: deletes a global command.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteGlobalCommand {
    client: Client,
    application_id: ApplicationId,
    command_id: CommandId,
}

impl Endpoint for DeleteGlobalCommand {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/applications/{application_id}/commands/{command_id}",
            format!(
                "/applications/{}/commands/{}",
                self.application_id, self.command_id
            ),
            MajorParameter::None,
        )
    }
}

/// `PUT /applications/{application.id}/commands`: replaces all global commands.
///
/// The list replaces commands of all types. Commands that do not exist yet count against the
/// daily limit of command creations.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct SetGlobalCommands {
    client: Client,
    application_id: ApplicationId,
    definitions: Vec<CommandDefinition>,
}

impl Endpoint for SetGlobalCommands {
    type Response = Vec<Command>;

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/applications/{application_id}/commands",
            format!("/applications/{}/commands", self.application_id),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        commands_body(&self.definitions, validate)
    }
}

/// `GET /applications/{application.id}/guilds/{guild.id}/commands`: the commands of an
/// application in a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildCommands {
    client: Client,
    application_id: ApplicationId,
    guild_id: GuildId,
    with_localizations: Option<bool>,
}

impl GetGuildCommands {
    /// Asks for the full localization dictionaries instead of the names and descriptions in
    /// the locale of the request.
    pub fn with_localizations(mut self, with_localizations: bool) -> Self {
        self.with_localizations = Some(with_localizations);
        self
    }
}

impl Endpoint for GetGuildCommands {
    type Response = Vec<Command>;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("with_localizations", self.with_localizations);
        Route::new(
            Method::Get,
            "/applications/{application_id}/guilds/{guild_id}/commands",
            format!(
                "/applications/{}/guilds/{}/commands{}",
                self.application_id,
                self.guild_id,
                query.finish()
            ),
            MajorParameter::None,
        )
    }
}

/// `POST /applications/{application.id}/guilds/{guild.id}/commands`: creates a guild
/// command.
///
/// A command with the same name replaces the old command.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateGuildCommand {
    client: Client,
    application_id: ApplicationId,
    guild_id: GuildId,
    definition: CommandDefinition,
}

impl Endpoint for CreateGuildCommand {
    type Response = Command;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/applications/{application_id}/guilds/{guild_id}/commands",
            format!(
                "/applications/{}/guilds/{}/commands",
                self.application_id, self.guild_id
            ),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        command_body(&self.definition, true, validate)
    }
}

/// `GET /applications/{application.id}/guilds/{guild.id}/commands/{command.id}`: a guild
/// command.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildCommand {
    client: Client,
    application_id: ApplicationId,
    guild_id: GuildId,
    command_id: CommandId,
}

impl Endpoint for GetGuildCommand {
    type Response = Command;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/applications/{application_id}/guilds/{guild_id}/commands/{command_id}",
            format!(
                "/applications/{}/guilds/{}/commands/{}",
                self.application_id, self.guild_id, self.command_id
            ),
            MajorParameter::None,
        )
    }
}

/// `PATCH /applications/{application.id}/guilds/{guild.id}/commands/{command.id}`: edits a
/// guild command.
///
/// Discord replaces each field that the definition sends. Fields that the definition does
/// not send (for example `None` values) keep their current value. The type does not change.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuildCommand {
    client: Client,
    application_id: ApplicationId,
    guild_id: GuildId,
    command_id: CommandId,
    definition: CommandDefinition,
}

impl Endpoint for EditGuildCommand {
    type Response = Command;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/applications/{application_id}/guilds/{guild_id}/commands/{command_id}",
            format!(
                "/applications/{}/guilds/{}/commands/{}",
                self.application_id, self.guild_id, self.command_id
            ),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        command_body(&self.definition, false, validate)
    }
}

/// `DELETE /applications/{application.id}/guilds/{guild.id}/commands/{command.id}`: deletes
/// a guild command.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteGuildCommand {
    client: Client,
    application_id: ApplicationId,
    guild_id: GuildId,
    command_id: CommandId,
}

impl Endpoint for DeleteGuildCommand {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/applications/{application_id}/guilds/{guild_id}/commands/{command_id}",
            format!(
                "/applications/{}/guilds/{}/commands/{}",
                self.application_id, self.guild_id, self.command_id
            ),
            MajorParameter::None,
        )
    }
}

/// `PUT /applications/{application.id}/guilds/{guild.id}/commands`: replaces all commands
/// of an application in a guild.
///
/// The list replaces commands of all types.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct SetGuildCommands {
    client: Client,
    application_id: ApplicationId,
    guild_id: GuildId,
    definitions: Vec<CommandDefinition>,
}

impl Endpoint for SetGuildCommands {
    type Response = Vec<Command>;

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/applications/{application_id}/guilds/{guild_id}/commands",
            format!(
                "/applications/{}/guilds/{}/commands",
                self.application_id, self.guild_id
            ),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        commands_body(&self.definitions, validate)
    }
}

/// `GET /applications/{application.id}/guilds/{guild.id}/commands/permissions`: the
/// permissions of all commands of an application in a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildCommandPermissions {
    client: Client,
    application_id: ApplicationId,
    guild_id: GuildId,
}

impl Endpoint for GetGuildCommandPermissions {
    type Response = Vec<GuildCommandPermissions>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/applications/{application_id}/guilds/{guild_id}/commands/permissions",
            format!(
                "/applications/{}/guilds/{}/commands/permissions",
                self.application_id, self.guild_id
            ),
            MajorParameter::None,
        )
    }
}

/// `GET /applications/{application.id}/guilds/{guild.id}/commands/{command.id}/permissions`:
/// the permissions of one command in a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetCommandPermissions {
    client: Client,
    application_id: ApplicationId,
    guild_id: GuildId,
    command_id: CommandId,
}

impl Endpoint for GetCommandPermissions {
    type Response = GuildCommandPermissions;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/applications/{application_id}/guilds/{guild_id}/commands/{command_id}/permissions",
            format!(
                "/applications/{}/guilds/{}/commands/{}/permissions",
                self.application_id, self.guild_id, self.command_id
            ),
            MajorParameter::None,
        )
    }
}

/// `PUT /applications/{application.id}/guilds/{guild.id}/commands/{command.id}/permissions`:
/// replaces the permissions of one command in a guild.
///
/// This endpoint needs an OAuth2 Bearer token with permission to manage the guild and its
/// roles. A bot token gives an error. Send the request with a client that has a
/// [`Token::bearer`](crate::Token::bearer) token.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditCommandPermissions {
    client: Client,
    application_id: ApplicationId,
    guild_id: GuildId,
    command_id: CommandId,
    permissions: Vec<CommandPermission>,
}

#[derive(Serialize)]
struct EditCommandPermissionsBody<'a> {
    permissions: &'a [CommandPermission],
}

impl Endpoint for EditCommandPermissions {
    type Response = GuildCommandPermissions;

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/applications/{application_id}/guilds/{guild_id}/commands/{command_id}/permissions",
            format!(
                "/applications/{}/guilds/{}/commands/{}/permissions",
                self.application_id, self.guild_id, self.command_id
            ),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            let count = self.permissions.len();
            if count > PERMISSION_COUNT_LIMIT {
                return Err(ValidationError::new(
                    "permissions",
                    format!("has {count} items; the maximum is {PERMISSION_COUNT_LIMIT}"),
                ));
            }
            Ok(())
        })?;
        RequestBody::json(&EditCommandPermissionsBody {
            permissions: &self.permissions,
        })
    }
}

super::into_future!(
    GetGlobalCommands,
    CreateGlobalCommand,
    GetGlobalCommand,
    EditGlobalCommand,
    DeleteGlobalCommand,
    SetGlobalCommands,
    GetGuildCommands,
    CreateGuildCommand,
    GetGuildCommand,
    EditGuildCommand,
    DeleteGuildCommand,
    SetGuildCommands,
    GetGuildCommandPermissions,
    GetCommandPermissions,
    EditCommandPermissions
);

impl Client {
    /// Gets the global commands of an application.
    pub fn global_commands(&self, application_id: ApplicationId) -> GetGlobalCommands {
        GetGlobalCommands {
            client: self.clone(),
            application_id,
            with_localizations: None,
        }
    }

    /// Creates a global command. A command with the same name replaces the old command.
    pub fn create_global_command(
        &self,
        application_id: ApplicationId,
        definition: CommandDefinition,
    ) -> CreateGlobalCommand {
        CreateGlobalCommand {
            client: self.clone(),
            application_id,
            definition,
        }
    }

    /// Gets a global command.
    pub fn global_command(
        &self,
        application_id: ApplicationId,
        command_id: CommandId,
    ) -> GetGlobalCommand {
        GetGlobalCommand {
            client: self.clone(),
            application_id,
            command_id,
        }
    }

    /// Edits a global command. Discord replaces each field that the definition sends.
    pub fn edit_global_command(
        &self,
        application_id: ApplicationId,
        command_id: CommandId,
        definition: CommandDefinition,
    ) -> EditGlobalCommand {
        EditGlobalCommand {
            client: self.clone(),
            application_id,
            command_id,
            definition,
        }
    }

    /// Deletes a global command.
    pub fn delete_global_command(
        &self,
        application_id: ApplicationId,
        command_id: CommandId,
    ) -> DeleteGlobalCommand {
        DeleteGlobalCommand {
            client: self.clone(),
            application_id,
            command_id,
        }
    }

    /// Replaces all global commands with a new list (a bulk overwrite).
    pub fn set_global_commands(
        &self,
        application_id: ApplicationId,
        definitions: Vec<CommandDefinition>,
    ) -> SetGlobalCommands {
        SetGlobalCommands {
            client: self.clone(),
            application_id,
            definitions,
        }
    }

    /// Gets the commands of an application in a guild.
    pub fn guild_commands(
        &self,
        application_id: ApplicationId,
        guild_id: GuildId,
    ) -> GetGuildCommands {
        GetGuildCommands {
            client: self.clone(),
            application_id,
            guild_id,
            with_localizations: None,
        }
    }

    /// Creates a guild command. A command with the same name replaces the old command.
    pub fn create_guild_command(
        &self,
        application_id: ApplicationId,
        guild_id: GuildId,
        definition: CommandDefinition,
    ) -> CreateGuildCommand {
        CreateGuildCommand {
            client: self.clone(),
            application_id,
            guild_id,
            definition,
        }
    }

    /// Gets a guild command.
    pub fn guild_command(
        &self,
        application_id: ApplicationId,
        guild_id: GuildId,
        command_id: CommandId,
    ) -> GetGuildCommand {
        GetGuildCommand {
            client: self.clone(),
            application_id,
            guild_id,
            command_id,
        }
    }

    /// Edits a guild command. Discord replaces each field that the definition sends.
    pub fn edit_guild_command(
        &self,
        application_id: ApplicationId,
        guild_id: GuildId,
        command_id: CommandId,
        definition: CommandDefinition,
    ) -> EditGuildCommand {
        EditGuildCommand {
            client: self.clone(),
            application_id,
            guild_id,
            command_id,
            definition,
        }
    }

    /// Deletes a guild command.
    pub fn delete_guild_command(
        &self,
        application_id: ApplicationId,
        guild_id: GuildId,
        command_id: CommandId,
    ) -> DeleteGuildCommand {
        DeleteGuildCommand {
            client: self.clone(),
            application_id,
            guild_id,
            command_id,
        }
    }

    /// Replaces all commands of an application in a guild with a new list (a bulk overwrite).
    pub fn set_guild_commands(
        &self,
        application_id: ApplicationId,
        guild_id: GuildId,
        definitions: Vec<CommandDefinition>,
    ) -> SetGuildCommands {
        SetGuildCommands {
            client: self.clone(),
            application_id,
            guild_id,
            definitions,
        }
    }

    /// Gets the permissions of all commands of an application in a guild.
    pub fn guild_command_permissions(
        &self,
        application_id: ApplicationId,
        guild_id: GuildId,
    ) -> GetGuildCommandPermissions {
        GetGuildCommandPermissions {
            client: self.clone(),
            application_id,
            guild_id,
        }
    }

    /// Gets the permissions of one command in a guild.
    pub fn command_permissions(
        &self,
        application_id: ApplicationId,
        guild_id: GuildId,
        command_id: CommandId,
    ) -> GetCommandPermissions {
        GetCommandPermissions {
            client: self.clone(),
            application_id,
            guild_id,
            command_id,
        }
    }

    /// Replaces the permissions of one command in a guild (up to 100).
    ///
    /// This endpoint needs a client with an OAuth2 Bearer token. A bot token gives an error.
    pub fn edit_command_permissions(
        &self,
        application_id: ApplicationId,
        guild_id: GuildId,
        command_id: CommandId,
        permissions: Vec<CommandPermission>,
    ) -> EditCommandPermissions {
        EditCommandPermissions {
            client: self.clone(),
            application_id,
            guild_id,
            command_id,
            permissions,
        }
    }
}
