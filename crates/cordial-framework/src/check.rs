//! Checks and cooldowns that run before a command handler.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cordial_model::Permissions;
use cordial_model::id::UserId;
use dashmap::DashMap;

use crate::interaction::InteractionContext;

/// The reason that a check stops a command. The user sees the message (ephemeral).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct CheckFailure {
    /// The message for the user.
    pub message: String,
}

impl CheckFailure {
    /// Makes a failure with a message for the user.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

type CheckFuture = Pin<Box<dyn Future<Output = Result<(), CheckFailure>> + Send>>;

/// A check that runs before a command handler.
pub struct Check<S> {
    run: Arc<dyn Fn(InteractionContext<S>) -> CheckFuture + Send + Sync>,
}

impl<S> Clone for Check<S> {
    fn clone(&self) -> Self {
        Self {
            run: Arc::clone(&self.run),
        }
    }
}

impl<S> std::fmt::Debug for Check<S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Check").finish_non_exhaustive()
    }
}

impl<S: Send + Sync + 'static> Check<S> {
    /// Makes a check from an async function.
    pub fn new<F, Fut>(check: F) -> Self
    where
        F: Fn(InteractionContext<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), CheckFailure>> + Send + 'static,
    {
        Self {
            run: Arc::new(move |context| Box::pin(check(context))),
        }
    }

    /// Allows the command only in guilds.
    ///
    /// Prefer the command `contexts` for this: then Discord does not show the command in
    /// DMs at all. This check is for commands that share a definition.
    #[must_use]
    pub fn guild_only() -> Self {
        Self::new(|context: InteractionContext<S>| async move {
            context
                .interaction()
                .guild_id
                .map(|_| ())
                .ok_or_else(|| CheckFailure::new("This command works only in a server."))
        })
    }

    /// Allows the command only to members with all of the permissions, in the channel of
    /// the interaction.
    #[must_use]
    pub fn permissions(required: Permissions) -> Self {
        Self::new(move |context: InteractionContext<S>| async move {
            let granted = context
                .member_permissions()
                .unwrap_or_else(Permissions::empty);
            let missing = required - granted;
            if missing.is_empty() {
                Ok(())
            } else {
                Err(CheckFailure::new(format!(
                    "You need these permissions: {missing:?}."
                )))
            }
        })
    }

    /// Allows the command only to some users, for example the owners of the bot.
    #[must_use]
    pub fn users(users: impl IntoIterator<Item = UserId>) -> Self {
        let users: Arc<[UserId]> = users.into_iter().collect();
        Self::new(move |context: InteractionContext<S>| {
            let users = Arc::clone(&users);
            async move {
                if users.contains(&context.user_id()) {
                    Ok(())
                } else {
                    Err(CheckFailure::new("You cannot use this command."))
                }
            }
        })
    }

    pub(crate) async fn run(&self, context: InteractionContext<S>) -> Result<(), CheckFailure> {
        (self.run)(context).await
    }
}

/// What a cooldown counts separately.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CooldownScope {
    /// Each user.
    User,
    /// Each guild. DMs count for each user.
    Guild,
    /// Each channel.
    Channel,
    /// All uses together.
    Global,
}

/// A minimum time between two uses of a command.
#[derive(Clone, Debug)]
pub struct Cooldown {
    scope: CooldownScope,
    duration: Duration,
    last_use: Arc<DashMap<u64, Instant>>,
}

impl Cooldown {
    /// Makes a cooldown.
    #[must_use]
    pub fn new(scope: CooldownScope, duration: Duration) -> Self {
        Self {
            scope,
            duration,
            last_use: Arc::new(DashMap::new()),
        }
    }

    /// Records a use now. Gives the time to wait when the cooldown is active.
    pub(crate) fn try_use<S: Send + Sync + 'static>(
        &self,
        context: &InteractionContext<S>,
    ) -> Result<(), Duration> {
        let interaction = context.interaction();
        let user = context.user_id().get();
        let key = match self.scope {
            CooldownScope::User => user,
            CooldownScope::Guild => interaction
                .guild_id
                .map_or(user, cordial_model::id::Id::get),
            CooldownScope::Channel => interaction
                .channel_id()
                .map_or(user, cordial_model::id::Id::get),
            CooldownScope::Global => 0,
        };

        let now = Instant::now();
        let mut entry = self.last_use.entry(key).or_insert(now - self.duration);
        let elapsed = now.saturating_duration_since(*entry);
        if elapsed < self.duration {
            return Err(self.duration - elapsed);
        }
        *entry = now;
        drop(entry);

        // Remove old entries now and then, so that the map does not grow without limit.
        if self.last_use.len() > 10_000 {
            self.last_use
                .retain(|_, last| now.saturating_duration_since(*last) < self.duration);
        }
        Ok(())
    }
}
