//! Application endpoints: the application, activity instances, attachments, and role
//! connection metadata.

use std::collections::HashMap;

use cordial_model::Patch;
use cordial_model::application::{
    ActivityInstance, Application, ApplicationFlags, ApplicationIntegrationType,
    EventWebhookStatus, InstallParams, IntegrationTypeConfig, RoleConnectionMetadata,
};
use cordial_model::id::ApplicationId;
use cordial_model::message::Attachment;
use cordial_model::validate::ValidationError;
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, FileUpload, ImageData, Multipart, RequestBody, check};
use crate::route::{MajorParameter, Method, Route, encode_segment};

/// The maximum number of tags of an application.
const TAG_COUNT_LIMIT: usize = 5;
/// The maximum number of characters in a tag of an application.
const TAG_LENGTH_LIMIT: usize = 20;
/// The maximum number of role connection metadata records of an application.
const ROLE_CONNECTION_METADATA_LIMIT: usize = 5;

/// `GET /applications/@me` or `GET /applications/{application.id}`: an application.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetApplication {
    client: Client,
    application_id: Option<ApplicationId>,
}

impl Endpoint for GetApplication {
    type Response = Application;

    fn route(&self) -> Route {
        match self.application_id {
            None => Route::new(
                Method::Get,
                "/applications/@me",
                "/applications/@me".to_owned(),
                MajorParameter::None,
            ),
            Some(application_id) => Route::new(
                Method::Get,
                "/applications/{application_id}",
                format!("/applications/{application_id}"),
                MajorParameter::None,
            ),
        }
    }
}

/// `PATCH /applications/@me` or `PATCH /applications/{application.id}`: edits an application.
///
/// Discord changes only the fields that you set.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditApplication {
    client: Client,
    application_id: Option<ApplicationId>,
    body: EditApplicationBody,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditApplicationBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    custom_install_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role_connections_verification_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    install_params: Option<InstallParams>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integration_types_config: Option<HashMap<ApplicationIntegrationType, IntegrationTypeConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<ApplicationFlags>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    icon: Patch<ImageData>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    cover_image: Patch<ImageData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    interactions_endpoint_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_webhooks_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_webhooks_status: Option<EventWebhookStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_webhooks_types: Option<Vec<String>>,
}

impl EditApplication {
    /// Sets the custom install URL.
    pub fn custom_install_url(mut self, url: impl Into<String>) -> Self {
        self.body.custom_install_url = Some(url.into());
        self
    }

    /// Sets the description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.body.description = Some(description.into());
        self
    }

    /// Sets the URL for role connection verification.
    pub fn role_connections_verification_url(mut self, url: impl Into<String>) -> Self {
        self.body.role_connections_verification_url = Some(url.into());
        self
    }

    /// Sets the settings of the default install link.
    pub fn install_params(mut self, params: InstallParams) -> Self {
        self.body.install_params = Some(params);
        self
    }

    /// Sets the default install settings for each installation context.
    pub fn integration_types_config(
        mut self,
        config: HashMap<ApplicationIntegrationType, IntegrationTypeConfig>,
    ) -> Self {
        self.body.integration_types_config = Some(config);
        self
    }

    /// Sets the flags. Discord accepts only the limited intent flags:
    /// `GATEWAY_PRESENCE_LIMITED`, `GATEWAY_GUILD_MEMBERS_LIMITED`, and
    /// `GATEWAY_MESSAGE_CONTENT_LIMITED`.
    pub fn flags(mut self, flags: ApplicationFlags) -> Self {
        self.body.flags = Some(flags);
        self
    }

    /// Sets the icon.
    pub fn icon(mut self, icon: ImageData) -> Self {
        self.body.icon = Patch::Set(icon);
        self
    }

    /// Removes the icon.
    pub fn clear_icon(mut self) -> Self {
        self.body.icon = Patch::Clear;
        self
    }

    /// Sets the cover image for rich presence invites.
    pub fn cover_image(mut self, image: ImageData) -> Self {
        self.body.cover_image = Patch::Set(image);
        self
    }

    /// Removes the cover image.
    pub fn clear_cover_image(mut self) -> Self {
        self.body.cover_image = Patch::Clear;
        self
    }

    /// Sets the URL that receives interactions over HTTP. Discord checks the URL first.
    pub fn interactions_endpoint_url(mut self, url: impl Into<String>) -> Self {
        self.body.interactions_endpoint_url = Some(url.into());
        self
    }

    /// Sets the tags: up to 5, each up to 20 characters.
    pub fn tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.body.tags = Some(tags.into_iter().map(Into::into).collect());
        self
    }

    /// Sets the URL that receives webhook events.
    pub fn event_webhooks_url(mut self, url: impl Into<String>) -> Self {
        self.body.event_webhooks_url = Some(url.into());
        self
    }

    /// Turns webhook events on (`Enabled`) or off (`Disabled`).
    pub fn event_webhooks_status(mut self, status: EventWebhookStatus) -> Self {
        self.body.event_webhooks_status = Some(status);
        self
    }

    /// Sets the webhook event types to subscribe to, for example `ENTITLEMENT_CREATE`.
    pub fn event_webhooks_types(
        mut self,
        types: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.body.event_webhooks_types = Some(types.into_iter().map(Into::into).collect());
        self
    }
}

impl Endpoint for EditApplication {
    type Response = Application;

    fn route(&self) -> Route {
        match self.application_id {
            None => Route::new(
                Method::Patch,
                "/applications/@me",
                "/applications/@me".to_owned(),
                MajorParameter::None,
            ),
            Some(application_id) => Route::new(
                Method::Patch,
                "/applications/{application_id}",
                format!("/applications/{application_id}"),
                MajorParameter::None,
            ),
        }
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || check_tags(self.body.tags.as_deref()))?;
        RequestBody::json(&self.body)
    }
}

fn check_tags(tags: Option<&[String]>) -> Result<(), ValidationError> {
    let Some(tags) = tags else {
        return Ok(());
    };

    if tags.len() > TAG_COUNT_LIMIT {
        return Err(ValidationError::new(
            "tags",
            format!("has {} items; the maximum is {TAG_COUNT_LIMIT}", tags.len()),
        ));
    }
    for (index, tag) in tags.iter().enumerate() {
        if tag.chars().count() > TAG_LENGTH_LIMIT {
            return Err(ValidationError::new(
                format!("tags[{index}]"),
                format!("has more than {TAG_LENGTH_LIMIT} characters"),
            ));
        }
    }
    Ok(())
}

/// `GET /applications/{application.id}/activity-instances/{instance_id}`: an activity
/// instance.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetActivityInstance {
    client: Client,
    application_id: ApplicationId,
    instance_id: String,
}

impl Endpoint for GetActivityInstance {
    type Response = ActivityInstance;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/applications/{application_id}/activity-instances/{instance_id}",
            format!(
                "/applications/{}/activity-instances/{}",
                self.application_id,
                encode_segment(&self.instance_id)
            ),
            MajorParameter::None,
        )
    }
}

/// The response of `POST /applications/{application.id}/attachment`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ApplicationAttachment {
    /// The uploaded attachment.
    pub attachment: Attachment,
}

/// `POST /applications/{application.id}/attachment`: uploads a file for an activity.
///
/// The Discord docs do not describe this endpoint. Only the OpenAPI specification has it.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct UploadApplicationAttachment {
    client: Client,
    application_id: ApplicationId,
    file: FileUpload,
}

impl Endpoint for UploadApplicationAttachment {
    type Response = ApplicationAttachment;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/applications/{application_id}/attachment",
            format!("/applications/{}/attachment", self.application_id),
            MajorParameter::None,
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        let multipart = Multipart::new().named_file("file", self.file.clone());
        Ok(RequestBody::Multipart(multipart))
    }
}

/// `GET /applications/{application.id}/role-connections/metadata`: the role connection
/// metadata records of an application.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetRoleConnectionMetadata {
    client: Client,
    application_id: ApplicationId,
}

impl Endpoint for GetRoleConnectionMetadata {
    type Response = Vec<RoleConnectionMetadata>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/applications/{application_id}/role-connections/metadata",
            format!(
                "/applications/{}/role-connections/metadata",
                self.application_id
            ),
            MajorParameter::None,
        )
    }
}

/// `PUT /applications/{application.id}/role-connections/metadata`: replaces the role
/// connection metadata records of an application.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct SetRoleConnectionMetadata {
    client: Client,
    application_id: ApplicationId,
    records: Vec<RoleConnectionMetadata>,
}

impl Endpoint for SetRoleConnectionMetadata {
    type Response = Vec<RoleConnectionMetadata>;

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/applications/{application_id}/role-connections/metadata",
            format!(
                "/applications/{}/role-connections/metadata",
                self.application_id
            ),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || check_role_connection_metadata(&self.records))?;
        RequestBody::json(&self.records)
    }
}

fn check_role_connection_metadata(
    records: &[RoleConnectionMetadata],
) -> Result<(), ValidationError> {
    if records.len() > ROLE_CONNECTION_METADATA_LIMIT {
        return Err(ValidationError::new(
            "records",
            format!(
                "has {} items; the maximum is {ROLE_CONNECTION_METADATA_LIMIT}",
                records.len()
            ),
        ));
    }

    for (index, record) in records.iter().enumerate() {
        let key_length = record.key.chars().count();
        let key_characters_ok = record.key.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        });
        if !(1..=50).contains(&key_length) || !key_characters_ok {
            return Err(ValidationError::new(
                format!("records[{index}].key"),
                "must be 1 to 50 characters of a-z, 0-9, or '_'",
            ));
        }
        check_length(&format!("records[{index}].name"), &record.name, 100)?;
        check_length(
            &format!("records[{index}].description"),
            &record.description,
            200,
        )?;
    }
    Ok(())
}

fn check_length(field: &str, text: &str, max: usize) -> Result<(), ValidationError> {
    let count = text.chars().count();
    if !(1..=max).contains(&count) {
        return Err(ValidationError::new(
            field,
            format!("has {count} characters; it must have 1 to {max}"),
        ));
    }
    Ok(())
}

super::into_future!(
    GetApplication,
    EditApplication,
    GetActivityInstance,
    UploadApplicationAttachment,
    GetRoleConnectionMetadata,
    SetRoleConnectionMetadata
);

impl Client {
    /// Gets the application of the current bot user.
    pub fn current_application(&self) -> GetApplication {
        GetApplication {
            client: self.clone(),
            application_id: None,
        }
    }

    /// Gets an application by its ID. Only the OpenAPI specification has this route.
    pub fn application(&self, application_id: ApplicationId) -> GetApplication {
        GetApplication {
            client: self.clone(),
            application_id: Some(application_id),
        }
    }

    /// Edits the application of the current bot user.
    pub fn edit_current_application(&self) -> EditApplication {
        EditApplication {
            client: self.clone(),
            application_id: None,
            body: EditApplicationBody::default(),
        }
    }

    /// Edits an application by its ID. Only the OpenAPI specification has this route.
    pub fn edit_application(&self, application_id: ApplicationId) -> EditApplication {
        EditApplication {
            client: self.clone(),
            application_id: Some(application_id),
            body: EditApplicationBody::default(),
        }
    }

    /// Gets an activity instance, for example to check that a user is in it.
    pub fn activity_instance(
        &self,
        application_id: ApplicationId,
        instance_id: impl Into<String>,
    ) -> GetActivityInstance {
        GetActivityInstance {
            client: self.clone(),
            application_id,
            instance_id: instance_id.into(),
        }
    }

    /// Uploads a file for an activity. The file goes in the part `file`.
    pub fn upload_application_attachment(
        &self,
        application_id: ApplicationId,
        file: FileUpload,
    ) -> UploadApplicationAttachment {
        UploadApplicationAttachment {
            client: self.clone(),
            application_id,
            file,
        }
    }

    /// Gets the role connection metadata records of an application.
    pub fn role_connection_metadata(
        &self,
        application_id: ApplicationId,
    ) -> GetRoleConnectionMetadata {
        GetRoleConnectionMetadata {
            client: self.clone(),
            application_id,
        }
    }

    /// Replaces the role connection metadata records of an application (up to 5).
    pub fn set_role_connection_metadata(
        &self,
        application_id: ApplicationId,
        records: Vec<RoleConnectionMetadata>,
    ) -> SetRoleConnectionMetadata {
        SetRoleConnectionMetadata {
            client: self.clone(),
            application_id,
            records,
        }
    }
}
