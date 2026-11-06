//! The sync: get the remote commands, make the plan, and send the changes.

use cordial_http::Client;
use cordial_model::command::{Command, CommandDefinition};
use cordial_model::id::{ApplicationId, CommandId};

use super::plan::{Action, Plan, PlanError, plan};
use super::{SyncCommand, SyncOptions, SyncScope};

/// The result of a sync.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct SyncReport {
    /// The plan that the sync made.
    pub plan: Plan,
    /// What the sync sent.
    pub outcome: SyncOutcome,
}

/// What a sync sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SyncOutcome {
    /// The dry-run mode was on. The sync sent no change.
    DryRun,
    /// The plan had no changes. The sync sent no change.
    NoChanges,
    /// The sync sent one request for each change.
    Granular,
    /// The sync sent one bulk overwrite.
    Bulk,
}

impl SyncOutcome {
    /// Tells if the sync sent a change request.
    #[must_use]
    pub fn sent_requests(self) -> bool {
        matches!(self, Self::Granular | Self::Bulk)
    }
}

/// An error of a sync.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SyncError {
    /// The request for the remote commands failed. The sync sent no change.
    #[error("cannot get the remote commands")]
    Fetch(#[source] cordial_http::Error),
    /// The local definitions are not valid. The sync sent no change.
    #[error(transparent)]
    Plan(#[from] PlanError),
    /// A request of the granular strategy failed. The sync sent no request after it.
    ///
    /// The next sync makes a new plan from the new remote state.
    #[error("cannot {failed} after {} applied actions", applied.len())]
    Granular {
        /// The actions that Discord applied before the failure, in order.
        applied: Vec<Action>,
        /// The action that failed.
        failed: Box<Action>,
        /// The error of the request.
        #[source]
        source: cordial_http::Error,
    },
    /// The bulk overwrite failed. Discord applied no change.
    #[error("the bulk overwrite failed")]
    Bulk(#[source] cordial_http::Error),
}

/// Registers the local commands in one scope and sends only the changes.
///
/// The steps:
/// 1. Get the remote commands with their localizations.
/// 2. Make the plan (see [`plan`]) and write its summary to the log.
/// 3. Stop here in the dry-run mode, or when the plan has no changes.
/// 4. Send the changes with the strategy of the options.
///
/// # Errors
///
/// Gives an error when a request fails or when two local definitions have the same key.
/// After a failed granular request, the error tells the actions that Discord applied.
pub async fn sync(
    client: &Client,
    application_id: ApplicationId,
    scope: SyncScope,
    local: &[SyncCommand],
    options: &SyncOptions,
) -> Result<SyncReport, SyncError> {
    let target = Target {
        client,
        application_id,
        scope,
    };

    if let SyncScope::Guild(_) = scope {
        warn_global_only_fields(local);
    }

    let remote = target.fetch().await.map_err(SyncError::Fetch)?;
    let plan = plan(local, &remote, options)?;
    tracing::info!(%scope, dry_run = options.dry_run, "command sync plan: {plan}");

    let outcome = if options.dry_run {
        SyncOutcome::DryRun
    } else if !plan.has_changes() {
        SyncOutcome::NoChanges
    } else if options.strategy.uses_bulk(plan.change_count()) {
        target
            .bulk(plan.bulk_definitions().to_vec())
            .await
            .map_err(SyncError::Bulk)?;
        SyncOutcome::Bulk
    } else {
        target.granular(&plan).await?;
        SyncOutcome::Granular
    };

    Ok(SyncReport { plan, outcome })
}

/// Warns about settings that Discord drops from guild commands.
fn warn_global_only_fields(local: &[SyncCommand]) {
    for command in local {
        let definition = &command.definition;
        if definition.contexts.is_some() || definition.integration_types.is_some() {
            tracing::warn!(
                command = %definition.name,
                "contexts and integration_types have no effect on a guild command"
            );
        }
    }
}

/// The application and the scope that receive the requests.
struct Target<'a> {
    client: &'a Client,
    application_id: ApplicationId,
    scope: SyncScope,
}

impl Target<'_> {
    async fn fetch(&self) -> Result<Vec<Command>, cordial_http::Error> {
        match self.scope {
            SyncScope::Global => {
                self.client
                    .global_commands(self.application_id)
                    .with_localizations(true)
                    .await
            }
            SyncScope::Guild(guild_id) => {
                self.client
                    .guild_commands(self.application_id, guild_id)
                    .with_localizations(true)
                    .await
            }
        }
    }

    /// Sends the changes one by one and stops at the first failure.
    async fn granular(&self, plan: &Plan) -> Result<(), SyncError> {
        let mut applied = Vec::new();

        for action in plan.changes() {
            if let Err(source) = self.send(action).await {
                return Err(SyncError::Granular {
                    applied,
                    failed: Box::new(action.clone()),
                    source,
                });
            }
            tracing::debug!("command sync: {action}");
            applied.push(action.clone());
        }

        Ok(())
    }

    async fn send(&self, action: &Action) -> Result<(), cordial_http::Error> {
        match action {
            Action::Unchanged { .. } => Ok(()),
            Action::Update { id, definition } | Action::Rename { id, definition, .. } => {
                self.edit(*id, definition.clone()).await
            }
            Action::Create { definition } => self.create(definition.clone()).await,
            Action::Delete { id, .. } => self.delete(*id).await,
        }
    }

    async fn create(&self, definition: CommandDefinition) -> Result<(), cordial_http::Error> {
        let application_id = self.application_id;
        match self.scope {
            SyncScope::Global => {
                self.client
                    .create_global_command(application_id, definition)
                    .await?;
            }
            SyncScope::Guild(guild_id) => {
                self.client
                    .create_guild_command(application_id, guild_id, definition)
                    .await?;
            }
        }
        Ok(())
    }

    async fn edit(
        &self,
        id: CommandId,
        definition: CommandDefinition,
    ) -> Result<(), cordial_http::Error> {
        let application_id = self.application_id;
        match self.scope {
            SyncScope::Global => {
                self.client
                    .edit_global_command(application_id, id, definition)
                    .await?;
            }
            SyncScope::Guild(guild_id) => {
                self.client
                    .edit_guild_command(application_id, guild_id, id, definition)
                    .await?;
            }
        }
        Ok(())
    }

    async fn delete(&self, id: CommandId) -> Result<(), cordial_http::Error> {
        let application_id = self.application_id;
        match self.scope {
            SyncScope::Global => self.client.delete_global_command(application_id, id).await,
            SyncScope::Guild(guild_id) => {
                self.client
                    .delete_guild_command(application_id, guild_id, id)
                    .await
            }
        }
    }

    async fn bulk(&self, definitions: Vec<CommandDefinition>) -> Result<(), cordial_http::Error> {
        let application_id = self.application_id;
        match self.scope {
            SyncScope::Global => {
                self.client
                    .set_global_commands(application_id, definitions)
                    .await?;
            }
            SyncScope::Guild(guild_id) => {
                self.client
                    .set_guild_commands(application_id, guild_id, definitions)
                    .await?;
            }
        }
        Ok(())
    }
}
