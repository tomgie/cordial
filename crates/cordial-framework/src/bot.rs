//! The bot builder and the main loop.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use cordial_cache::{Cache, ResourceTypes};
use cordial_gateway::{
    FatalError, Session, ShardEvent, ShardManager, ShardManagerBuilder, ShardMessage, ShardRange,
};
use cordial_http::{Client, Token};
use cordial_model::application::ApplicationFlags;
use cordial_model::command::CommandOptionChoice;
use cordial_model::gateway::Intents;
use cordial_model::gateway::command::UpdatePresence;
use cordial_model::gateway::event::{
    Event, EventType, EventTypes, GatewayEvent, InteractionCreate,
};
use cordial_model::id::GuildId;
use tokio::sync::mpsc;

use crate::check::{Check, Cooldown};
use crate::command::Command;
use crate::component::{ComponentContext, Pattern};
use crate::context::Context;
use crate::error::{Error, HandlerError, HandlerErrorKind, IntoHandlerResult};
use crate::intents::{self, IntentCheck, IntentInput};
use crate::interaction::{CommandContext, InteractionContext, SlowResponse};
use crate::localize::{self, Localizer};
use crate::modal::Modal;
use crate::router::{
    BoxFuture, CommandEntry, CommandScope, EventHandler, Focused, PatternEntry, Router,
};
use crate::sync::{SyncCommand, SyncOptions, SyncScope};

/// The settings hook of [`BotBuilder::gateway`].
type GatewaySettings = Arc<dyn Fn(ShardManagerBuilder) -> ShardManagerBuilder + Send + Sync>;

/// An autocomplete handler: an async function that gives the choices.
pub trait AutocompleteHandler<S>: Send + Sync + 'static {
    /// Runs the handler.
    fn call(
        &self,
        context: CommandContext<S>,
        focused: Focused,
    ) -> BoxFuture<Vec<CommandOptionChoice>>;
}

impl<S, F, Fut> AutocompleteHandler<S> for F
where
    F: Fn(CommandContext<S>, Focused) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Vec<CommandOptionChoice>> + Send + 'static,
{
    fn call(
        &self,
        context: CommandContext<S>,
        focused: Focused,
    ) -> BoxFuture<Vec<CommandOptionChoice>> {
        Box::pin(self(context, focused))
    }
}

/// How the bot orders the event handlers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum EventOrder {
    /// Each event runs in a new task. Handlers of different events run at the same time.
    #[default]
    Concurrent,
    /// The events of each shard run one after the other, in the order of arrival. All events
    /// of one guild come from one shard, so the order of each guild is kept.
    ///
    /// Interactions always run at the same time, because Discord needs a response within
    /// 3 seconds.
    PerShard,
}

/// What the bot does with its sessions when it stops.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ShutdownMode {
    /// End the sessions. The bot goes offline.
    #[default]
    End,
    /// Keep the sessions for a short time. Give [`RunOutcome::sessions`] to the next process
    /// with [`BotBuilder::resume_sessions`], so that it resumes without missed events.
    Resumable,
}

/// When the bot syncs its commands with Discord.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub enum CommandSync {
    /// Do not sync. Use this when another tool registers the commands.
    Off,
    /// Sync at each start, with these options.
    OnStart(SyncOptions),
    /// Sync at each start with the default options (granular).
    #[default]
    Default,
}

/// The result of [`Bot::run`].
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct RunOutcome {
    /// The sessions to resume, with [`ShutdownMode::Resumable`]. Empty otherwise.
    pub sessions: HashMap<u32, Session>,
}

/// Settings of one command.
pub struct CommandSettings<S> {
    checks: Vec<Check<S>>,
    cooldown: Option<Cooldown>,
    scope: CommandScope,
    previous_names: Vec<String>,
}

impl<S> std::fmt::Debug for CommandSettings<S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommandSettings")
            .field("scope", &self.scope)
            .field("previous_names", &self.previous_names)
            .finish_non_exhaustive()
    }
}

impl<S> Default for CommandSettings<S> {
    fn default() -> Self {
        Self {
            checks: Vec::new(),
            cooldown: None,
            scope: CommandScope::Global,
            previous_names: Vec::new(),
        }
    }
}

impl<S> CommandSettings<S> {
    /// Adds a check. Checks run in order before the handler.
    #[must_use]
    pub fn check(mut self, check: Check<S>) -> Self {
        self.checks.push(check);
        self
    }

    /// Sets a cooldown.
    #[must_use]
    pub fn cooldown(mut self, cooldown: Cooldown) -> Self {
        self.cooldown = Some(cooldown);
        self
    }

    /// Registers the command only in some guilds, in place of globally.
    #[must_use]
    pub fn guilds(mut self, guilds: impl IntoIterator<Item = cordial_model::id::GuildId>) -> Self {
        self.scope = CommandScope::Guilds(guilds.into_iter().collect());
        self
    }

    /// Gives an old name of the command. The sync renames the command and keeps its ID and
    /// permissions, in place of a delete and a create.
    #[must_use]
    pub fn previous_name(mut self, name: impl Into<String>) -> Self {
        self.previous_names.push(name.into());
        self
    }
}

/// Builds a [`Bot`]. [`Bot::builder`] makes one.
///
/// Call [`BotBuilder::state`] first, because the handlers use the state type. Then add the
/// handlers and the settings, and call [`BotBuilder::build`].
///
/// ```no_run
/// use std::sync::atomic::{AtomicU64, Ordering};
///
/// use cordial_cache::ResourceTypes;
/// use cordial_framework::{Bot, Context, EventOrder};
/// use cordial_http::Token;
/// use cordial_model::gateway::event::MessageCreate;
///
/// #[derive(Default)]
/// struct State {
///     messages: AtomicU64,
/// }
///
/// async fn count(ctx: Context<State>, _: MessageCreate) {
///     ctx.state().messages.fetch_add(1, Ordering::Relaxed);
/// }
///
/// # fn build() -> Result<(), Box<dyn std::error::Error>> {
/// let bot = Bot::builder(Token::from_env("DISCORD_TOKEN")?)
///     .state(State::default())
///     .cache(ResourceTypes::GUILD | ResourceTypes::CHANNEL)
///     .event_order(EventOrder::PerShard)
///     .on(count)
///     .build()?;
/// # Ok(())
/// # }
/// ```
pub struct BotBuilder<S> {
    token: Token,
    http: Option<Client>,
    state: S,
    router: Router<S>,
    intents: Option<Intents>,
    intent_check: IntentCheck,
    cache: Option<(ResourceTypes, usize)>,
    localizer: Option<Arc<dyn Localizer>>,
    command_sync: CommandSync,
    event_order: EventOrder,
    shutdown_mode: ShutdownMode,
    ctrl_c: bool,
    voice: bool,
    gateway: Option<GatewaySettings>,
    shards: ShardRange,
    presence: Option<UpdatePresence>,
    sessions: HashMap<u32, Session>,
    extra_events: EventTypes,
    sync_guilds: Vec<GuildId>,
    problems: Vec<String>,
}

impl<S> std::fmt::Debug for BotBuilder<S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BotBuilder")
            .field("intents", &self.intents)
            .field("event_order", &self.event_order)
            .finish_non_exhaustive()
    }
}

/// A bot: the gateway connection, the cache, and the handlers.
///
/// Make it with [`Bot::builder`]. [`Bot::run`] connects to the gateway and runs until the
/// bot stops. With [`ShutdownMode::Resumable`], the outcome has the sessions for the next
/// process.
///
/// ```no_run
/// use cordial_framework::{Bot, Context, ShutdownMode};
/// use cordial_http::Token;
/// use cordial_model::gateway::event::Ready;
///
/// async fn ready(_: Context<()>, ready: Ready) {
///     tracing::info!(user = %ready.user.username, "the bot is ready");
/// }
///
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let bot = Bot::builder(Token::from_env("DISCORD_TOKEN")?)
///     .on(ready)
///     .shutdown_mode(ShutdownMode::Resumable)
///     .build()?;
/// let outcome = bot.run().await?;
/// tracing::info!(sessions = outcome.sessions.len(), "the bot stopped");
/// # Ok(())
/// # }
/// ```
pub struct Bot<S> {
    pub(crate) context: Context<S>,
    router: Arc<Router<S>>,
    intents: Option<Intents>,
    intent_check: IntentCheck,
    resources: Option<ResourceTypes>,
    localizer: Option<Arc<dyn Localizer>>,
    command_sync: CommandSync,
    event_order: EventOrder,
    shutdown_mode: ShutdownMode,
    ctrl_c: bool,
    voice: bool,
    gateway: Option<GatewaySettings>,
    shards: ShardRange,
    presence: Option<UpdatePresence>,
    sessions: HashMap<u32, Session>,
    extra_events: EventTypes,
    sync_guilds: Vec<GuildId>,
}

impl<S> std::fmt::Debug for Bot<S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Bot")
            .field("intents", &self.intents)
            .finish_non_exhaustive()
    }
}

impl Bot<()> {
    /// Starts a builder with no user state. Call [`BotBuilder::state`] first to add state.
    #[must_use]
    pub fn builder(token: Token) -> BotBuilder<()> {
        BotBuilder {
            token,
            http: None,
            state: (),
            router: Router::default(),
            intents: None,
            intent_check: IntentCheck::default(),
            cache: None,
            localizer: None,
            command_sync: CommandSync::default(),
            event_order: EventOrder::default(),
            shutdown_mode: ShutdownMode::default(),
            ctrl_c: true,
            voice: false,
            gateway: None,
            shards: ShardRange::Recommended,
            presence: None,
            sessions: HashMap::new(),
            extra_events: EventTypes::empty(),
            sync_guilds: Vec::new(),
            problems: Vec::new(),
        }
    }
}

impl<S: Send + Sync + 'static> BotBuilder<S> {
    /// Sets the user state. Handlers get it with `ctx.state()`.
    ///
    /// Call it before you add handlers, because the handlers use the state type. The
    /// handlers that you added before are removed, and `build` gives an error.
    pub fn state<T: Send + Sync + 'static>(self, state: T) -> BotBuilder<T> {
        let mut problems = self.problems;
        if !self.router.is_empty() {
            problems.push("call `state` before you add handlers".to_owned());
        }
        BotBuilder {
            token: self.token,
            http: self.http,
            state,
            router: Router {
                slow_response: self.router.slow_response,
                ..Router::default()
            },
            intents: self.intents,
            intent_check: self.intent_check,
            cache: self.cache,
            localizer: self.localizer,
            command_sync: self.command_sync,
            event_order: self.event_order,
            shutdown_mode: self.shutdown_mode,
            ctrl_c: self.ctrl_c,
            voice: self.voice,
            gateway: self.gateway,
            shards: self.shards,
            presence: self.presence,
            sessions: self.sessions,
            extra_events: self.extra_events,
            sync_guilds: self.sync_guilds,
            problems,
        }
    }

    /// Uses an HTTP client with custom settings. Its token must be the bot token. The bot then
    /// uses the token of the client, not the token of [`Bot::builder`].
    #[must_use]
    pub fn http(mut self, client: Client) -> Self {
        self.http = Some(client);
        self
    }

    /// Sets the intents. Without this call, the bot computes them from the handlers and the
    /// cache. The intent check runs in both cases.
    #[must_use]
    pub fn intents(mut self, intents: Intents) -> Self {
        self.intents = Some(intents);
        self
    }

    /// Sets how strict the intent check is.
    #[must_use]
    pub fn intent_check(mut self, check: IntentCheck) -> Self {
        self.intent_check = check;
        self
    }

    /// Turns on the cache for some resource types.
    #[must_use]
    pub fn cache(mut self, resources: ResourceTypes) -> Self {
        let capacity = self.cache.map_or(100, |(_, capacity)| capacity);
        self.cache = Some((resources, capacity));
        self
    }

    /// Sets the number of messages that the cache keeps for each channel.
    #[must_use]
    pub fn cache_messages(mut self, capacity: usize) -> Self {
        let resources = self.cache.map_or(ResourceTypes::MESSAGE, |(resources, _)| {
            resources | ResourceTypes::MESSAGE
        });
        self.cache = Some((resources, capacity));
        self
    }

    /// Sets the localizer for command names, descriptions, and choices.
    #[must_use]
    pub fn localizer(mut self, localizer: impl Localizer) -> Self {
        self.localizer = Some(Arc::new(localizer));
        self
    }

    /// Sets when the bot syncs its commands.
    #[must_use]
    pub fn command_sync(mut self, sync: CommandSync) -> Self {
        self.command_sync = sync;
        self
    }

    /// Sets the order of the event handlers.
    #[must_use]
    pub fn event_order(mut self, order: EventOrder) -> Self {
        self.event_order = order;
        self
    }

    /// Sets what the bot does when a handler is slow to respond to an interaction.
    #[must_use]
    pub fn slow_response(mut self, policy: SlowResponse) -> Self {
        self.router.slow_response = policy;
        self
    }

    /// Sets what the bot does with its sessions when it stops.
    #[must_use]
    pub fn shutdown_mode(mut self, mode: ShutdownMode) -> Self {
        self.shutdown_mode = mode;
        self
    }

    /// Sets if Ctrl+C stops the bot. The default is `true`.
    #[must_use]
    pub fn stop_on_ctrl_c(mut self, enabled: bool) -> Self {
        self.ctrl_c = enabled;
        self
    }

    /// Turns on the voice manager. The bot makes it when the shards start, with DAVE
    /// end-to-end encryption, and gives it the voice events. Get it with
    /// [`Context::voice`].
    #[cfg(feature = "voice")]
    #[must_use]
    pub fn voice(mut self) -> Self {
        self.voice = true;
        self
    }

    /// Changes the shard manager settings before the shards start, for example the
    /// compression, a shared identify queue for many processes, or the large threshold.
    ///
    /// ```no_run
    /// # use cordial_framework::Bot;
    /// # use cordial_gateway::Compression;
    /// # let token = cordial_http::Token::bot("abc");
    /// let bot = Bot::builder(token)
    ///     .gateway(|builder| builder.compression(Compression::Zlib).large_threshold(100))
    ///     .build();
    /// ```
    #[must_use]
    pub fn gateway(
        mut self,
        configure: impl Fn(ShardManagerBuilder) -> ShardManagerBuilder + Send + Sync + 'static,
    ) -> Self {
        self.gateway = Some(Arc::new(configure));
        self
    }

    /// Sets the shards of this process.
    #[must_use]
    pub fn shards(mut self, shards: ShardRange) -> Self {
        self.shards = shards;
        self
    }

    /// Sets the presence at identify.
    #[must_use]
    pub fn presence(mut self, presence: UpdatePresence) -> Self {
        self.presence = Some(presence);
        self
    }

    /// Gives sessions to resume, from [`RunOutcome::sessions`] of a previous process.
    #[must_use]
    pub fn resume_sessions(mut self, sessions: HashMap<u32, Session>) -> Self {
        self.sessions = sessions;
        self
    }

    /// Adds a handler for one event type.
    #[must_use]
    pub fn on<E, F, Fut, R>(mut self, handler: F) -> Self
    where
        E: GatewayEvent + Clone + Send + 'static,
        F: Fn(Context<S>, E) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoHandlerResult,
    {
        let handler = Arc::new(handler);
        self.router.events.push(EventHandler {
            kind: Some(E::TYPE),
            run: Arc::new(move |context, event: Arc<Event>| {
                let payload = E::from_event_ref(&event)?.clone();
                let handler = Arc::clone(&handler);
                let future: BoxFuture<_> =
                    Box::pin(async move { handler(context, payload).await.into_handler_result() });
                Some(future)
            }),
        });
        self
    }

    /// Adds a handler for all events. It gets each event before the typed handlers.
    #[must_use]
    pub fn on_any<F, Fut, R>(mut self, handler: F) -> Self
    where
        F: Fn(Context<S>, Arc<Event>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoHandlerResult,
    {
        let handler = Arc::new(handler);
        self.router.events.insert(
            0,
            EventHandler {
                kind: None,
                run: Arc::new(move |context, event| {
                    let handler = Arc::clone(&handler);
                    let future: BoxFuture<_> =
                        Box::pin(
                            async move { handler(context, event).await.into_handler_result() },
                        );
                    Some(future)
                }),
            },
        );
        self
    }

    /// Sets the error hook. It gets the errors and panics of all handlers.
    ///
    /// Without a hook, the bot logs the errors and answers a failed interaction with an
    /// ephemeral "An error occurred." message.
    #[must_use]
    pub fn on_error<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn(Context<S>, HandlerError) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.router.error_hook = Some(Arc::new(move |context, error| {
            Box::pin(hook(context, error))
        }));
        self
    }

    /// Sets the hook for failed interaction handlers. It gets the interaction context, so it
    /// can answer the user, for example with an ephemeral error message.
    ///
    /// With this hook, the bot does not send its default "An error occurred." response, and
    /// the failure does not go to [`BotBuilder::on_error`].
    #[must_use]
    pub fn on_interaction_error<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn(InteractionContext<S>, HandlerError) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.router.interaction_error_hook = Some(Arc::new(move |context, error| {
            Box::pin(hook(context, error))
        }));
        self
    }

    /// Requests more event types from the gateway, for example for a collector
    /// (`wait_for::<MessageReactionAdd>`) when no handler uses the type.
    #[must_use]
    pub fn extra_events(mut self, events: impl IntoIterator<Item = EventType>) -> Self {
        for kind in events {
            self.extra_events.insert(kind);
        }
        self
    }

    /// Syncs these guild scopes at each start, also when no command is registered there now.
    /// The sync then removes old guild commands, for example after a command moves from a
    /// test guild to global.
    #[must_use]
    pub fn sync_guilds(mut self, guilds: impl IntoIterator<Item = GuildId>) -> Self {
        self.sync_guilds.extend(guilds);
        self
    }

    /// Adds a command with its handler.
    #[must_use]
    pub fn command<C, F, Fut, R>(self, handler: F) -> Self
    where
        C: Command,
        F: Fn(CommandContext<S>, C) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoHandlerResult,
    {
        self.command_with::<C, F, Fut, R>(handler, |settings| settings)
    }

    /// Adds a command with its handler and settings: checks, a cooldown, and the scope.
    #[must_use]
    pub fn command_with<C, F, Fut, R>(
        mut self,
        handler: F,
        settings: impl FnOnce(CommandSettings<S>) -> CommandSettings<S>,
    ) -> Self
    where
        C: Command,
        F: Fn(CommandContext<S>, C) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoHandlerResult,
    {
        let settings = settings(CommandSettings::default());
        let key = (C::KIND, C::NAME.to_owned());
        if self.router.commands.contains_key(&key) {
            self.problems
                .push(format!("the command `{}` is registered two times", C::NAME));
        }

        let handler = Arc::new(handler);
        let entry = CommandEntry {
            definition: C::definition(),
            previous_names: settings.previous_names,
            scope: settings.scope,
            checks: settings.checks,
            cooldown: settings.cooldown,
            run: Arc::new(move |context: CommandContext<S>| {
                let handler = Arc::clone(&handler);
                Box::pin(async move {
                    let data = context.interaction().command_data().ok_or_else(|| {
                        HandlerErrorKind::Error("the interaction has no command data".into())
                    })?;
                    let arguments = C::parse(data).map_err(HandlerErrorKind::Parse)?;
                    handler(context, arguments)
                        .await
                        .into_handler_result()
                        .map_err(HandlerErrorKind::Error)
                })
            }),
        };
        self.router.commands.insert(key, entry);
        self
    }

    /// Adds the autocomplete handler of a command. It gives the choices (up to 25).
    #[must_use]
    ///
    /// Write `.autocomplete::<MyCommand>(handler)`. The handler is an async function
    /// `(CommandContext<S>, Focused) -> Vec<CommandOptionChoice>`.
    pub fn autocomplete<C: Command>(mut self, handler: impl AutocompleteHandler<S>) -> Self {
        let handler = Arc::new(handler);
        self.router.autocomplete.insert(
            C::NAME.to_owned(),
            Arc::new(move |context, focused| handler.call(context, focused)),
        );
        self
    }

    /// Adds a component handler for a `custom_id` pattern, for example `"vote:{poll}:{choice}"`.
    /// The first pattern that matches wins.
    #[must_use]
    pub fn component<F, Fut, R>(mut self, pattern: &str, handler: F) -> Self
    where
        F: Fn(ComponentContext<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoHandlerResult,
    {
        let Some(pattern) = self.pattern(pattern) else {
            return self;
        };
        let handler = Arc::new(handler);
        self.router.components.push(PatternEntry {
            pattern,
            run: Arc::new(move |context| {
                let handler = Arc::clone(&handler);
                Box::pin(async move {
                    handler(context)
                        .await
                        .into_handler_result()
                        .map_err(HandlerErrorKind::Error)
                })
            }),
        });
        self
    }

    /// Adds a modal handler for a `custom_id` pattern. The handler gets the parsed modal.
    #[must_use]
    pub fn modal<M, F, Fut, R>(mut self, pattern: &str, handler: F) -> Self
    where
        M: Modal + Send + 'static,
        F: Fn(ComponentContext<S>, M) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoHandlerResult,
    {
        let Some(pattern) = self.pattern(pattern) else {
            return self;
        };
        let handler = Arc::new(handler);
        self.router.modals.push(PatternEntry {
            pattern,
            run: Arc::new(move |context: ComponentContext<S>| {
                let handler = Arc::clone(&handler);
                Box::pin(async move {
                    let data = context.interaction().modal_data().ok_or_else(|| {
                        HandlerErrorKind::Error("the interaction has no modal data".into())
                    })?;
                    let modal = M::parse(data).map_err(HandlerErrorKind::Parse)?;
                    handler(context, modal)
                        .await
                        .into_handler_result()
                        .map_err(HandlerErrorKind::Error)
                })
            }),
        });
        self
    }

    fn pattern(&mut self, source: &str) -> Option<Pattern> {
        match Pattern::parse(source) {
            Ok(pattern) => Some(pattern),
            Err(error) => {
                self.problems.push(error.to_string());
                None
            }
        }
    }

    /// Builds the bot.
    ///
    /// # Errors
    ///
    /// Gives an error when the settings have a problem, for example a pattern that is not
    /// valid or a command that is registered two times.
    pub fn build(self) -> Result<Bot<S>, Error> {
        if !self.problems.is_empty() {
            return Err(Error::Config(self.problems.join("; ")));
        }

        let http = self.http.unwrap_or_else(|| Client::new(self.token));
        let cache = self
            .cache
            .map(|(resources, capacity)| Cache::with_message_capacity(resources, capacity));
        let resources = self.cache.map(|(resources, _)| resources);

        Ok(Bot {
            context: Context::new(http, cache, self.state),
            router: Arc::new(self.router),
            intents: self.intents,
            intent_check: self.intent_check,
            resources,
            localizer: self.localizer,
            command_sync: self.command_sync,
            event_order: self.event_order,
            shutdown_mode: self.shutdown_mode,
            ctrl_c: self.ctrl_c,
            voice: self.voice,
            gateway: self.gateway,
            shards: self.shards,
            presence: self.presence,
            sessions: self.sessions,
            extra_events: self.extra_events,
            sync_guilds: self.sync_guilds,
        })
    }
}

impl<S: Send + Sync + 'static> Bot<S> {
    /// Gives a context, for example to start tasks before [`Bot::run`].
    #[must_use]
    pub fn context(&self) -> Context<S> {
        self.context.clone()
    }

    /// Gives the command definitions with the translations, grouped by scope.
    #[must_use]
    pub fn command_definitions(&self) -> HashMap<SyncScope, Vec<SyncCommand>> {
        let mut scopes: HashMap<SyncScope, Vec<SyncCommand>> = HashMap::new();
        for entry in self.router.commands.values() {
            let mut definition = entry.definition.clone();
            if let Some(localizer) = &self.localizer {
                localize::apply(&mut definition, localizer.as_ref());
            }
            let mut command = SyncCommand::new(definition);
            for name in &entry.previous_names {
                command = command.previous_name(name.clone());
            }

            let targets = match &entry.scope {
                CommandScope::Global => vec![SyncScope::Global],
                CommandScope::Guilds(guilds) => {
                    guilds.iter().copied().map(SyncScope::Guild).collect()
                }
            };
            for scope in targets {
                scopes.entry(scope).or_default().push(command.clone());
            }
        }
        scopes
    }

    /// Gets the application, and syncs the commands when the settings say so.
    ///
    /// [`Bot::run`] calls it. Call it yourself when the bot gets interactions only over HTTP
    /// (the `endpoint` feature) and does not call `run`.
    ///
    /// # Errors
    ///
    /// Gives an error when a request fails.
    pub async fn prepare(&self) -> Result<ApplicationFlags, Error> {
        let application = self.context.http().current_application().await?;
        let _ = self.context.shared.application_id.set(application.id);

        let options = match &self.command_sync {
            CommandSync::Off => None,
            CommandSync::Default => Some(SyncOptions::default()),
            CommandSync::OnStart(options) => Some(options.clone()),
        };
        if let Some(options) = options {
            self.sync_commands(&options).await?;
        }
        Ok(application.flags())
    }

    async fn sync_commands(&self, options: &SyncOptions) -> Result<(), Error> {
        let mut scopes = self.command_definitions();
        // Remove the old global commands also when the bot has none now.
        scopes.entry(SyncScope::Global).or_default();
        for guild_id in &self.sync_guilds {
            scopes.entry(SyncScope::Guild(*guild_id)).or_default();
        }

        for (scope, commands) in scopes {
            let report = crate::sync::sync(
                self.context.http(),
                self.context.application_id(),
                scope,
                &commands,
                options,
            )
            .await
            .map_err(|error| Error::Sync(Box::new(error)))?;
            tracing::info!(%scope, plan = %report.plan, "synced the commands");
        }
        Ok(())
    }

    /// Gives the event types that the handlers and the cache need.
    fn event_types(&self) -> EventTypes {
        let mut types = self.router.event_types();
        types.extend_from(self.extra_events);
        if let Some(resources) = self.resources {
            types.extend_from(resources.event_types());
        }
        types.insert(EventType::Ready);
        if self.voice {
            types.insert(EventType::VoiceStateUpdate);
            types.insert(EventType::VoiceServerUpdate);
        }
        types
    }

    /// Connects to the gateway and runs the handlers until the bot stops.
    ///
    /// The bot stops at Ctrl+C (when on), at [`Context::shutdown`], or at a fatal gateway
    /// error.
    ///
    /// # Errors
    ///
    /// Gives an error when the start fails, the intent check fails, or a shard stops with a
    /// fatal error.
    pub async fn run(self) -> Result<RunOutcome, Error> {
        let flags = self.prepare().await?;
        let events = self.event_types();
        let input = IntentInput {
            configured: self.intents,
            events,
            flags,
        };
        let intents = intents::resolve(input, self.intent_check).map_err(Error::Intents)?;

        let mut builder = ShardManager::builder(self.context.http().clone(), intents)
            .shards(self.shards.clone())
            .event_types(events)
            .resume_sessions(self.sessions.clone());
        if let Some(presence) = self.presence.clone() {
            builder = builder.presence(presence);
        }
        if let Some(configure) = &self.gateway {
            builder = configure(builder);
        }
        let mut manager = builder.start().await?;
        *self
            .context
            .shared
            .shards
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = manager.shards().to_vec();
        self.context
            .shared
            .shard_count
            .store(manager.shard_count(), std::sync::atomic::Ordering::Relaxed);
        let _ = self.context.shared.event_types.set(events);
        #[cfg(feature = "voice")]
        if self.voice {
            let handles =
                cordial_voice::ShardHandles::new(manager.shards().to_vec(), manager.shard_count());
            let _ = self
                .context
                .shared
                .voice
                .set(cordial_voice::VoiceManager::new(handles));
        }

        let result = self.event_loop(&mut manager, intents, flags).await;

        let sessions = match self.shutdown_mode {
            ShutdownMode::Resumable => manager.close_resumable().await,
            ShutdownMode::End => {
                manager.close().await;
                HashMap::new()
            }
        };
        result.map(|()| RunOutcome { sessions })
    }

    async fn event_loop(
        &self,
        manager: &mut ShardManager,
        intents: Intents,
        flags: ApplicationFlags,
    ) -> Result<(), Error> {
        let mut shutdown = self.context.shared.shutdown.subscribe();
        let ctrl_c = tokio::signal::ctrl_c();
        tokio::pin!(ctrl_c);
        let mut workers: HashMap<u32, mpsc::UnboundedSender<Arc<Event>>> = HashMap::new();

        loop {
            let message = tokio::select! {
                message = manager.next_event() => message,
                _ = shutdown.wait_for(|stop| *stop) => return Ok(()),
                _ = &mut ctrl_c, if self.ctrl_c => {
                    tracing::info!("Ctrl+C: the bot stops");
                    return Ok(());
                }
            };
            let Some(ShardMessage {
                shard_id, event, ..
            }) = message
            else {
                return Ok(());
            };

            match event {
                ShardEvent::Dispatch(event) => {
                    let event = Arc::from(event);
                    self.dispatch(shard_id, event, &mut workers);
                }
                ShardEvent::Fatal(FatalError::Closed { code }) => {
                    let message = match code {
                        4013 => intents::invalid_message(intents),
                        4014 => intents::disallowed_message(intents, flags),
                        _ => FatalError::Closed { code }.to_string(),
                    };
                    return Err(Error::ShardFatal { shard_id, message });
                }
                ShardEvent::DecodeError { name, error } => {
                    tracing::warn!(
                        shard_id,
                        event = name,
                        error,
                        "an event could not be decoded"
                    );
                }
                _ => {}
            }
        }
    }

    /// Gives one event to the cache, the collectors, and the handlers.
    pub(crate) fn dispatch(
        &self,
        shard_id: u32,
        event: Arc<Event>,
        workers: &mut HashMap<u32, mpsc::UnboundedSender<Arc<Event>>>,
    ) {
        if let Some(cache) = self.context.cache() {
            cache.update(&event);
        }
        #[cfg(feature = "voice")]
        if let Some(voice) = self.context.voice() {
            voice.handle_event(&event);
        }

        match InteractionCreate::from_event_ref(&event) {
            Some(InteractionCreate(interaction)) => {
                self.router
                    .route_interaction(&self.context, &event, interaction);
            }
            None => {
                self.context.shared.collectors.dispatch(&event);
            }
        }

        match self.event_order {
            EventOrder::Concurrent => {
                let router = Arc::clone(&self.router);
                let context = self.context.clone();
                tokio::spawn(async move { run_event(&router, &context, &event).await });
            }
            EventOrder::PerShard => {
                let sender = workers
                    .entry(shard_id)
                    .or_insert_with(|| self.start_worker());
                let _ = sender.send(event);
            }
        }
    }

    fn start_worker(&self) -> mpsc::UnboundedSender<Arc<Event>> {
        let (sender, mut receiver) = mpsc::unbounded_channel::<Arc<Event>>();
        let router = Arc::clone(&self.router);
        let context = self.context.clone();
        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                run_event(&router, &context, &event).await;
            }
        });
        sender
    }

    /// Gives a handler for the `cordial-endpoint` crate, so that interactions come over HTTP.
    ///
    /// Call [`Bot::prepare`] before the server starts.
    #[cfg(feature = "endpoint")]
    #[must_use]
    pub fn endpoint_handler(&self) -> crate::endpoint::EndpointHandler<S> {
        crate::endpoint::EndpointHandler::new(self.context.clone(), Arc::clone(&self.router))
    }
}

/// Runs the event handlers of one event, one after the other.
async fn run_event<S: Send + Sync + 'static>(
    router: &Arc<Router<S>>,
    context: &Context<S>,
    event: &Arc<Event>,
) {
    for (source, future) in router.event_futures(context, event) {
        router.guard(context, source, future).await;
    }
}
