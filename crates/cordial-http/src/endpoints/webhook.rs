//! Webhook endpoints.
//!
//! The routes with a webhook token in the path send no `Authorization` header, and the global
//! rate limit does not apply to them.

use std::fmt;

use cordial_model::Patch;
use cordial_model::id::{ChannelId, ForumTagId, GuildId, MessageId, WebhookId};
use cordial_model::message::{Message, MessageFlags};
use cordial_model::validate::ValidationError;
use cordial_model::webhook::Webhook;
use serde::Serialize;

use crate::client::Client;
use crate::error::Error;
use crate::payload::{MessagePayload, message_setters};
use crate::request::{Endpoint, FileUpload, ImageData, RequestBody, check};
use crate::route::{MajorParameter, Method, Query, Route, encode_segment, token_hash};

/// A webhook or interaction token for a path. The `Debug` output hides it.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PathToken(String);

impl PathToken {
    pub(crate) fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// Gives the token as an encoded path segment.
    pub(crate) fn segment(&self) -> String {
        encode_segment(&self.0)
    }

    /// Gives the hash for the rate limit key.
    pub(crate) fn hash(&self) -> u64 {
        token_hash(&self.0)
    }
}

impl fmt::Debug for PathToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PathToken(<hidden>)")
    }
}

/// Gives an error when the message has flags other than the permitted flags.
pub(crate) fn check_permitted_flags(
    payload: &MessagePayload,
    permitted: MessageFlags,
) -> Result<(), ValidationError> {
    let flags = payload.flags.unwrap_or_default();
    if !flags.difference(permitted).is_empty() {
        return Err(ValidationError::new(
            "flags",
            format!("only these flags are permitted: {permitted:?}"),
        ));
    }
    Ok(())
}

/// Tells if a new message has nothing to show.
pub(crate) fn is_empty_message(payload: &MessagePayload, files: &[FileUpload]) -> bool {
    payload.content.as_deref().is_none_or(str::is_empty)
        && payload.embeds.as_ref().is_none_or(Vec::is_empty)
        && payload.components.as_ref().is_none_or(Vec::is_empty)
        && payload.poll.is_none()
        && files.is_empty()
}

/// Gives the JSON or multipart body of a message edit.
pub(crate) fn edit_message_body(
    payload: &MessagePayload,
    files: &[FileUpload],
    validate: bool,
) -> Result<RequestBody, Error> {
    payload.validate(validate, files)?;
    let mut payload = payload.clone();
    payload.fill_attachments(files);
    RequestBody::json_with_files(&payload, files)
}

/// The largest length of a webhook name, in characters.
const WEBHOOK_NAME_MAX: usize = 80;

/// Checks a webhook name: 1 to 80 characters, without `clyde` or `discord`.
fn check_webhook_name(name: &str) -> Result<(), ValidationError> {
    let length = name.chars().count();
    if !(1..=WEBHOOK_NAME_MAX).contains(&length) {
        return Err(ValidationError::new(
            "name",
            format!("has {length} characters; the range is 1 to 80"),
        ));
    }
    let lowercase = name.to_lowercase();
    if lowercase.contains("clyde") || lowercase.contains("discord") {
        return Err(ValidationError::new(
            "name",
            "cannot contain `clyde` or `discord`",
        ));
    }
    Ok(())
}

/// `POST /channels/{channel.id}/webhooks`: makes a webhook.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateWebhook {
    client: Client,
    channel_id: ChannelId,
    fields: CreateWebhookBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct CreateWebhookBody {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar: Option<ImageData>,
}

impl CreateWebhook {
    /// Sets the default avatar.
    pub fn avatar(mut self, avatar: ImageData) -> Self {
        self.fields.avatar = Some(avatar);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for CreateWebhook {
    type Response = Webhook;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/channels/{channel_id}/webhooks",
            format!("/channels/{}/webhooks", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || check_webhook_name(&self.fields.name))?;
        RequestBody::json(&self.fields)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `GET /channels/{channel.id}/webhooks`: the webhooks of a channel.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetChannelWebhooks {
    client: Client,
    channel_id: ChannelId,
}

impl Endpoint for GetChannelWebhooks {
    type Response = Vec<Webhook>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/channels/{channel_id}/webhooks",
            format!("/channels/{}/webhooks", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `GET /guilds/{guild.id}/webhooks`: the webhooks of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildWebhooks {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildWebhooks {
    type Response = Vec<Webhook>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/webhooks",
            format!("/guilds/{}/webhooks", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /webhooks/{webhook.id}`: one webhook.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetWebhook {
    client: Client,
    webhook_id: WebhookId,
}

impl Endpoint for GetWebhook {
    type Response = Webhook;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/webhooks/{webhook_id}",
            format!("/webhooks/{}", self.webhook_id),
            MajorParameter::None,
        )
    }
}

/// `GET /webhooks/{webhook.id}/{webhook.token}`: one webhook, with its token instead of the
/// client token. The response has no `user`.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetWebhookWithToken {
    client: Client,
    webhook_id: WebhookId,
    token: PathToken,
}

impl Endpoint for GetWebhookWithToken {
    type Response = Webhook;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/webhooks/{webhook_id}/{webhook_token}",
            format!("/webhooks/{}/{}", self.webhook_id, self.token.segment()),
            MajorParameter::Webhook(self.webhook_id, self.token.hash()),
        )
        .with_path_token()
    }
}

#[derive(Clone, Debug, Default, Serialize)]
struct ModifyWebhookBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    avatar: Patch<ImageData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_id: Option<ChannelId>,
}

impl ModifyWebhookBody {
    fn encode(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || match &self.name {
            Some(name) => check_webhook_name(name),
            None => Ok(()),
        })?;
        RequestBody::json(self)
    }
}

/// `PATCH /webhooks/{webhook.id}`: changes a webhook.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct ModifyWebhook {
    client: Client,
    webhook_id: WebhookId,
    fields: ModifyWebhookBody,
    reason: Option<String>,
}

impl ModifyWebhook {
    /// Sets the default name (1 to 80 characters).
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.fields.name = Some(name.into());
        self
    }

    /// Sets the default avatar.
    pub fn avatar(mut self, avatar: ImageData) -> Self {
        self.fields.avatar = Patch::Set(avatar);
        self
    }

    /// Removes the default avatar.
    pub fn clear_avatar(mut self) -> Self {
        self.fields.avatar = Patch::Clear;
        self
    }

    /// Moves the webhook to a different channel.
    pub fn channel_id(mut self, channel_id: ChannelId) -> Self {
        self.fields.channel_id = Some(channel_id);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for ModifyWebhook {
    type Response = Webhook;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/webhooks/{webhook_id}",
            format!("/webhooks/{}", self.webhook_id),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        self.fields.encode(validate)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `PATCH /webhooks/{webhook.id}/{webhook.token}`: changes a webhook, with its token instead
/// of the client token. It cannot move the webhook to a different channel.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct ModifyWebhookWithToken {
    client: Client,
    webhook_id: WebhookId,
    token: PathToken,
    fields: ModifyWebhookBody,
    reason: Option<String>,
}

impl ModifyWebhookWithToken {
    /// Sets the default name (1 to 80 characters).
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.fields.name = Some(name.into());
        self
    }

    /// Sets the default avatar.
    pub fn avatar(mut self, avatar: ImageData) -> Self {
        self.fields.avatar = Patch::Set(avatar);
        self
    }

    /// Removes the default avatar.
    pub fn clear_avatar(mut self) -> Self {
        self.fields.avatar = Patch::Clear;
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for ModifyWebhookWithToken {
    type Response = Webhook;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/webhooks/{webhook_id}/{webhook_token}",
            format!("/webhooks/{}/{}", self.webhook_id, self.token.segment()),
            MajorParameter::Webhook(self.webhook_id, self.token.hash()),
        )
        .with_path_token()
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        self.fields.encode(validate)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `DELETE /webhooks/{webhook.id}`: deletes a webhook.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteWebhook {
    client: Client,
    webhook_id: WebhookId,
    reason: Option<String>,
}

impl DeleteWebhook {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for DeleteWebhook {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/webhooks/{webhook_id}",
            format!("/webhooks/{}", self.webhook_id),
            MajorParameter::None,
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `DELETE /webhooks/{webhook.id}/{webhook.token}`: deletes a webhook, with its token instead
/// of the client token.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteWebhookWithToken {
    client: Client,
    webhook_id: WebhookId,
    token: PathToken,
    reason: Option<String>,
}

impl DeleteWebhookWithToken {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for DeleteWebhookWithToken {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/webhooks/{webhook_id}/{webhook_token}",
            format!("/webhooks/{}/{}", self.webhook_id, self.token.segment()),
            MajorParameter::Webhook(self.webhook_id, self.token.hash()),
        )
        .with_path_token()
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// The fields of an execute request that a channel message does not have.
#[derive(Clone, Debug, Default, Serialize)]
struct ExecuteWebhookExtra {
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    applied_tags: Option<Vec<ForumTagId>>,
}

#[derive(Serialize)]
struct ExecuteWebhookBody<'a> {
    #[serde(flatten)]
    payload: &'a MessagePayload,
    #[serde(flatten)]
    extra: &'a ExecuteWebhookExtra,
}

/// The query parameters of an execute request.
#[derive(Clone, Copy, Debug, Default)]
struct ExecuteWebhookQuery {
    thread_id: Option<ChannelId>,
    with_components: Option<bool>,
}

/// The flags that an execute request can set.
const EXECUTE_FLAGS: MessageFlags = MessageFlags::SUPPRESS_EMBEDS
    .union(MessageFlags::SUPPRESS_NOTIFICATIONS)
    .union(MessageFlags::IS_COMPONENTS_V2);

fn execute_route(
    webhook_id: WebhookId,
    token: &PathToken,
    wait: bool,
    options: ExecuteWebhookQuery,
) -> Route {
    let mut query = Query::new();
    if wait {
        query.push("wait", true);
    }
    query.push_opt("thread_id", options.thread_id);
    query.push_opt("with_components", options.with_components);
    Route::new(
        Method::Post,
        "/webhooks/{webhook_id}/{webhook_token}",
        format!(
            "/webhooks/{}/{}{}",
            webhook_id,
            token.segment(),
            query.finish()
        ),
        MajorParameter::Webhook(webhook_id, token.hash()),
    )
    .with_path_token()
}

fn execute_body(
    payload: &MessagePayload,
    extra: &ExecuteWebhookExtra,
    files: &[FileUpload],
    validate: bool,
) -> Result<RequestBody, Error> {
    payload.validate(validate, files)?;
    check(validate, || {
        check_permitted_flags(payload, EXECUTE_FLAGS)?;
        if is_empty_message(payload, files) {
            return Err(ValidationError::new(
                "message",
                "needs content, embeds, components, a poll, or files",
            ));
        }
        Ok(())
    })?;

    let mut payload = payload.clone();
    payload.fill_attachments(files);
    let body = ExecuteWebhookBody {
        payload: &payload,
        extra,
    };
    RequestBody::json_with_files(&body, files)
}

/// Adds the setters that are only for the execute requests.
macro_rules! execute_setters {
    () => {
        /// Sets a name that replaces the default name of the webhook.
        pub fn username(mut self, username: impl Into<String>) -> Self {
            self.extra.username = Some(username.into());
            self
        }

        /// Sets an avatar URL that replaces the default avatar of the webhook.
        pub fn avatar_url(mut self, avatar_url: impl Into<String>) -> Self {
            self.extra.avatar_url = Some(avatar_url.into());
            self
        }

        /// Sends the message in a thread of the webhook channel. Discord unarchives the thread.
        pub fn thread_id(mut self, thread_id: ChannelId) -> Self {
            self.query.thread_id = Some(thread_id);
            self
        }

        /// Makes a thread with this name. Only for a forum or media channel.
        pub fn thread_name(mut self, thread_name: impl Into<String>) -> Self {
            self.extra.thread_name = Some(thread_name.into());
            self
        }

        /// Sets the tags of the new thread. Only for a forum or media channel.
        pub fn applied_tags(mut self, tag_ids: Vec<ForumTagId>) -> Self {
            self.extra.applied_tags = Some(tag_ids);
            self
        }

        /// Makes Discord use the components. A webhook that no application owns can then send
        /// components that are not interactive.
        pub fn with_components(mut self, with_components: bool) -> Self {
            self.query.with_components = Some(with_components);
            self
        }
    };
}

/// `POST /webhooks/{webhook.id}/{webhook.token}`: sends a message with a webhook.
///
/// Discord does not wait until it saves the message, and gives no message. Call
/// [`wait`](Self::wait) to get the message.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct ExecuteWebhook {
    client: Client,
    webhook_id: WebhookId,
    token: PathToken,
    payload: MessagePayload,
    extra: ExecuteWebhookExtra,
    files: Vec<FileUpload>,
    query: ExecuteWebhookQuery,
}

impl ExecuteWebhook {
    message_setters!();
    execute_setters!();

    /// Waits until Discord saves the message, and gives the message (`wait=true`). Discord
    /// then also gives an error when it cannot save the message.
    pub fn wait(self) -> ExecuteWebhookAndWait {
        ExecuteWebhookAndWait {
            client: self.client,
            webhook_id: self.webhook_id,
            token: self.token,
            payload: self.payload,
            extra: self.extra,
            files: self.files,
            query: self.query,
        }
    }
}

impl Endpoint for ExecuteWebhook {
    type Response = ();

    fn route(&self) -> Route {
        execute_route(self.webhook_id, &self.token, false, self.query)
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        execute_body(&self.payload, &self.extra, &self.files, validate)
    }
}

/// `POST /webhooks/{webhook.id}/{webhook.token}?wait=true`: sends a message with a webhook,
/// and gives the message.
///
/// Make it with [`ExecuteWebhook::wait`].
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct ExecuteWebhookAndWait {
    client: Client,
    webhook_id: WebhookId,
    token: PathToken,
    payload: MessagePayload,
    extra: ExecuteWebhookExtra,
    files: Vec<FileUpload>,
    query: ExecuteWebhookQuery,
}

impl ExecuteWebhookAndWait {
    message_setters!();
    execute_setters!();
}

impl Endpoint for ExecuteWebhookAndWait {
    type Response = Message;

    fn route(&self) -> Route {
        execute_route(self.webhook_id, &self.token, true, self.query)
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        execute_body(&self.payload, &self.extra, &self.files, validate)
    }
}

/// The query parameters of the Slack and GitHub compatible execute requests.
fn compatible_query(thread_id: Option<ChannelId>, wait: Option<bool>) -> String {
    let mut query = Query::new();
    query.push_opt("thread_id", thread_id);
    query.push_opt("wait", wait);
    query.finish()
}

/// `POST /webhooks/{webhook.id}/{webhook.token}/slack`: sends a message in the format of a
/// Slack incoming webhook.
///
/// The body is the Slack JSON payload. Discord does not support `channel`, `icon_emoji`,
/// `mrkdwn`, or `mrkdwn_in`. The response body is not JSON, thus `.await` ignores it.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct ExecuteSlackWebhook {
    client: Client,
    webhook_id: WebhookId,
    token: PathToken,
    payload: serde_json::Value,
    thread_id: Option<ChannelId>,
    wait: Option<bool>,
}

impl ExecuteSlackWebhook {
    /// Sends the message in a thread of the webhook channel.
    pub fn thread_id(mut self, thread_id: ChannelId) -> Self {
        self.thread_id = Some(thread_id);
        self
    }

    /// Sets whether Discord waits until it saves the message. The default is `true`.
    pub fn wait(mut self, wait: bool) -> Self {
        self.wait = Some(wait);
        self
    }
}

impl Endpoint for ExecuteSlackWebhook {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/webhooks/{webhook_id}/{webhook_token}/slack",
            format!(
                "/webhooks/{}/{}/slack{}",
                self.webhook_id,
                self.token.segment(),
                compatible_query(self.thread_id, self.wait)
            ),
            MajorParameter::Webhook(self.webhook_id, self.token.hash()),
        )
        .with_path_token()
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.payload)
    }
}

/// `POST /webhooks/{webhook.id}/{webhook.token}/github`: sends a message for a GitHub event.
///
/// The body is the GitHub event payload. Discord supports only some events, for example
/// `push`, `issues`, and `pull_request`.
///
/// GitHub gives the event name in the `X-GitHub-Event` header. This request cannot send that
/// header yet, because the `Endpoint` trait has no custom headers.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct ExecuteGitHubWebhook {
    client: Client,
    webhook_id: WebhookId,
    token: PathToken,
    payload: serde_json::Value,
    thread_id: Option<ChannelId>,
    wait: Option<bool>,
}

impl ExecuteGitHubWebhook {
    /// Sends the message in a thread of the webhook channel.
    pub fn thread_id(mut self, thread_id: ChannelId) -> Self {
        self.thread_id = Some(thread_id);
        self
    }

    /// Sets whether Discord waits until it saves the message. The default is `true`.
    pub fn wait(mut self, wait: bool) -> Self {
        self.wait = Some(wait);
        self
    }
}

impl Endpoint for ExecuteGitHubWebhook {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/webhooks/{webhook_id}/{webhook_token}/github",
            format!(
                "/webhooks/{}/{}/github{}",
                self.webhook_id,
                self.token.segment(),
                compatible_query(self.thread_id, self.wait)
            ),
            MajorParameter::Webhook(self.webhook_id, self.token.hash()),
        )
        .with_path_token()
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.payload)
    }
}

/// Implements `IntoFuture` for a request whose response body is not JSON. The body is
/// ignored.
macro_rules! into_future_ignore_body {
    ($($name:ident),* $(,)?) => {
        $(
            impl ::std::future::IntoFuture for $name {
                type Output = ::std::result::Result<(), crate::error::Error>;
                type IntoFuture = crate::transport::BoxFuture<'static, Self::Output>;

                fn into_future(self) -> Self::IntoFuture {
                    Box::pin(async move { self.client.request_raw(&self).await.map(|_| ()) })
                }
            }
        )*
    };
}

into_future_ignore_body!(ExecuteSlackWebhook, ExecuteGitHubWebhook);

/// `GET /webhooks/{webhook.id}/{webhook.token}/messages/{message.id}`: a message that the
/// webhook sent.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetWebhookMessage {
    client: Client,
    webhook_id: WebhookId,
    token: PathToken,
    message_id: MessageId,
    thread_id: Option<ChannelId>,
}

impl GetWebhookMessage {
    /// Sets the thread that holds the message.
    pub fn thread_id(mut self, thread_id: ChannelId) -> Self {
        self.thread_id = Some(thread_id);
        self
    }
}

impl Endpoint for GetWebhookMessage {
    type Response = Message;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("thread_id", self.thread_id);
        Route::new(
            Method::Get,
            "/webhooks/{webhook_id}/{webhook_token}/messages/{message_id}",
            format!(
                "/webhooks/{}/{}/messages/{}{}",
                self.webhook_id,
                self.token.segment(),
                self.message_id,
                query.finish()
            ),
            MajorParameter::Webhook(self.webhook_id, self.token.hash()),
        )
        .with_path_token()
    }
}

/// `PATCH /webhooks/{webhook.id}/{webhook.token}/messages/{message.id}`: edits a message that
/// the webhook sent.
///
/// New files are added to the message. To remove files, list the attachments to keep with
/// [`attachments`](Self::attachments).
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditWebhookMessage {
    client: Client,
    webhook_id: WebhookId,
    token: PathToken,
    message_id: MessageId,
    payload: MessagePayload,
    files: Vec<FileUpload>,
    thread_id: Option<ChannelId>,
    with_components: Option<bool>,
}

impl EditWebhookMessage {
    message_setters!();

    /// Removes the content.
    pub fn clear_content(mut self) -> Self {
        self.payload.content = Some(String::new());
        self
    }

    /// Sets the thread that holds the message.
    pub fn thread_id(mut self, thread_id: ChannelId) -> Self {
        self.thread_id = Some(thread_id);
        self
    }

    /// Makes Discord use the components. A webhook that no application owns can then send
    /// components that are not interactive.
    pub fn with_components(mut self, with_components: bool) -> Self {
        self.with_components = Some(with_components);
        self
    }
}

impl Endpoint for EditWebhookMessage {
    type Response = Message;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("thread_id", self.thread_id);
        query.push_opt("with_components", self.with_components);
        Route::new(
            Method::Patch,
            "/webhooks/{webhook_id}/{webhook_token}/messages/{message_id}",
            format!(
                "/webhooks/{}/{}/messages/{}{}",
                self.webhook_id,
                self.token.segment(),
                self.message_id,
                query.finish()
            ),
            MajorParameter::Webhook(self.webhook_id, self.token.hash()),
        )
        .with_path_token()
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        edit_message_body(&self.payload, &self.files, validate)
    }
}

/// `DELETE /webhooks/{webhook.id}/{webhook.token}/messages/{message.id}`: deletes a message
/// that the webhook sent.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteWebhookMessage {
    client: Client,
    webhook_id: WebhookId,
    token: PathToken,
    message_id: MessageId,
    thread_id: Option<ChannelId>,
}

impl DeleteWebhookMessage {
    /// Sets the thread that holds the message.
    pub fn thread_id(mut self, thread_id: ChannelId) -> Self {
        self.thread_id = Some(thread_id);
        self
    }
}

impl Endpoint for DeleteWebhookMessage {
    type Response = ();

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("thread_id", self.thread_id);
        Route::new(
            Method::Delete,
            "/webhooks/{webhook_id}/{webhook_token}/messages/{message_id}",
            format!(
                "/webhooks/{}/{}/messages/{}{}",
                self.webhook_id,
                self.token.segment(),
                self.message_id,
                query.finish()
            ),
            MajorParameter::Webhook(self.webhook_id, self.token.hash()),
        )
        .with_path_token()
    }
}

super::into_future!(
    CreateWebhook,
    GetChannelWebhooks,
    GetGuildWebhooks,
    GetWebhook,
    GetWebhookWithToken,
    ModifyWebhook,
    ModifyWebhookWithToken,
    DeleteWebhook,
    DeleteWebhookWithToken,
    ExecuteWebhook,
    ExecuteWebhookAndWait,
    GetWebhookMessage,
    EditWebhookMessage,
    DeleteWebhookMessage,
);

impl Client {
    /// Makes a webhook in a channel. The name has 1 to 80 characters.
    pub fn create_webhook(&self, channel_id: ChannelId, name: impl Into<String>) -> CreateWebhook {
        CreateWebhook {
            client: self.clone(),
            channel_id,
            fields: CreateWebhookBody {
                name: name.into(),
                avatar: None,
            },
            reason: None,
        }
    }

    /// Gets the webhooks of a channel.
    pub fn channel_webhooks(&self, channel_id: ChannelId) -> GetChannelWebhooks {
        GetChannelWebhooks {
            client: self.clone(),
            channel_id,
        }
    }

    /// Gets the webhooks of a guild.
    pub fn guild_webhooks(&self, guild_id: GuildId) -> GetGuildWebhooks {
        GetGuildWebhooks {
            client: self.clone(),
            guild_id,
        }
    }

    /// Gets a webhook.
    pub fn webhook(&self, webhook_id: WebhookId) -> GetWebhook {
        GetWebhook {
            client: self.clone(),
            webhook_id,
        }
    }

    /// Gets a webhook with its token. The request does not use the client token.
    pub fn webhook_with_token(
        &self,
        webhook_id: WebhookId,
        token: impl Into<String>,
    ) -> GetWebhookWithToken {
        GetWebhookWithToken {
            client: self.clone(),
            webhook_id,
            token: PathToken::new(token),
        }
    }

    /// Changes a webhook.
    pub fn modify_webhook(&self, webhook_id: WebhookId) -> ModifyWebhook {
        ModifyWebhook {
            client: self.clone(),
            webhook_id,
            fields: ModifyWebhookBody::default(),
            reason: None,
        }
    }

    /// Changes a webhook with its token. The request does not use the client token.
    pub fn modify_webhook_with_token(
        &self,
        webhook_id: WebhookId,
        token: impl Into<String>,
    ) -> ModifyWebhookWithToken {
        ModifyWebhookWithToken {
            client: self.clone(),
            webhook_id,
            token: PathToken::new(token),
            fields: ModifyWebhookBody::default(),
            reason: None,
        }
    }

    /// Deletes a webhook.
    pub fn delete_webhook(&self, webhook_id: WebhookId) -> DeleteWebhook {
        DeleteWebhook {
            client: self.clone(),
            webhook_id,
            reason: None,
        }
    }

    /// Deletes a webhook with its token. The request does not use the client token.
    pub fn delete_webhook_with_token(
        &self,
        webhook_id: WebhookId,
        token: impl Into<String>,
    ) -> DeleteWebhookWithToken {
        DeleteWebhookWithToken {
            client: self.clone(),
            webhook_id,
            token: PathToken::new(token),
            reason: None,
        }
    }

    /// Sends a message with a webhook. Call [`ExecuteWebhook::wait`] to get the message.
    pub fn execute_webhook(
        &self,
        webhook_id: WebhookId,
        token: impl Into<String>,
    ) -> ExecuteWebhook {
        ExecuteWebhook {
            client: self.clone(),
            webhook_id,
            token: PathToken::new(token),
            payload: MessagePayload::default(),
            extra: ExecuteWebhookExtra::default(),
            files: Vec::new(),
            query: ExecuteWebhookQuery::default(),
        }
    }

    /// Sends a message with a webhook in the format of a Slack incoming webhook.
    pub fn execute_slack_webhook(
        &self,
        webhook_id: WebhookId,
        token: impl Into<String>,
        payload: serde_json::Value,
    ) -> ExecuteSlackWebhook {
        ExecuteSlackWebhook {
            client: self.clone(),
            webhook_id,
            token: PathToken::new(token),
            payload,
            thread_id: None,
            wait: None,
        }
    }

    /// Sends a message with a webhook for a GitHub event payload.
    pub fn execute_github_webhook(
        &self,
        webhook_id: WebhookId,
        token: impl Into<String>,
        payload: serde_json::Value,
    ) -> ExecuteGitHubWebhook {
        ExecuteGitHubWebhook {
            client: self.clone(),
            webhook_id,
            token: PathToken::new(token),
            payload,
            thread_id: None,
            wait: None,
        }
    }

    /// Gets a message that a webhook sent.
    pub fn webhook_message(
        &self,
        webhook_id: WebhookId,
        token: impl Into<String>,
        message_id: MessageId,
    ) -> GetWebhookMessage {
        GetWebhookMessage {
            client: self.clone(),
            webhook_id,
            token: PathToken::new(token),
            message_id,
            thread_id: None,
        }
    }

    /// Edits a message that a webhook sent.
    pub fn edit_webhook_message(
        &self,
        webhook_id: WebhookId,
        token: impl Into<String>,
        message_id: MessageId,
    ) -> EditWebhookMessage {
        EditWebhookMessage {
            client: self.clone(),
            webhook_id,
            token: PathToken::new(token),
            message_id,
            payload: MessagePayload::default(),
            files: Vec::new(),
            thread_id: None,
            with_components: None,
        }
    }

    /// Deletes a message that a webhook sent.
    pub fn delete_webhook_message(
        &self,
        webhook_id: WebhookId,
        token: impl Into<String>,
        message_id: MessageId,
    ) -> DeleteWebhookMessage {
        DeleteWebhookMessage {
            client: self.clone(),
            webhook_id,
            token: PathToken::new(token),
            message_id,
            thread_id: None,
        }
    }
}
