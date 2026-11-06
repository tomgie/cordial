//! Invite endpoints: the invites of a channel, invites by code, and invite target users.

use cordial_model::id::{ApplicationId, ChannelId, RoleId, ScheduledEventId, UserId};
use cordial_model::invite::{Invite, InviteTargetType, InviteTargetUsersJob};
use cordial_model::validate::ValidationError;
use serde::Serialize;

use super::channel::check_count;
use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, FileUpload, Multipart, RequestBody, check};
use crate::route::{MajorParameter, Method, Query, Route, encode_segment};

/// The maximum number of user IDs in one target user request.
const TARGET_USER_LIMIT: usize = 1000;

/// The part name of the CSV file with target users.
const TARGET_USERS_FILE: &str = "target_users_file";

/// Gives the CSV file with the media type `text/csv` when the caller did not set one.
fn csv_file(file: &FileUpload) -> FileUpload {
    let mut file = file.clone();
    if file.content_type.is_none() {
        file.content_type = Some("text/csv".to_owned());
    }
    file
}

/// `GET /channels/{channel.id}/invites`: the invites of a guild channel, with metadata.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetChannelInvites {
    client: Client,
    channel_id: ChannelId,
}

impl Endpoint for GetChannelInvites {
    type Response = Vec<Invite>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/channels/{channel_id}/invites",
            format!("/channels/{}/invites", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `POST /channels/{channel.id}/invites`: makes an invite for a guild channel.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateChannelInvite {
    client: Client,
    channel_id: ChannelId,
    fields: CreateInviteFields,
    target_users_file: Option<FileUpload>,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct CreateInviteFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_age: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_uses: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temporary: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unique: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_type: Option<InviteTargetType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_user_id: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_application_id: Option<ApplicationId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_user_ids: Option<Vec<UserId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role_ids: Option<Vec<RoleId>>,
}

impl CreateChannelInvite {
    /// Sets the time in seconds after which the invite expires. 0 means never. The default
    /// is 86400 (24 hours).
    pub fn max_age(mut self, seconds: u32) -> Self {
        self.fields.max_age = Some(seconds);
        self
    }

    /// Sets the maximum number of uses, 0 to 100. 0 means no limit.
    pub fn max_uses(mut self, max_uses: u32) -> Self {
        self.fields.max_uses = Some(max_uses);
        self
    }

    /// Sets whether the invite gives only temporary membership.
    pub fn temporary(mut self, temporary: bool) -> Self {
        self.fields.temporary = Some(temporary);
        self
    }

    /// Makes a new invite, also when a similar invite exists.
    pub fn unique(mut self, unique: bool) -> Self {
        self.fields.unique = Some(unique);
        self
    }

    /// Makes an invite to the stream of a user in a voice channel. The user must stream in
    /// the channel.
    pub fn target_stream(mut self, user_id: UserId) -> Self {
        self.fields.target_type = Some(InviteTargetType::Stream);
        self.fields.target_user_id = Some(user_id);
        self
    }

    /// Makes an invite that opens an embedded application in a voice channel. The
    /// application must have the `EMBEDDED` flag.
    pub fn target_application(mut self, application_id: ApplicationId) -> Self {
        self.fields.target_type = Some(InviteTargetType::EmbeddedApplication);
        self.fields.target_application_id = Some(application_id);
        self
    }

    /// Sets the only users that can see and accept the invite (up to 1000). Do not use it
    /// together with [`CreateChannelInvite::target_users_file`].
    pub fn target_user_ids(mut self, user_ids: Vec<UserId>) -> Self {
        self.fields.target_user_ids = Some(user_ids);
        self
    }

    /// Sets the only users that can see and accept the invite, as a CSV file with one
    /// column of user IDs. The request then uses a multipart body.
    pub fn target_users_file(mut self, file: FileUpload) -> Self {
        self.target_users_file = Some(file);
        self
    }

    /// Sets the roles that a user gets when the user accepts the invite. This needs the
    /// `MANAGE_ROLES` permission.
    pub fn role_ids(mut self, role_ids: Vec<RoleId>) -> Self {
        self.fields.role_ids = Some(role_ids);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    fn validate(&self) -> Result<(), ValidationError> {
        if let Some(max_uses) = self.fields.max_uses
            && max_uses > 100
        {
            return Err(ValidationError::new(
                "max_uses",
                format!("is {max_uses}; the maximum is 100"),
            ));
        }
        if let Some(user_ids) = &self.fields.target_user_ids {
            check_count("target_user_ids", user_ids.len(), TARGET_USER_LIMIT)?;
            if self.target_users_file.is_some() {
                return Err(ValidationError::new(
                    "target_user_ids",
                    "cannot be sent together with target_users_file",
                ));
            }
        }
        Ok(())
    }
}

impl Endpoint for CreateChannelInvite {
    type Response = Invite;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/channels/{channel_id}/invites",
            format!("/channels/{}/invites", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || self.validate())?;

        // Discord needs a body, also when no field is set: `{}`.
        let Some(file) = &self.target_users_file else {
            return RequestBody::json(&self.fields);
        };
        let payload = serde_json::to_vec(&self.fields).map_err(Error::Encode)?;
        let mut multipart = Multipart::new().named_file(TARGET_USERS_FILE, csv_file(file));
        multipart.payload_json = Some(payload.into());
        Ok(RequestBody::Multipart(multipart))
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `GET /invites/{invite.code}`: an invite by its code.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetInvite {
    client: Client,
    code: String,
    with_counts: Option<bool>,
    guild_scheduled_event_id: Option<ScheduledEventId>,
}

impl GetInvite {
    /// Includes the approximate member counts.
    pub fn with_counts(mut self, with_counts: bool) -> Self {
        self.with_counts = Some(with_counts);
        self
    }

    /// Includes a scheduled event of the guild.
    pub fn scheduled_event(mut self, event_id: ScheduledEventId) -> Self {
        self.guild_scheduled_event_id = Some(event_id);
        self
    }
}

impl Endpoint for GetInvite {
    type Response = Invite;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("with_counts", self.with_counts);
        query.push_opt("guild_scheduled_event_id", self.guild_scheduled_event_id);
        Route::new(
            Method::Get,
            "/invites/{code}",
            format!("/invites/{}{}", encode_segment(&self.code), query.finish()),
            MajorParameter::None,
        )
    }
}

/// `DELETE /invites/{invite.code}`: deletes an invite.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteInvite {
    client: Client,
    code: String,
    reason: Option<String>,
}

impl DeleteInvite {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for DeleteInvite {
    type Response = Invite;

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/invites/{code}",
            format!("/invites/{}", encode_segment(&self.code)),
            MajorParameter::None,
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `GET /invites/{invite.code}/target-users`: the users that can see and accept an invite.
///
/// The response is a CSV text with the header `user_id` and one user ID on each line.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetInviteTargetUsers {
    client: Client,
    code: String,
}

impl Endpoint for GetInviteTargetUsers {
    // The body is CSV, not JSON. `IntoFuture` reads the raw body.
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/invites/{code}/target-users",
            format!("/invites/{}/target-users", encode_segment(&self.code)),
            MajorParameter::None,
        )
    }
}

impl std::future::IntoFuture for GetInviteTargetUsers {
    type Output = Result<String, Error>;
    type IntoFuture = crate::transport::BoxFuture<'static, Self::Output>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let body = self.client.request_raw(&self).await?;
            Ok(String::from_utf8_lossy(&body).into_owned())
        })
    }
}

/// `PUT /invites/{invite.code}/target-users`: replaces the users that can see and accept an
/// invite, from a CSV file.
///
/// Discord processes the file in the background. See
/// [`Client::invite_target_users_job_status`].
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct UpdateInviteTargetUsers {
    client: Client,
    code: String,
    file: FileUpload,
}

impl Endpoint for UpdateInviteTargetUsers {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/invites/{code}/target-users",
            format!("/invites/{}/target-users", encode_segment(&self.code)),
            MajorParameter::None,
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        let multipart = Multipart::new().named_file(TARGET_USERS_FILE, csv_file(&self.file));
        Ok(RequestBody::Multipart(multipart))
    }
}

/// `PUT /invites/{invite.code}/target-users/{user.id}`: adds a target user to an invite.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct AddInviteTargetUser {
    client: Client,
    code: String,
    user_id: UserId,
}

impl Endpoint for AddInviteTargetUser {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/invites/{code}/target-users/{user_id}",
            format!(
                "/invites/{}/target-users/{}",
                encode_segment(&self.code),
                self.user_id
            ),
            MajorParameter::None,
        )
    }
}

/// `DELETE /invites/{invite.code}/target-users/{user.id}`: removes a target user from an
/// invite.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct RemoveInviteTargetUser {
    client: Client,
    code: String,
    user_id: UserId,
}

impl Endpoint for RemoveInviteTargetUser {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/invites/{code}/target-users/{user_id}",
            format!(
                "/invites/{}/target-users/{}",
                encode_segment(&self.code),
                self.user_id
            ),
            MajorParameter::None,
        )
    }
}

#[derive(Clone, Debug, Serialize)]
struct UserIdsBody {
    user_ids: Vec<UserId>,
}

impl UserIdsBody {
    fn validate(&self) -> Result<(), ValidationError> {
        if self.user_ids.is_empty() {
            return Err(ValidationError::new("user_ids", "is empty"));
        }
        check_count("user_ids", self.user_ids.len(), TARGET_USER_LIMIT)
    }
}

/// `POST /invites/{invite.code}/target-users/bulk-add`: adds up to 1000 target users to an
/// invite.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct BulkAddInviteTargetUsers {
    client: Client,
    code: String,
    body: UserIdsBody,
}

impl Endpoint for BulkAddInviteTargetUsers {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/invites/{code}/target-users/bulk-add",
            format!(
                "/invites/{}/target-users/bulk-add",
                encode_segment(&self.code)
            ),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || self.body.validate())?;
        RequestBody::json(&self.body)
    }

    fn retry_safe(&self) -> bool {
        // Adding the same users again has no other effect.
        true
    }
}

/// `POST /invites/{invite.code}/target-users/bulk-delete`: removes up to 1000 target users
/// from an invite.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct BulkDeleteInviteTargetUsers {
    client: Client,
    code: String,
    body: UserIdsBody,
}

impl Endpoint for BulkDeleteInviteTargetUsers {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/invites/{code}/target-users/bulk-delete",
            format!(
                "/invites/{}/target-users/bulk-delete",
                encode_segment(&self.code)
            ),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || self.body.validate())?;
        RequestBody::json(&self.body)
    }

    fn retry_safe(&self) -> bool {
        // Removing the same users again has no other effect.
        true
    }
}

/// `GET /invites/{invite.code}/target-users/job-status`: the state of the background job
/// that processes a target users file.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetInviteTargetUsersJobStatus {
    client: Client,
    code: String,
}

impl Endpoint for GetInviteTargetUsersJobStatus {
    type Response = InviteTargetUsersJob;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/invites/{code}/target-users/job-status",
            format!(
                "/invites/{}/target-users/job-status",
                encode_segment(&self.code)
            ),
            MajorParameter::None,
        )
    }
}

super::into_future!(
    GetChannelInvites,
    CreateChannelInvite,
    GetInvite,
    DeleteInvite,
    UpdateInviteTargetUsers,
    AddInviteTargetUser,
    RemoveInviteTargetUser,
    BulkAddInviteTargetUsers,
    BulkDeleteInviteTargetUsers,
    GetInviteTargetUsersJobStatus,
);

impl Client {
    /// Gets the invites of a guild channel, with metadata.
    pub fn channel_invites(&self, channel_id: ChannelId) -> GetChannelInvites {
        GetChannelInvites {
            client: self.clone(),
            channel_id,
        }
    }

    /// Makes an invite for a guild channel.
    pub fn create_channel_invite(&self, channel_id: ChannelId) -> CreateChannelInvite {
        CreateChannelInvite {
            client: self.clone(),
            channel_id,
            fields: CreateInviteFields::default(),
            target_users_file: None,
            reason: None,
        }
    }

    /// Gets an invite by its code.
    pub fn invite(&self, code: impl Into<String>) -> GetInvite {
        GetInvite {
            client: self.clone(),
            code: code.into(),
            with_counts: None,
            guild_scheduled_event_id: None,
        }
    }

    /// Deletes an invite.
    pub fn delete_invite(&self, code: impl Into<String>) -> DeleteInvite {
        DeleteInvite {
            client: self.clone(),
            code: code.into(),
            reason: None,
        }
    }

    /// Gets the users that can see and accept an invite, as CSV text.
    pub fn invite_target_users(&self, code: impl Into<String>) -> GetInviteTargetUsers {
        GetInviteTargetUsers {
            client: self.clone(),
            code: code.into(),
        }
    }

    /// Replaces the users that can see and accept an invite, from a CSV file with one column
    /// of user IDs.
    pub fn update_invite_target_users(
        &self,
        code: impl Into<String>,
        file: FileUpload,
    ) -> UpdateInviteTargetUsers {
        UpdateInviteTargetUsers {
            client: self.clone(),
            code: code.into(),
            file,
        }
    }

    /// Adds a target user to an invite.
    pub fn add_invite_target_user(
        &self,
        code: impl Into<String>,
        user_id: UserId,
    ) -> AddInviteTargetUser {
        AddInviteTargetUser {
            client: self.clone(),
            code: code.into(),
            user_id,
        }
    }

    /// Removes a target user from an invite.
    pub fn remove_invite_target_user(
        &self,
        code: impl Into<String>,
        user_id: UserId,
    ) -> RemoveInviteTargetUser {
        RemoveInviteTargetUser {
            client: self.clone(),
            code: code.into(),
            user_id,
        }
    }

    /// Adds 1 to 1000 target users to an invite.
    pub fn bulk_add_invite_target_users(
        &self,
        code: impl Into<String>,
        user_ids: Vec<UserId>,
    ) -> BulkAddInviteTargetUsers {
        BulkAddInviteTargetUsers {
            client: self.clone(),
            code: code.into(),
            body: UserIdsBody { user_ids },
        }
    }

    /// Removes 1 to 1000 target users from an invite.
    pub fn bulk_delete_invite_target_users(
        &self,
        code: impl Into<String>,
        user_ids: Vec<UserId>,
    ) -> BulkDeleteInviteTargetUsers {
        BulkDeleteInviteTargetUsers {
            client: self.clone(),
            code: code.into(),
            body: UserIdsBody { user_ids },
        }
    }

    /// Gets the state of the background job that processes a target users file.
    pub fn invite_target_users_job_status(
        &self,
        code: impl Into<String>,
    ) -> GetInviteTargetUsersJobStatus {
        GetInviteTargetUsersJobStatus {
            client: self.clone(),
            code: code.into(),
        }
    }
}
