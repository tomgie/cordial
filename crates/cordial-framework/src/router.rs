//! Handler storage and dispatch: events, commands, autocomplete, components, and modals.

use std::collections::HashMap;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::Arc;

use cordial_model::command::{CommandDefinition, CommandOptionChoice, CommandType};
use cordial_model::gateway::event::{Event, EventType, EventTypes};
use cordial_model::id::GuildId;
use cordial_model::interaction::{Interaction, InteractionMessage, InteractionType};
use futures_util::FutureExt;

use crate::check::{Check, Cooldown};
use crate::component::{ComponentContext, Pattern};
use crate::context::Context;
use crate::error::{ErrorSource, HandlerError, HandlerErrorKind, HandlerResult};
use crate::interaction::{
    InteractionContext, ResponseState, ResponseTracker, SlowResponse, watch_response,
};

pub(crate) type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

type EventFn<S> =
    Arc<dyn Fn(Context<S>, Arc<Event>) -> Option<BoxFuture<HandlerResult>> + Send + Sync>;
type InteractionFn<C> = Arc<dyn Fn(C) -> BoxFuture<Result<(), HandlerErrorKind>> + Send + Sync>;
type AutocompleteFn<S> = Arc<
    dyn Fn(InteractionContext<S>, Focused) -> BoxFuture<Vec<CommandOptionChoice>> + Send + Sync,
>;
type ErrorHook<S> = Arc<dyn Fn(Context<S>, HandlerError) -> BoxFuture<()> + Send + Sync>;
type InteractionErrorHook<S> =
    Arc<dyn Fn(InteractionContext<S>, HandlerError) -> BoxFuture<()> + Send + Sync>;

/// The option that has focus in an autocomplete interaction.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Focused {
    /// The option name.
    pub name: String,
    /// The text that the user typed so far. Numbers are given as text.
    pub value: String,
    /// The names of the subcommand group and the subcommand, outermost first.
    pub path: Vec<String>,
}

pub(crate) struct EventHandler<S> {
    pub(crate) kind: Option<EventType>,
    pub(crate) run: EventFn<S>,
}

/// Where a command is registered.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommandScope {
    /// In all guilds and DMs.
    #[default]
    Global,
    /// Only in these guilds. Good for tests, because a guild sync shows the change at once.
    Guilds(Vec<GuildId>),
}

pub(crate) struct CommandEntry<S> {
    pub(crate) definition: CommandDefinition,
    pub(crate) previous_names: Vec<String>,
    pub(crate) scope: CommandScope,
    pub(crate) checks: Vec<Check<S>>,
    pub(crate) cooldown: Option<Cooldown>,
    pub(crate) run: InteractionFn<InteractionContext<S>>,
}

pub(crate) struct PatternEntry<S> {
    pub(crate) pattern: Pattern,
    pub(crate) run: InteractionFn<ComponentContext<S>>,
}

pub(crate) struct Router<S> {
    pub(crate) events: Vec<EventHandler<S>>,
    pub(crate) commands: HashMap<(CommandType, String), CommandEntry<S>>,
    pub(crate) autocomplete: HashMap<String, AutocompleteFn<S>>,
    pub(crate) components: Vec<PatternEntry<S>>,
    pub(crate) modals: Vec<PatternEntry<S>>,
    pub(crate) error_hook: Option<ErrorHook<S>>,
    pub(crate) interaction_error_hook: Option<InteractionErrorHook<S>>,
    pub(crate) slow_response: SlowResponse,
}

impl<S> Default for Router<S> {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            commands: HashMap::new(),
            autocomplete: HashMap::new(),
            components: Vec::new(),
            modals: Vec::new(),
            error_hook: None,
            interaction_error_hook: None,
            slow_response: SlowResponse::default(),
        }
    }
}

impl<S: Send + Sync + 'static> Router<S> {
    /// Tells if the router has no handler at all.
    pub(crate) fn is_empty(&self) -> bool {
        self.events.is_empty()
            && self.commands.is_empty()
            && self.autocomplete.is_empty()
            && self.components.is_empty()
            && self.modals.is_empty()
            && self.error_hook.is_none()
            && self.interaction_error_hook.is_none()
    }

    /// Gives the event types that the handlers need.
    pub(crate) fn event_types(&self) -> EventTypes {
        let mut types = EventTypes::empty();
        for handler in &self.events {
            match handler.kind {
                Some(kind) => types.insert(kind),
                None => types.extend_from(EventTypes::all()),
            }
        }
        if self.handles_interactions() {
            types.insert(EventType::InteractionCreate);
        }
        types
    }

    pub(crate) fn handles_interactions(&self) -> bool {
        !self.commands.is_empty() || !self.components.is_empty() || !self.modals.is_empty()
    }

    /// Gives the futures of the event handlers for one event.
    pub(crate) fn event_futures(
        &self,
        context: &Context<S>,
        event: &Arc<Event>,
    ) -> Vec<(ErrorSource, BoxFuture<Result<(), HandlerErrorKind>>)> {
        let kind = event.kind();
        self.events
            .iter()
            .filter(|handler| handler.kind.is_none() || handler.kind == kind)
            .filter_map(|handler| {
                let future = (handler.run)(context.clone(), Arc::clone(event))?;
                let source = ErrorSource::Event(event.name().to_owned());
                let future: BoxFuture<Result<(), HandlerErrorKind>> =
                    Box::pin(future.map(|result| result.map_err(HandlerErrorKind::Error)));
                Some((source, future))
            })
            .collect()
    }

    /// Runs a handler future. A panic or an error goes to the error hook.
    pub(crate) async fn guard(
        self: &Arc<Self>,
        context: &Context<S>,
        source: ErrorSource,
        future: BoxFuture<Result<(), HandlerErrorKind>>,
    ) -> bool {
        let kind = match AssertUnwindSafe(future).catch_unwind().await {
            Ok(Ok(())) => return true,
            Ok(Err(kind)) => kind,
            Err(panic) => HandlerErrorKind::Panic(panic_message(panic.as_ref())),
        };
        self.report(context, HandlerError { source, kind }).await;
        false
    }

    async fn report(&self, context: &Context<S>, error: HandlerError) {
        match &self.error_hook {
            Some(hook) => {
                let hook = hook(context.clone(), error);
                if let Err(panic) = AssertUnwindSafe(hook).catch_unwind().await {
                    tracing::error!(
                        panic = panic_message(panic.as_ref()),
                        "the error hook panicked"
                    );
                }
            }
            None => tracing::error!(%error, "a handler failed"),
        }
    }

    /// Gives an interaction to the collectors, or else to the handlers. Gives the response
    /// tracker at once, so that an HTTP source can wait for the initial response.
    pub(crate) fn route_interaction(
        self: &Arc<Self>,
        context: &Context<S>,
        event: &Arc<Event>,
        interaction: &Interaction,
    ) -> Arc<ResponseTracker> {
        let tracker = ResponseTracker::new();
        context
            .shared
            .keep_tracker(interaction.id, Arc::clone(&tracker));
        let taken = context.shared.collectors.dispatch(event);
        if taken {
            return tracker;
        }

        context.shared.take_tracker(interaction.id);
        if self.handles_interactions() {
            let interaction_context = InteractionContext::new(
                context.clone(),
                Arc::new(interaction.clone()),
                Arc::clone(&tracker),
            );
            let router = Arc::clone(self);
            let context = context.clone();
            tokio::spawn(async move { router.run_interaction(context, interaction_context).await });
        }
        tracker
    }

    async fn run_interaction(
        self: Arc<Self>,
        context: Context<S>,
        interaction: InteractionContext<S>,
    ) {
        let Some((source, future)) = self.interaction_future(&interaction) else {
            return;
        };
        // The timer starts only for an interaction with a handler. An unknown command fails
        // fast in the Discord client, and does not show "thinking" for 15 minutes.
        if interaction.interaction().kind != InteractionType::ApplicationCommandAutocomplete {
            tokio::spawn(watch_response(interaction.clone(), self.slow_response));
        }

        let kind = match AssertUnwindSafe(future).catch_unwind().await {
            Ok(Ok(())) => {
                if interaction.response_state().await == ResponseState::Pending {
                    tracing::warn!(
                        interaction_id = %interaction.interaction().id,
                        "the handler ended without a response to the interaction"
                    );
                }
                return;
            }
            Ok(Err(kind)) => kind,
            Err(panic) => HandlerErrorKind::Panic(panic_message(panic.as_ref())),
        };
        let error = HandlerError { source, kind };

        match &self.interaction_error_hook {
            Some(hook) => {
                let hook = hook(interaction, error);
                if let Err(panic) = AssertUnwindSafe(hook).catch_unwind().await {
                    tracing::error!(
                        panic = panic_message(panic.as_ref()),
                        "the interaction error hook panicked"
                    );
                }
            }
            None => {
                self.report(&context, error).await;
                default_failure_response(&interaction).await;
            }
        }
    }

    /// Finds the handler of an interaction and gives its future.
    fn interaction_future(
        &self,
        context: &InteractionContext<S>,
    ) -> Option<(ErrorSource, BoxFuture<Result<(), HandlerErrorKind>>)> {
        let interaction = context.interaction();
        match interaction.kind {
            InteractionType::ApplicationCommand => self.command_future(context),
            InteractionType::ApplicationCommandAutocomplete => self.autocomplete_future(context),
            InteractionType::MessageComponent => {
                let custom_id = &interaction.component_data()?.custom_id;
                self.pattern_future(&self.components, context, custom_id, ErrorSource::Component)
            }
            InteractionType::ModalSubmit => {
                let custom_id = &interaction.modal_data()?.custom_id;
                self.pattern_future(&self.modals, context, custom_id, ErrorSource::Modal)
            }
            _ => None,
        }
    }

    /// Gives the future of a command: the checks, the cooldown, and the handler. The checks
    /// run in the future, so that a panic in a check goes to the error hook.
    fn command_future(
        &self,
        context: &InteractionContext<S>,
    ) -> Option<(ErrorSource, BoxFuture<Result<(), HandlerErrorKind>>)> {
        let data = context.interaction().command_data()?;
        let Some(entry) = self.commands.get(&(data.kind, data.name.clone())) else {
            tracing::warn!(
                command = %data.name,
                "no handler for this command; sync the commands to remove old ones"
            );
            return None;
        };
        let source = ErrorSource::Command(data.name.clone());

        let checks = entry.checks.clone();
        let cooldown = entry.cooldown.clone();
        let run = Arc::clone(&entry.run);
        let context = context.clone();
        let future = async move {
            for check in &checks {
                if let Err(failure) = check.run(context.clone()).await {
                    refuse(&context, failure.message).await;
                    return Ok(());
                }
            }
            if let Some(cooldown) = &cooldown
                && let Err(wait) = cooldown.try_use(&context)
            {
                let message = format!("Try again in {} seconds.", wait.as_secs().max(1));
                refuse(&context, message).await;
                return Ok(());
            }
            run(context).await
        };
        Some((source, Box::pin(future)))
    }

    fn autocomplete_future(
        &self,
        context: &InteractionContext<S>,
    ) -> Option<(ErrorSource, BoxFuture<Result<(), HandlerErrorKind>>)> {
        let data = context.interaction().command_data()?;
        let handler = self.autocomplete.get(&data.name)?;
        let (_, path) = data.leaf_options();
        let option = data.focused_option()?;
        let value = option.value.as_ref().map_or_else(String::new, |value| {
            value.as_str().map_or_else(
                || {
                    value
                        .as_f64()
                        .map_or_else(String::new, |number| number.to_string())
                },
                str::to_owned,
            )
        });
        let focused = Focused {
            name: option.name.clone(),
            value,
            path: path.into_iter().map(str::to_owned).collect(),
        };

        let choices = handler(context.clone(), focused);
        let context = context.clone();
        let future = async move {
            let mut choices = choices.await;
            choices.truncate(25);
            context
                .autocomplete(choices)
                .await
                .map_err(|error| HandlerErrorKind::Error(error.into()))
        };
        Some((
            ErrorSource::Autocomplete(data.name.clone()),
            Box::pin(future),
        ))
    }

    fn pattern_future(
        &self,
        entries: &[PatternEntry<S>],
        context: &InteractionContext<S>,
        custom_id: &str,
        source: fn(String) -> ErrorSource,
    ) -> Option<(ErrorSource, BoxFuture<Result<(), HandlerErrorKind>>)> {
        let found = entries
            .iter()
            .find_map(|entry| Some((entry, entry.pattern.matches(custom_id)?)));
        let Some((entry, params)) = found else {
            tracing::debug!(custom_id, "no handler matches this custom ID");
            return None;
        };
        let component_context = ComponentContext::new(context.clone(), params);
        Some((source(custom_id.to_owned()), (entry.run)(component_context)))
    }
}

async fn refuse<S: Send + Sync + 'static>(context: &InteractionContext<S>, message: String) {
    if let Err(error) = context.reply(message).ephemeral().await {
        tracing::warn!(%error, "the reply of a failed check failed");
    }
}

/// Tells the user that the interaction failed. Without a response, the user sees "The
/// application did not respond", or "thinking" for 15 minutes after a defer.
async fn default_failure_response<S: Send + Sync + 'static>(context: &InteractionContext<S>) {
    const TEXT: &str = "An error occurred.";

    let result = match context.response_state().await {
        ResponseState::Pending
            if context.interaction().kind == InteractionType::ApplicationCommandAutocomplete =>
        {
            context.autocomplete(Vec::new()).await
        }
        ResponseState::Pending => context.reply(TEXT).ephemeral().await,
        ResponseState::Deferred { .. } => context.edit_reply(TEXT).await.map(drop),
        ResponseState::DeferredUpdate => context
            .followup(InteractionMessage::text(TEXT).ephemeral())
            .await
            .map(drop),
        _ => Ok(()),
    };
    if let Err(error) = result {
        tracing::warn!(%error, "the error response failed");
    }
}

fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic with no message".to_owned())
}
