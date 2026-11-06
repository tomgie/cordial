//! The context of an interaction handler, with response state tracking.
//!
//! Discord needs one initial response within 3 seconds. After that, the bot edits the
//! original response or sends follow-up messages for 15 minutes. The context keeps the
//! response state, so [`InteractionContext::reply`] always picks the correct request.

use std::future::{Future, IntoFuture};
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use cordial_http::MessagePayload;
use cordial_model::Permissions;
use cordial_model::command::CommandOptionChoice;
use cordial_model::gateway::event::InteractionCreate;
use cordial_model::id::{ChannelId, GuildId, UserId};
use cordial_model::interaction::{
    Interaction, InteractionMessage, InteractionResponse, InteractionType, ModalResponse,
};
use cordial_model::locale::Locale;
use cordial_model::message::{Message, MessageFlags};
use cordial_model::user::User;
use tokio::sync::{Mutex, watch};

use crate::context::Context;

/// The response state of an interaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResponseState {
    /// No response yet.
    Pending,
    /// The bot deferred a message response. The user sees a loading state.
    Deferred {
        /// Tells if the coming message is ephemeral.
        ephemeral: bool,
    },
    /// The bot deferred an update of the component message.
    DeferredUpdate,
    /// The bot updated the component message. The original response is that message.
    Updated,
    /// The bot sent the initial response.
    Responded,
}

impl ResponseState {
    /// Tells if Discord has an initial response.
    #[must_use]
    pub fn is_acknowledged(self) -> bool {
        self != Self::Pending
    }
}

/// An error of a response.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ResponseError {
    /// The request failed.
    #[error(transparent)]
    Http(#[from] cordial_http::Error),
    /// The response is possible only as the initial response, and the interaction already
    /// has one. A modal and autocomplete choices are examples.
    #[error("the interaction already has an initial response")]
    AlreadyAcknowledged,
    /// The original response is not the component message, so an update cannot change it.
    /// This happens after a reply or a message defer.
    #[error("the original response is not the component message")]
    NotComponentMessage,
}

/// The shared response state of one interaction.
pub(crate) struct ResponseTracker {
    state: Mutex<ResponseState>,
    acknowledged: watch::Sender<bool>,
}

impl ResponseTracker {
    pub(crate) fn new() -> Arc<Self> {
        let (acknowledged, _) = watch::channel(false);
        Arc::new(Self {
            state: Mutex::new(ResponseState::Pending),
            acknowledged,
        })
    }

    /// Gives a receiver that changes to `true` at the initial response.
    pub(crate) fn subscribe(&self) -> watch::Receiver<bool> {
        self.acknowledged.subscribe()
    }

    pub(crate) async fn state(&self) -> ResponseState {
        *self.state.lock().await
    }
}

/// The context of an interaction handler.
///
/// It dereferences to [`Context`], so `ctx.http()` and `ctx.state()` work.
pub struct InteractionContext<S> {
    context: Context<S>,
    interaction: Arc<Interaction>,
    tracker: Arc<ResponseTracker>,
}

impl<S> Clone for InteractionContext<S> {
    fn clone(&self) -> Self {
        Self {
            context: self.context.clone(),
            interaction: Arc::clone(&self.interaction),
            tracker: Arc::clone(&self.tracker),
        }
    }
}

impl<S> std::fmt::Debug for InteractionContext<S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InteractionContext")
            .field("interaction_id", &self.interaction.id)
            .finish_non_exhaustive()
    }
}

impl<S> Deref for InteractionContext<S> {
    type Target = Context<S>;

    fn deref(&self) -> &Context<S> {
        &self.context
    }
}

/// The context of a command handler.
pub type CommandContext<S> = InteractionContext<S>;

/// The reason that a value of the interaction is absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MissingValue {
    /// The interaction is not in a guild.
    #[error("the interaction is not in a guild")]
    NotInGuild,
    /// The interaction has no channel.
    #[error("the interaction has no channel")]
    NoChannel,
}

impl<S: Send + Sync + 'static> InteractionContext<S> {
    pub(crate) fn new(
        context: Context<S>,
        interaction: Arc<Interaction>,
        tracker: Arc<ResponseTracker>,
    ) -> Self {
        Self {
            context,
            interaction,
            tracker,
        }
    }

    /// Gives the interaction.
    #[must_use]
    pub fn interaction(&self) -> &Interaction {
        &self.interaction
    }

    /// Gives the guild ID.
    ///
    /// # Errors
    ///
    /// Gives an error outside of guilds, so that `?` works in guild-only commands.
    pub fn guild_id(&self) -> Result<GuildId, MissingValue> {
        self.interaction.guild_id.ok_or(MissingValue::NotInGuild)
    }

    /// Gives the channel ID.
    ///
    /// # Errors
    ///
    /// Gives an error when the interaction has no channel.
    pub fn channel_id(&self) -> Result<ChannelId, MissingValue> {
        self.interaction.channel_id().ok_or(MissingValue::NoChannel)
    }

    /// Gives the user that used the interaction, in a guild or in a DM.
    ///
    /// # Panics
    ///
    /// Panics when Discord sends neither a member nor a user, which the documentation
    /// excludes.
    #[must_use]
    pub fn user(&self) -> &User {
        self.interaction
            .author()
            .expect("an interaction has a member or a user")
    }

    /// Gives the ID of the user that used the interaction.
    #[must_use]
    pub fn user_id(&self) -> UserId {
        self.user().id
    }

    /// Gives the permissions of the user in the channel, in a guild.
    #[must_use]
    pub fn member_permissions(&self) -> Option<Permissions> {
        self.interaction
            .member
            .as_ref()
            .and_then(|member| member.permissions)
    }

    /// Gives the locale of the user.
    #[must_use]
    pub fn locale(&self) -> Option<&Locale> {
        self.interaction.locale.as_ref()
    }

    /// Gives the response state.
    pub async fn response_state(&self) -> ResponseState {
        self.tracker.state().await
    }

    /// Sends a message. The request depends on the response state:
    ///
    /// | State | Request |
    /// |---|---|
    /// | Pending | Initial response with the message |
    /// | Deferred | Edit of the original response |
    /// | Deferred update, or responded | Follow-up message |
    ///
    /// After a defer, the defer decides if the message is ephemeral.
    pub fn reply(&self, message: impl Into<InteractionMessage>) -> Reply<'_, S> {
        Reply {
            context: self,
            message: message.into(),
        }
    }

    /// Defers a message response. The user sees "thinking" until the next reply.
    ///
    /// Does nothing when the interaction already has an initial response.
    ///
    /// # Errors
    ///
    /// Gives an error when the request fails.
    pub async fn defer(&self) -> Result<(), ResponseError> {
        self.defer_with(false).await
    }

    /// Defers an ephemeral message response.
    ///
    /// # Errors
    ///
    /// Gives an error when the request fails.
    pub async fn defer_ephemeral(&self) -> Result<(), ResponseError> {
        self.defer_with(true).await
    }

    async fn defer_with(&self, ephemeral: bool) -> Result<(), ResponseError> {
        let mut state = self.tracker.state.lock().await;
        if *state != ResponseState::Pending {
            return Ok(());
        }
        self.initial(InteractionResponse::defer(ephemeral)).await?;
        *state = ResponseState::Deferred { ephemeral };
        self.mark_acknowledged();
        Ok(())
    }

    /// For components and modals: acknowledges now, and edits the component message later.
    ///
    /// # Errors
    ///
    /// Gives an error when the request fails.
    pub async fn defer_update(&self) -> Result<(), ResponseError> {
        let mut state = self.tracker.state.lock().await;
        if *state != ResponseState::Pending {
            return Ok(());
        }
        self.initial(InteractionResponse::defer_update()).await?;
        *state = ResponseState::DeferredUpdate;
        self.mark_acknowledged();
        Ok(())
    }

    /// For components and modals: edits the message of the component.
    ///
    /// # Errors
    ///
    /// Gives an error when the request fails.
    pub async fn update_message(
        &self,
        message: impl Into<InteractionMessage>,
    ) -> Result<(), ResponseError> {
        let message = message.into();
        let mut state = self.tracker.state.lock().await;
        match *state {
            ResponseState::Pending => {
                self.initial(InteractionResponse::update(message)).await?;
                self.mark_acknowledged();
            }
            ResponseState::DeferredUpdate | ResponseState::Updated => {
                self.edit_original(MessagePayload::from(message)).await?;
            }
            _ => return Err(ResponseError::NotComponentMessage),
        }
        *state = ResponseState::Updated;
        Ok(())
    }

    /// Shows a modal. Only possible as the initial response, and not for a modal submit.
    ///
    /// # Errors
    ///
    /// Gives an error when the interaction already has a response, or the request fails.
    pub async fn modal(&self, modal: ModalResponse) -> Result<(), ResponseError> {
        let mut state = self.tracker.state.lock().await;
        if *state != ResponseState::Pending {
            return Err(ResponseError::AlreadyAcknowledged);
        }
        self.initial(InteractionResponse::modal(modal)).await?;
        *state = ResponseState::Responded;
        self.mark_acknowledged();
        Ok(())
    }

    /// Answers an autocomplete interaction with up to 25 choices.
    ///
    /// # Errors
    ///
    /// Gives an error when the interaction already has a response, or the request fails.
    pub async fn autocomplete(
        &self,
        choices: Vec<CommandOptionChoice>,
    ) -> Result<(), ResponseError> {
        let mut state = self.tracker.state.lock().await;
        if *state != ResponseState::Pending {
            return Err(ResponseError::AlreadyAcknowledged);
        }
        self.initial(InteractionResponse::autocomplete(choices))
            .await?;
        *state = ResponseState::Responded;
        self.mark_acknowledged();
        Ok(())
    }

    /// Edits the original response.
    ///
    /// # Errors
    ///
    /// Gives an error when the request fails.
    pub async fn edit_reply(
        &self,
        message: impl Into<InteractionMessage>,
    ) -> Result<Message, ResponseError> {
        let message = self
            .edit_original(MessagePayload::from(message.into()))
            .await?;
        let mut state = self.tracker.state.lock().await;
        if matches!(*state, ResponseState::Deferred { .. }) {
            *state = ResponseState::Responded;
        }
        Ok(message)
    }

    /// Deletes the original response.
    ///
    /// # Errors
    ///
    /// Gives an error when the request fails.
    pub async fn delete_reply(&self) -> Result<(), ResponseError> {
        self.http()
            .delete_original_interaction_response(self.interaction.application_id, self.token())
            .await?;
        Ok(())
    }

    /// Gets the original response.
    ///
    /// # Errors
    ///
    /// Gives an error when the request fails.
    pub async fn original_response(&self) -> Result<Message, ResponseError> {
        Ok(self
            .http()
            .original_interaction_response(self.interaction.application_id, self.token())
            .await?)
    }

    /// Sends a follow-up message. The interaction needs an initial response first.
    ///
    /// # Errors
    ///
    /// Gives an error when the request fails.
    pub async fn followup(
        &self,
        message: impl Into<InteractionMessage>,
    ) -> Result<Message, ResponseError> {
        Ok(self
            .http()
            .create_followup(self.interaction.application_id, self.token())
            .payload(MessagePayload::from(message.into()))
            .await?)
    }

    /// Waits for the next component interaction that matches the filter, for example a
    /// button press on the message of this reply. The router does not get it.
    ///
    /// Gives `None` at the timeout.
    pub async fn wait_for_component(
        &self,
        filter: impl FnMut(&Interaction) -> bool + Send + 'static,
        timeout: Duration,
    ) -> Option<crate::component::ComponentContext<S>> {
        self.wait_for_interaction(InteractionType::MessageComponent, filter, timeout)
            .await
    }

    /// Waits for the next modal submit that matches the filter. The router does not get it.
    ///
    /// Gives `None` at the timeout.
    pub async fn wait_for_modal(
        &self,
        filter: impl FnMut(&Interaction) -> bool + Send + 'static,
        timeout: Duration,
    ) -> Option<crate::component::ComponentContext<S>> {
        self.wait_for_interaction(InteractionType::ModalSubmit, filter, timeout)
            .await
    }

    async fn wait_for_interaction(
        &self,
        kind: InteractionType,
        mut filter: impl FnMut(&Interaction) -> bool + Send + 'static,
        timeout: Duration,
    ) -> Option<crate::component::ComponentContext<S>> {
        let created: InteractionCreate = self
            .context
            .wait_for(
                move |event: &InteractionCreate| event.kind == kind && filter(event),
                timeout,
            )
            .await?;
        let tracker = self.context.shared.take_tracker(created.id);
        let context = InteractionContext::new(self.context.clone(), Arc::new(created.0), tracker);
        Some(crate::component::ComponentContext::new(
            context,
            crate::component::Params::default(),
        ))
    }

    fn token(&self) -> String {
        self.interaction.token.expose().to_owned()
    }

    async fn initial(&self, response: InteractionResponse) -> Result<(), cordial_http::Error> {
        self.http()
            .create_interaction_response(self.interaction.id, self.token(), response)
            .await?;
        Ok(())
    }

    async fn edit_original(&self, payload: MessagePayload) -> Result<Message, cordial_http::Error> {
        self.http()
            .edit_original_interaction_response(self.interaction.application_id, self.token())
            .payload(payload)
            .await
    }

    fn mark_acknowledged(&self) {
        self.tracker.acknowledged.send_replace(true);
    }

    async fn send_reply(&self, message: InteractionMessage) -> Result<(), ResponseError> {
        let mut state = self.tracker.state.lock().await;
        match *state {
            ResponseState::Pending => {
                self.initial(InteractionResponse::message(message)).await?;
                self.mark_acknowledged();
            }
            ResponseState::Deferred { ephemeral } => {
                let wants_ephemeral = message
                    .flags
                    .is_some_and(|flags| flags.contains(MessageFlags::EPHEMERAL));
                if wants_ephemeral == ephemeral {
                    // The defer already set the ephemeral flag. An edit cannot change it.
                    let mut payload = MessagePayload::from(message);
                    payload.flags = payload.flags.map(|flags| flags - MessageFlags::EPHEMERAL);
                    self.edit_original(payload).await?;
                } else {
                    // The defer has the other visibility, for example an automatic public
                    // defer before an ephemeral reply. Replace the loading message, so that
                    // private text never becomes public.
                    self.http()
                        .delete_original_interaction_response(
                            self.interaction.application_id,
                            self.token(),
                        )
                        .await?;
                    self.http()
                        .create_followup(self.interaction.application_id, self.token())
                        .payload(MessagePayload::from(message))
                        .await?;
                }
            }
            ResponseState::DeferredUpdate | ResponseState::Updated | ResponseState::Responded => {
                self.http()
                    .create_followup(self.interaction.application_id, self.token())
                    .payload(MessagePayload::from(message))
                    .await?;
            }
        }
        *state = ResponseState::Responded;
        Ok(())
    }
}

/// A reply in preparation. Await it to send it.
#[must_use = "a reply does nothing until it is awaited"]
pub struct Reply<'a, S> {
    context: &'a InteractionContext<S>,
    message: InteractionMessage,
}

impl<S> std::fmt::Debug for Reply<'_, S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Reply")
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}

impl<S> Reply<'_, S> {
    /// Makes the message visible only to the user of the interaction.
    pub fn ephemeral(mut self) -> Self {
        self.message = self.message.ephemeral();
        self
    }

    /// Changes the message.
    pub fn message(mut self, edit: impl FnOnce(InteractionMessage) -> InteractionMessage) -> Self {
        self.message = edit(self.message);
        self
    }
}

impl<'a, S: Send + Sync + 'static> IntoFuture for Reply<'a, S> {
    type Output = Result<(), ResponseError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.context.send_reply(self.message))
    }
}

/// What the framework does when a handler is slow to respond.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SlowResponse {
    /// Log a warning after the delay.
    Warn(Duration),
    /// Defer the response after the delay. A command gets a message defer; a component or
    /// modal gets an update defer.
    Defer(Duration),
    /// Do nothing.
    Ignore,
}

impl Default for SlowResponse {
    fn default() -> Self {
        Self::Warn(Duration::from_millis(2500))
    }
}

/// Runs the slow-response timer for one interaction.
pub(crate) async fn watch_response<S: Send + Sync + 'static>(
    context: InteractionContext<S>,
    policy: SlowResponse,
) {
    let delay = match policy {
        SlowResponse::Warn(delay) | SlowResponse::Defer(delay) => delay,
        SlowResponse::Ignore => return,
    };
    let mut acknowledged = context.tracker.subscribe();
    let wait = acknowledged.wait_for(|acknowledged| *acknowledged);
    if tokio::time::timeout(delay, wait).await.is_ok() {
        return;
    }

    let interaction = context.interaction();
    match policy {
        SlowResponse::Defer(_) => {
            // An update defer needs a component message. A modal that a slash command opened
            // has none.
            let result = if interaction.command_data().is_none() && interaction.message.is_some() {
                context.defer_update().await
            } else {
                context.defer().await
            };
            if let Err(error) = result {
                tracing::warn!(interaction_id = %interaction.id, %error, "the automatic defer failed");
            }
        }
        _ => tracing::warn!(
            interaction_id = %interaction.id,
            delay_ms = delay.as_millis(),
            "the interaction has no response yet; Discord needs one within 3 seconds \
             (call defer(), or set SlowResponse::Defer)"
        ),
    }
}
