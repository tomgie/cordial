//! The plan: one action for each local and each remote command.

use std::collections::{HashMap, HashSet};
use std::fmt;

use cordial_model::Patch;
use cordial_model::command::{Command, CommandDefinition, CommandType};
use cordial_model::id::CommandId;
use cordial_model::locale::Localizations;

use super::normalize::is_same;
use super::{SyncCommand, SyncOptions};

/// What the sync does with one command.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Action {
    /// The command on Discord is the same as the local definition. The sync sends nothing.
    ///
    /// An Entry Point command with no local definition also gets this action, unless
    /// [`SyncOptions::delete_entry_point`] is set.
    Unchanged {
        /// The command ID.
        id: CommandId,
        /// The command type.
        kind: CommandType,
        /// The command name.
        name: String,
    },
    /// The command content changed. The sync edits the command (`PATCH`).
    Update {
        /// The command ID.
        id: CommandId,
        /// The definition that the edit request sends.
        definition: CommandDefinition,
    },
    /// The command has a new name. The sync edits the command (`PATCH`), so it keeps its ID
    /// and its permission overwrites.
    Rename {
        /// The command ID.
        id: CommandId,
        /// The name on Discord.
        from: String,
        /// The definition that the edit request sends, with the new name.
        definition: CommandDefinition,
    },
    /// The command is new. The sync creates it (`POST`).
    Create {
        /// The definition to create.
        definition: CommandDefinition,
    },
    /// The command has no local definition. The sync deletes it (`DELETE`).
    Delete {
        /// The command ID.
        id: CommandId,
        /// The command type.
        kind: CommandType,
        /// The command name.
        name: String,
    },
}

impl Action {
    /// Tells if the action sends a request in the granular strategy.
    #[must_use]
    pub fn is_change(&self) -> bool {
        !matches!(self, Self::Unchanged { .. })
    }

    /// Gives the type of the command.
    #[must_use]
    pub fn kind(&self) -> CommandType {
        match self {
            Self::Unchanged { kind, .. } | Self::Delete { kind, .. } => *kind,
            Self::Update { definition, .. }
            | Self::Rename { definition, .. }
            | Self::Create { definition } => definition.kind,
        }
    }

    /// Gives the name of the command after the action.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Unchanged { name, .. } | Self::Delete { name, .. } => name,
            Self::Update { definition, .. }
            | Self::Rename { definition, .. }
            | Self::Create { definition } => &definition.name,
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = KindLabel(self.kind());
        let name = self.name();
        match self {
            Self::Unchanged { .. } => write!(formatter, "keep {kind} command `{name}`"),
            Self::Update { .. } => write!(formatter, "update {kind} command `{name}`"),
            Self::Rename { from, .. } => {
                write!(formatter, "rename {kind} command `{from}` to `{name}`")
            }
            Self::Create { .. } => write!(formatter, "create {kind} command `{name}`"),
            Self::Delete { .. } => write!(formatter, "delete {kind} command `{name}`"),
        }
    }
}

/// A readable name of a command type for messages.
struct KindLabel(CommandType);

impl fmt::Display for KindLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            CommandType::ChatInput => formatter.write_str("slash"),
            CommandType::User => formatter.write_str("user"),
            CommandType::Message => formatter.write_str("message"),
            CommandType::PrimaryEntryPoint => formatter.write_str("entry point"),
            other => write!(formatter, "type {}", other.value()),
        }
    }
}

/// An error in the local definitions.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PlanError {
    /// Two local definitions have the same type and name. Discord allows only one.
    #[error("two local {} commands have the name `{name}`", KindLabel(*kind))]
    DuplicateCommand {
        /// The command type.
        kind: CommandType,
        /// The command name.
        name: String,
    },
}

/// The actions of one sync, in the order that the granular strategy sends them.
///
/// The `Display` form is a summary: `created 1, updated 2, renamed 0, deleted 1, unchanged 14`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    actions: Vec<Action>,
    bulk: Vec<CommandDefinition>,
}

impl Plan {
    /// Gives all actions: the changes in the order of the requests, then the unchanged
    /// commands.
    #[must_use]
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// Gives the actions that send a request, in the order of the requests.
    pub fn changes(&self) -> impl Iterator<Item = &Action> {
        self.actions.iter().filter(|action| action.is_change())
    }

    /// Gives the list for a bulk overwrite: all local definitions, with the ID of the matched
    /// remote command, and a kept Entry Point command.
    #[must_use]
    pub fn bulk_definitions(&self) -> &[CommandDefinition] {
        &self.bulk
    }

    /// Tells if the plan has an action that sends a request.
    #[must_use]
    pub fn has_changes(&self) -> bool {
        self.changes().next().is_some()
    }

    /// Gives the number of actions that send a request.
    #[must_use]
    pub fn change_count(&self) -> usize {
        self.changes().count()
    }

    /// Gives the number of commands to create.
    #[must_use]
    pub fn created(&self) -> usize {
        self.count(|action| matches!(action, Action::Create { .. }))
    }

    /// Gives the number of commands to update.
    #[must_use]
    pub fn updated(&self) -> usize {
        self.count(|action| matches!(action, Action::Update { .. }))
    }

    /// Gives the number of commands to rename.
    #[must_use]
    pub fn renamed(&self) -> usize {
        self.count(|action| matches!(action, Action::Rename { .. }))
    }

    /// Gives the number of commands to delete.
    #[must_use]
    pub fn deleted(&self) -> usize {
        self.count(|action| matches!(action, Action::Delete { .. }))
    }

    /// Gives the number of unchanged commands.
    #[must_use]
    pub fn unchanged(&self) -> usize {
        self.count(|action| matches!(action, Action::Unchanged { .. }))
    }

    fn count(&self, filter: impl Fn(&Action) -> bool) -> usize {
        self.actions.iter().filter(|action| filter(action)).count()
    }
}

impl fmt::Display for Plan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "created {}, updated {}, renamed {}, deleted {}, unchanged {}",
            self.created(),
            self.updated(),
            self.renamed(),
            self.deleted(),
            self.unchanged()
        )
    }
}

/// The key that Discord makes unique in each scope.
type Key<'a> = (CommandType, &'a str);

/// Makes the plan for one scope. This function sends no request.
///
/// Give the remote commands with their localizations (`with_localizations(true)`). Without
/// them, each localized command looks changed.
///
/// # Errors
///
/// Gives an error when two local definitions have the same type and name.
pub fn plan(
    local: &[SyncCommand],
    remote: &[Command],
    options: &SyncOptions,
) -> Result<Plan, PlanError> {
    check_duplicates(local)?;

    let matches = match_commands(local, remote);

    let mut deletes = Vec::new();
    let mut renames = Vec::new();
    let mut updates = Vec::new();
    let mut creates = Vec::new();
    let mut unchanged = Vec::new();
    let mut bulk = Vec::new();

    for (command, matched) in local.iter().zip(&matches) {
        let definition = &command.definition;
        let mut bulk_definition = definition.clone();
        bulk_definition.id = None;

        match matched {
            None => creates.push(Action::Create {
                definition: definition.clone(),
            }),
            Some(index) => {
                let current = &remote[*index];
                bulk_definition.id = Some(current.id);
                if current.name != definition.name {
                    renames.push(Action::Rename {
                        id: current.id,
                        from: current.name.clone(),
                        definition: edit_definition(definition, current),
                    });
                } else if is_same(definition, current) {
                    unchanged.push(Action::Unchanged {
                        id: current.id,
                        kind: current.kind,
                        name: current.name.clone(),
                    });
                } else {
                    updates.push(Action::Update {
                        id: current.id,
                        definition: edit_definition(definition, current),
                    });
                }
            }
        }

        bulk.push(bulk_definition);
    }

    let matched: HashSet<usize> = matches.iter().flatten().copied().collect();
    for (index, current) in remote.iter().enumerate() {
        if matched.contains(&index) {
            continue;
        }

        let keep = current.kind == CommandType::PrimaryEntryPoint && !options.delete_entry_point;
        if keep {
            unchanged.push(Action::Unchanged {
                id: current.id,
                kind: current.kind,
                name: current.name.clone(),
            });
            bulk.push(CommandDefinition::from(current));
        } else {
            deletes.push(Action::Delete {
                id: current.id,
                kind: current.kind,
                name: current.name.clone(),
            });
        }
    }

    // The order of the requests prevents name conflicts and count limits:
    // 1. Deletes free names and command slots (for example 100 slash commands) first.
    // 2. Renames come before creates. A create (`POST`) with the old name of a renamed command
    //    otherwise overwrites that command, because a create is an upsert by name.
    // 3. Updates change no name, so their position is free. They come before creates so that
    //    a failed create leaves the existing commands current.
    let mut actions = deletes;
    actions.extend(order_renames(renames));
    actions.extend(updates);
    actions.extend(creates);
    actions.extend(unchanged);

    Ok(Plan { actions, bulk })
}

fn check_duplicates(local: &[SyncCommand]) -> Result<(), PlanError> {
    let mut keys = HashSet::new();
    for command in local {
        let definition = &command.definition;
        if !keys.insert((definition.kind, definition.name.as_str())) {
            return Err(PlanError::DuplicateCommand {
                kind: definition.kind,
                name: definition.name.clone(),
            });
        }
    }
    Ok(())
}

/// Gives for each local command the index of its remote command, if one matches.
///
/// A match by the current name comes first. An earlier name matches only a remote command
/// that no local command has by its current name.
fn match_commands(local: &[SyncCommand], remote: &[Command]) -> Vec<Option<usize>> {
    let remote_index: HashMap<Key<'_>, usize> = remote
        .iter()
        .enumerate()
        .map(|(index, command)| ((command.kind, command.name.as_str()), index))
        .collect();

    let mut matches: Vec<Option<usize>> = local
        .iter()
        .map(|command| {
            let key = (command.definition.kind, command.definition.name.as_str());
            remote_index.get(&key).copied()
        })
        .collect();

    let mut taken: HashSet<usize> = matches.iter().flatten().copied().collect();
    for (command, matched) in local.iter().zip(&mut matches) {
        if matched.is_some() {
            continue;
        }

        let kind = command.definition.kind;
        let previous = command
            .previous_names
            .iter()
            .filter_map(|name| remote_index.get(&(kind, name.as_str())).copied())
            .find(|index| !taken.contains(index));
        if let Some(index) = previous {
            taken.insert(index);
            *matched = Some(index);
        }
    }

    matches
}

/// Puts the renames in an order where each new name is free when its request runs.
///
/// A rename from `b` to `c` must run before a rename from `a` to `b`. A cycle (two commands
/// that swap names) has no such order. Then the remaining renames keep their order, Discord
/// rejects the first conflict, and the sync reports it.
fn order_renames(mut pending: Vec<Action>) -> Vec<Action> {
    let mut ordered = Vec::with_capacity(pending.len());

    loop {
        let free = pending.iter().position(|action| {
            !pending.iter().any(|other| match other {
                Action::Rename { from, .. } => {
                    other.kind() == action.kind() && from.as_str() == action.name()
                }
                _ => false,
            })
        });
        let Some(index) = free else {
            break;
        };
        ordered.push(pending.remove(index));
    }

    ordered.extend(pending);
    ordered
}

/// Makes the definition for an edit request.
///
/// An edit keeps each field that the request does not send. Thus the definition sends an
/// explicit reset for each field that the remote command has and the local definition does
/// not.
fn edit_definition(local: &CommandDefinition, remote: &Command) -> CommandDefinition {
    let mut definition = local.clone();
    definition.id = None;

    if definition.default_member_permissions.is_keep() {
        definition.default_member_permissions = Patch::Clear;
    }

    reset_localizations(
        &mut definition.name_localizations,
        remote.name_localizations.as_ref(),
    );
    reset_localizations(
        &mut definition.description_localizations,
        remote.description_localizations.as_ref(),
    );

    definition
}

fn reset_localizations(local: &mut Option<Localizations>, remote: Option<&Localizations>) {
    let local_is_empty = local.as_ref().is_none_or(Localizations::is_empty);
    let remote_has_entries = remote.is_some_and(|map| !map.is_empty());
    if local_is_empty && remote_has_entries {
        *local = Some(Localizations::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rename(kind: CommandType, from: &str, to: &str) -> Action {
        let mut definition = CommandDefinition::chat_input(to, "x");
        definition.kind = kind;
        Action::Rename {
            id: CommandId::new_const(1),
            from: from.to_owned(),
            definition,
        }
    }

    fn rename_names(actions: &[Action]) -> Vec<(&str, &str)> {
        actions
            .iter()
            .map(|action| match action {
                Action::Rename { from, .. } => (from.as_str(), action.name()),
                _ => unreachable!(),
            })
            .collect()
    }

    #[test]
    fn renames_run_in_chain_order() {
        let renames = vec![
            rename(CommandType::ChatInput, "a", "b"),
            rename(CommandType::ChatInput, "b", "c"),
        ];

        let ordered = order_renames(renames);

        assert_eq!(rename_names(&ordered), [("b", "c"), ("a", "b")]);
    }

    #[test]
    fn renames_of_other_types_do_not_conflict() {
        let renames = vec![
            rename(CommandType::ChatInput, "a", "b"),
            rename(CommandType::User, "b", "c"),
        ];

        let ordered = order_renames(renames);

        assert_eq!(rename_names(&ordered), [("a", "b"), ("b", "c")]);
    }

    #[test]
    fn rename_cycle_keeps_its_order() {
        let renames = vec![
            rename(CommandType::ChatInput, "a", "b"),
            rename(CommandType::ChatInput, "b", "a"),
        ];

        let ordered = order_renames(renames);

        assert_eq!(rename_names(&ordered), [("a", "b"), ("b", "a")]);
    }
}
