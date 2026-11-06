//! The intent check: compare the intents with the handlers, the cache, and the application
//! flags before the bot connects. See `docs/ARCHITECTURE.md` section 9.2.

use cordial_model::application::ApplicationFlags;
use cordial_model::gateway::Intents;
use cordial_model::gateway::event::{EventType, EventTypes};

use crate::error::IntentProblem;

/// How strict the intent check is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum IntentCheck {
    /// Log the problems. A privileged intent that is not on still stops the bot, because
    /// Discord closes the connection for it.
    #[default]
    Warn,
    /// Stop the bot when a handler or the cache needs an intent that is not set.
    Strict,
    /// Do no check.
    Off,
}

/// The facts that the check uses.
#[derive(Clone, Copy, Debug)]
pub(crate) struct IntentInput {
    /// The intents that the user set, or `None` to compute them.
    pub(crate) configured: Option<Intents>,
    /// The event types of the handlers and the cache.
    pub(crate) events: EventTypes,
    /// The application flags.
    pub(crate) flags: ApplicationFlags,
}

/// The privileged intents with the application flags that turn them on.
const PRIVILEGED: [(Intents, ApplicationFlags); 3] = [
    (
        Intents::GUILD_MEMBERS,
        ApplicationFlags::GATEWAY_GUILD_MEMBERS
            .union(ApplicationFlags::GATEWAY_GUILD_MEMBERS_LIMITED),
    ),
    (
        Intents::GUILD_PRESENCES,
        ApplicationFlags::GATEWAY_PRESENCE.union(ApplicationFlags::GATEWAY_PRESENCE_LIMITED),
    ),
    (
        Intents::MESSAGE_CONTENT,
        ApplicationFlags::GATEWAY_MESSAGE_CONTENT
            .union(ApplicationFlags::GATEWAY_MESSAGE_CONTENT_LIMITED),
    ),
];

/// Gives the privileged intents that the application flags turn on.
pub(crate) fn enabled_privileged(flags: ApplicationFlags) -> Intents {
    PRIVILEGED
        .iter()
        .filter(|(_, flag)| flags.intersects(*flag))
        .map(|(intent, _)| *intent)
        .collect()
}

fn reads_messages(events: EventTypes) -> bool {
    events.contains(EventType::MessageCreate) || events.contains(EventType::MessageUpdate)
}

/// Runs the check and gives the intents to use.
///
/// # Errors
///
/// Gives an error for a privileged intent that is not on, and for missing intents in strict
/// mode.
pub(crate) fn resolve(input: IntentInput, mode: IntentCheck) -> Result<Intents, IntentProblem> {
    let enabled = enabled_privileged(input.flags);
    match input.configured {
        Some(intents) => check_configured(intents, input.events, enabled, mode),
        None => Ok(compute(input.events, enabled, mode)),
    }
}

/// Computes the intents from the events. Privileged intents that are not on are left out
/// with a warning, because Discord would close the connection.
fn compute(events: EventTypes, enabled: Intents, mode: IntentCheck) -> Intents {
    let mut intents = events.intents();
    if reads_messages(events) && enabled.contains(Intents::MESSAGE_CONTENT) {
        intents |= Intents::MESSAGE_CONTENT;
    }

    let blocked = intents.privileged() - enabled;
    if !blocked.is_empty() {
        if mode != IntentCheck::Off {
            for kind in events
                .iter()
                .filter(|kind| kind.intents().intersects(blocked))
            {
                tracing::warn!(
                    event = ?kind,
                    intents = ?kind.intents() & blocked,
                    "a handler or the cache needs a privileged intent that is not on in the \
                     Developer Portal; the bot does not get this event"
                );
            }
        }
        intents -= blocked;
    }

    if mode != IntentCheck::Off {
        warn_message_content(events, intents);
    }
    tracing::debug!(
        ?intents,
        "computed the intents from the handlers and the cache"
    );
    intents
}

fn check_configured(
    intents: Intents,
    events: EventTypes,
    enabled: Intents,
    mode: IntentCheck,
) -> Result<Intents, IntentProblem> {
    if mode == IntentCheck::Off {
        return Ok(intents);
    }

    let not_enabled = intents.privileged() - enabled;
    if !not_enabled.is_empty() {
        return Err(IntentProblem::PrivilegedNotEnabled(not_enabled));
    }

    let mut missing = Intents::empty();
    let mut used = Intents::empty();
    for kind in events.iter() {
        let options = kind.intents();
        if options.is_empty() {
            continue;
        }
        if options.intersects(intents) {
            used |= options & intents;
        } else {
            tracing::warn!(
                event = ?kind,
                needs_one_of = ?options,
                "a handler or the cache needs an intent that is not set; the bot does not get \
                 this event"
            );
            missing |= options;
        }
    }
    if reads_messages(events) && intents.contains(Intents::MESSAGE_CONTENT) {
        used |= Intents::MESSAGE_CONTENT;
    }

    let unused = intents - used;
    if !unused.is_empty() {
        tracing::info!(
            ?unused,
            "no handler and no cache resource uses these intents; remove them to get fewer events"
        );
    }
    warn_message_content(events, intents);

    if mode == IntentCheck::Strict && !missing.is_empty() {
        return Err(IntentProblem::Missing(missing));
    }
    Ok(intents)
}

fn warn_message_content(events: EventTypes, intents: Intents) {
    if reads_messages(events) && !intents.contains(Intents::MESSAGE_CONTENT) {
        tracing::warn!(
            "message handlers run without the MESSAGE_CONTENT intent; the content, embeds, \
             attachments, components, and poll fields are empty, except in DMs, in messages \
             that mention the bot, and in messages of the bot"
        );
    }
}

/// Gives a message for close code 4014 (disallowed intents).
pub(crate) fn disallowed_message(intents: Intents, flags: ApplicationFlags) -> String {
    let not_enabled = intents.privileged() - enabled_privileged(flags);
    format!(
        "Discord does not allow the intents: the privileged intents {not_enabled:?} are not on; \
         turn them on in the Developer Portal (Bot page). A verified bot needs approval for them"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(kinds: &[EventType]) -> EventTypes {
        kinds.iter().copied().collect()
    }

    #[test]
    fn computes_intents_and_drops_blocked_privileged_intents() {
        let input = IntentInput {
            configured: None,
            events: events(&[EventType::MessageCreate, EventType::GuildMemberAdd]),
            flags: ApplicationFlags::GATEWAY_MESSAGE_CONTENT_LIMITED,
        };

        let intents = resolve(input, IntentCheck::Warn).unwrap();

        assert!(intents.contains(Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT));
        assert!(!intents.contains(Intents::GUILD_MEMBERS));
    }

    #[test]
    fn privileged_intent_that_is_not_on_is_an_error() {
        let input = IntentInput {
            configured: Some(Intents::GUILDS | Intents::GUILD_PRESENCES),
            events: EventTypes::empty(),
            flags: ApplicationFlags::empty(),
        };

        assert_eq!(
            resolve(input, IntentCheck::Warn),
            Err(IntentProblem::PrivilegedNotEnabled(
                Intents::GUILD_PRESENCES
            ))
        );
    }

    #[test]
    fn strict_mode_rejects_missing_intents() {
        let input = IntentInput {
            configured: Some(Intents::GUILDS),
            events: events(&[EventType::MessageReactionAdd]),
            flags: ApplicationFlags::empty(),
        };

        assert!(resolve(input, IntentCheck::Warn).is_ok());
        assert!(matches!(
            resolve(input, IntentCheck::Strict),
            Err(IntentProblem::Missing(missing)) if missing.contains(Intents::GUILD_MESSAGE_REACTIONS)
        ));
    }
}
