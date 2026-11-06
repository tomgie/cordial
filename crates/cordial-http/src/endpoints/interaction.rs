//! Interaction response and follow-up message endpoints.
//!
//! These routes have the interaction token in the path. They send no `Authorization` header,
//! and the global rate limit does not apply to them. An interaction token is valid for 15
//! minutes. Send the first response within 3 seconds.

use cordial_model::id::{ApplicationId, InteractionId, MessageId};
use cordial_model::interaction::{
    InteractionCallbackResponse, InteractionResponse, InteractionResponseData,
};
use cordial_model::message::{AttachmentRequest, AttachmentRequestId, Message, MessageFlags};
use cordial_model::validate::{self, ValidationError};

use super::webhook::{PathToken, check_permitted_flags, edit_message_body, is_empty_message};
use crate::client::Client;
use crate::error::Error;
use crate::payload::{MessagePayload, message_setters};
use crate::request::{Endpoint, FileUpload, RequestBody, check};
use crate::route::{MajorParameter, Method, Query, Route};

/// The largest number of choices in an autocomplete response.
const AUTOCOMPLETE_CHOICE_LIMIT: usize = 25;

/// The flags that a follow-up message can set.
const FOLLOWUP_FLAGS: MessageFlags = MessageFlags::SUPPRESS_EMBEDS
    .union(MessageFlags::SUPPRESS_NOTIFICATIONS)
    .union(MessageFlags::IS_COMPONENTS_V2)
    .union(MessageFlags::EPHEMERAL);

/// Checks the data of an interaction response.
fn check_response(response: &InteractionResponse) -> Result<(), ValidationError> {
    match &response.data {
        Some(InteractionResponseData::Message(message)) => {
            if let Some(content) = &message.content {
                validate::message_content(content)?;
            }
            if let Some(embeds) = &message.embeds {
                validate::embeds(embeds)?;
            }
            let has_content = message
                .content
                .as_deref()
                .is_some_and(|content| !content.is_empty());
            let has_embeds = message
                .embeds
                .as_ref()
                .is_some_and(|embeds| !embeds.is_empty());
            validate::message_components(
                message.flags.unwrap_or_default(),
                message.components.as_deref().unwrap_or_default(),
                has_content,
                has_embeds,
                message.poll.is_some(),
            )
        }
        Some(InteractionResponseData::Modal(modal)) => {
            validate::modal(&modal.custom_id, &modal.title, &modal.components)
        }
        Some(InteractionResponseData::Autocomplete(autocomplete)) => {
            let count = autocomplete.choices.len();
            if count > AUTOCOMPLETE_CHOICE_LIMIT {
                return Err(ValidationError::new(
                    "choices",
                    format!("has {count} choices; the maximum is 25"),
                ));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Gives the body of an interaction callback: JSON, or multipart when there are files.
fn callback_body(
    response: &InteractionResponse,
    files: &[FileUpload],
    validate: bool,
) -> Result<RequestBody, Error> {
    check(validate, || check_response(response))?;
    if files.is_empty() {
        return RequestBody::json(response);
    }

    let mut response = response.clone();
    if let Some(InteractionResponseData::Message(message)) = &mut response.data {
        let attachments = message.attachments.get_or_insert_with(Vec::new);
        for (index, file) in files.iter().enumerate() {
            let index = u32::try_from(index).unwrap_or(u32::MAX);
            let present = attachments
                .iter()
                .any(|attachment| attachment.id == AttachmentRequestId::New(index));
            if !present {
                attachments.push(AttachmentRequest::new_file(index, &file.filename));
            }
        }
    }
    RequestBody::json_with_files(&response, files)
}

fn callback_route(interaction_id: InteractionId, token: &PathToken, with_response: bool) -> Route {
    let mut query = Query::new();
    if with_response {
        query.push("with_response", true);
    }
    Route::new(
        Method::Post,
        "/interactions/{interaction_id}/{interaction_token}/callback",
        format!(
            "/interactions/{}/{}/callback{}",
            interaction_id,
            token.segment(),
            query.finish()
        ),
        MajorParameter::None,
    )
    .with_path_token()
}

/// `POST /interactions/{interaction.id}/{interaction.token}/callback`: responds to an
/// interaction.
///
/// Discord answers with no body. Call [`with_response`](Self::with_response) to get the
/// interaction callback response.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateInteractionResponse {
    client: Client,
    interaction_id: InteractionId,
    token: PathToken,
    response: InteractionResponse,
    files: Vec<FileUpload>,
}

impl CreateInteractionResponse {
    /// Adds a file to upload. Refer to it in an embed as `attachment://<filename>`.
    pub fn file(mut self, file: FileUpload) -> Self {
        self.files.push(file);
        self
    }

    /// Makes Discord give the interaction callback response (`with_response=true`), for
    /// example the ID of the new message.
    pub fn with_response(self) -> CreateInteractionResponseWithCallback {
        CreateInteractionResponseWithCallback {
            client: self.client,
            interaction_id: self.interaction_id,
            token: self.token,
            response: self.response,
            files: self.files,
        }
    }
}

impl Endpoint for CreateInteractionResponse {
    type Response = ();

    fn route(&self) -> Route {
        callback_route(self.interaction_id, &self.token, false)
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        callback_body(&self.response, &self.files, validate)
    }
}

/// `POST /interactions/{interaction.id}/{interaction.token}/callback?with_response=true`:
/// responds to an interaction, and gives the interaction callback response.
///
/// Make it with [`CreateInteractionResponse::with_response`].
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateInteractionResponseWithCallback {
    client: Client,
    interaction_id: InteractionId,
    token: PathToken,
    response: InteractionResponse,
    files: Vec<FileUpload>,
}

impl CreateInteractionResponseWithCallback {
    /// Adds a file to upload. Refer to it in an embed as `attachment://<filename>`.
    pub fn file(mut self, file: FileUpload) -> Self {
        self.files.push(file);
        self
    }
}

impl Endpoint for CreateInteractionResponseWithCallback {
    type Response = InteractionCallbackResponse;

    fn route(&self) -> Route {
        callback_route(self.interaction_id, &self.token, true)
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        callback_body(&self.response, &self.files, validate)
    }
}

/// `GET /webhooks/{application.id}/{interaction.token}/messages/@original`: the first
/// response to an interaction.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetOriginalInteractionResponse {
    client: Client,
    application_id: ApplicationId,
    token: PathToken,
}

impl Endpoint for GetOriginalInteractionResponse {
    type Response = Message;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/webhooks/{application_id}/{interaction_token}/messages/@original",
            format!(
                "/webhooks/{}/{}/messages/@original",
                self.application_id,
                self.token.segment()
            ),
            MajorParameter::Webhook(self.application_id.cast(), self.token.hash()),
        )
        .with_path_token()
    }
}

/// `PATCH /webhooks/{application.id}/{interaction.token}/messages/@original`: edits the first
/// response to an interaction, for example after a deferred response.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditOriginalInteractionResponse {
    client: Client,
    application_id: ApplicationId,
    token: PathToken,
    payload: MessagePayload,
    files: Vec<FileUpload>,
}

impl EditOriginalInteractionResponse {
    message_setters!();

    /// Removes the content.
    pub fn clear_content(mut self) -> Self {
        self.payload.content = Some(String::new());
        self
    }
}

impl Endpoint for EditOriginalInteractionResponse {
    type Response = Message;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/webhooks/{application_id}/{interaction_token}/messages/@original",
            format!(
                "/webhooks/{}/{}/messages/@original",
                self.application_id,
                self.token.segment()
            ),
            MajorParameter::Webhook(self.application_id.cast(), self.token.hash()),
        )
        .with_path_token()
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        edit_message_body(&self.payload, &self.files, validate)
    }
}

/// `DELETE /webhooks/{application.id}/{interaction.token}/messages/@original`: deletes the
/// first response to an interaction.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteOriginalInteractionResponse {
    client: Client,
    application_id: ApplicationId,
    token: PathToken,
}

impl Endpoint for DeleteOriginalInteractionResponse {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/webhooks/{application_id}/{interaction_token}/messages/@original",
            format!(
                "/webhooks/{}/{}/messages/@original",
                self.application_id,
                self.token.segment()
            ),
            MajorParameter::Webhook(self.application_id.cast(), self.token.hash()),
        )
        .with_path_token()
    }
}

/// `POST /webhooks/{application.id}/{interaction.token}`: sends a follow-up message for an
/// interaction.
///
/// A user-installed app that the guild did not install can send 5 follow-up messages for
/// each interaction.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateFollowup {
    client: Client,
    application_id: ApplicationId,
    token: PathToken,
    payload: MessagePayload,
    files: Vec<FileUpload>,
}

impl CreateFollowup {
    message_setters!();

    /// Makes the message visible only to the user of the interaction.
    pub fn ephemeral(mut self) -> Self {
        let flags = self.payload.flags.unwrap_or_default();
        self.payload.flags = Some(flags | MessageFlags::EPHEMERAL);
        self
    }
}

impl Endpoint for CreateFollowup {
    type Response = Message;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/webhooks/{application_id}/{interaction_token}",
            format!("/webhooks/{}/{}", self.application_id, self.token.segment()),
            MajorParameter::Webhook(self.application_id.cast(), self.token.hash()),
        )
        .with_path_token()
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        self.payload.validate(validate, &self.files)?;
        check(validate, || {
            check_permitted_flags(&self.payload, FOLLOWUP_FLAGS)?;
            if is_empty_message(&self.payload, &self.files) {
                return Err(ValidationError::new(
                    "message",
                    "needs content, embeds, components, a poll, or files",
                ));
            }
            Ok(())
        })?;

        let mut payload = self.payload.clone();
        payload.fill_attachments(&self.files);
        RequestBody::json_with_files(&payload, &self.files)
    }
}

/// `GET /webhooks/{application.id}/{interaction.token}/messages/{message.id}`: a follow-up
/// message.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetFollowup {
    client: Client,
    application_id: ApplicationId,
    token: PathToken,
    message_id: MessageId,
}

impl Endpoint for GetFollowup {
    type Response = Message;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/webhooks/{application_id}/{interaction_token}/messages/{message_id}",
            format!(
                "/webhooks/{}/{}/messages/{}",
                self.application_id,
                self.token.segment(),
                self.message_id
            ),
            MajorParameter::Webhook(self.application_id.cast(), self.token.hash()),
        )
        .with_path_token()
    }
}

/// `PATCH /webhooks/{application.id}/{interaction.token}/messages/{message.id}`: edits a
/// follow-up message.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditFollowup {
    client: Client,
    application_id: ApplicationId,
    token: PathToken,
    message_id: MessageId,
    payload: MessagePayload,
    files: Vec<FileUpload>,
}

impl EditFollowup {
    message_setters!();

    /// Removes the content.
    pub fn clear_content(mut self) -> Self {
        self.payload.content = Some(String::new());
        self
    }
}

impl Endpoint for EditFollowup {
    type Response = Message;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/webhooks/{application_id}/{interaction_token}/messages/{message_id}",
            format!(
                "/webhooks/{}/{}/messages/{}",
                self.application_id,
                self.token.segment(),
                self.message_id
            ),
            MajorParameter::Webhook(self.application_id.cast(), self.token.hash()),
        )
        .with_path_token()
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        edit_message_body(&self.payload, &self.files, validate)
    }
}

/// `DELETE /webhooks/{application.id}/{interaction.token}/messages/{message.id}`: deletes a
/// follow-up message.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteFollowup {
    client: Client,
    application_id: ApplicationId,
    token: PathToken,
    message_id: MessageId,
}

impl Endpoint for DeleteFollowup {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/webhooks/{application_id}/{interaction_token}/messages/{message_id}",
            format!(
                "/webhooks/{}/{}/messages/{}",
                self.application_id,
                self.token.segment(),
                self.message_id
            ),
            MajorParameter::Webhook(self.application_id.cast(), self.token.hash()),
        )
        .with_path_token()
    }
}

super::into_future!(
    CreateInteractionResponse,
    CreateInteractionResponseWithCallback,
    GetOriginalInteractionResponse,
    EditOriginalInteractionResponse,
    DeleteOriginalInteractionResponse,
    CreateFollowup,
    GetFollowup,
    EditFollowup,
    DeleteFollowup,
);

impl Client {
    /// Responds to an interaction. Call [`CreateInteractionResponse::with_response`] to get
    /// the interaction callback response.
    pub fn create_interaction_response(
        &self,
        interaction_id: InteractionId,
        token: impl Into<String>,
        response: InteractionResponse,
    ) -> CreateInteractionResponse {
        CreateInteractionResponse {
            client: self.clone(),
            interaction_id,
            token: PathToken::new(token),
            response,
            files: Vec::new(),
        }
    }

    /// Gets the first response to an interaction.
    pub fn original_interaction_response(
        &self,
        application_id: ApplicationId,
        token: impl Into<String>,
    ) -> GetOriginalInteractionResponse {
        GetOriginalInteractionResponse {
            client: self.clone(),
            application_id,
            token: PathToken::new(token),
        }
    }

    /// Edits the first response to an interaction.
    pub fn edit_original_interaction_response(
        &self,
        application_id: ApplicationId,
        token: impl Into<String>,
    ) -> EditOriginalInteractionResponse {
        EditOriginalInteractionResponse {
            client: self.clone(),
            application_id,
            token: PathToken::new(token),
            payload: MessagePayload::default(),
            files: Vec::new(),
        }
    }

    /// Deletes the first response to an interaction.
    pub fn delete_original_interaction_response(
        &self,
        application_id: ApplicationId,
        token: impl Into<String>,
    ) -> DeleteOriginalInteractionResponse {
        DeleteOriginalInteractionResponse {
            client: self.clone(),
            application_id,
            token: PathToken::new(token),
        }
    }

    /// Sends a follow-up message for an interaction.
    pub fn create_followup(
        &self,
        application_id: ApplicationId,
        token: impl Into<String>,
    ) -> CreateFollowup {
        CreateFollowup {
            client: self.clone(),
            application_id,
            token: PathToken::new(token),
            payload: MessagePayload::default(),
            files: Vec::new(),
        }
    }

    /// Gets a follow-up message.
    pub fn followup(
        &self,
        application_id: ApplicationId,
        token: impl Into<String>,
        message_id: MessageId,
    ) -> GetFollowup {
        GetFollowup {
            client: self.clone(),
            application_id,
            token: PathToken::new(token),
            message_id,
        }
    }

    /// Edits a follow-up message.
    pub fn edit_followup(
        &self,
        application_id: ApplicationId,
        token: impl Into<String>,
        message_id: MessageId,
    ) -> EditFollowup {
        EditFollowup {
            client: self.clone(),
            application_id,
            token: PathToken::new(token),
            message_id,
            payload: MessagePayload::default(),
            files: Vec::new(),
        }
    }

    /// Deletes a follow-up message.
    pub fn delete_followup(
        &self,
        application_id: ApplicationId,
        token: impl Into<String>,
        message_id: MessageId,
    ) -> DeleteFollowup {
        DeleteFollowup {
            client: self.clone(),
            application_id,
            token: PathToken::new(token),
            message_id,
        }
    }
}
