//! Auto moderation endpoints. They all need `MANAGE_GUILD`.

use cordial_model::auto_moderation::{
    AutoModerationAction, AutoModerationEventType, AutoModerationRule,
    AutoModerationTriggerMetadata, AutoModerationTriggerType,
};
use cordial_model::id::{AutoModerationRuleId, ChannelId, GuildId, RoleId};
use cordial_model::validate::ValidationError;
use serde::Serialize;

use super::limits;
use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, RequestBody, check};
use crate::route::{MajorParameter, Method, Route};

/// The maximum number of exempt roles of a rule.
const EXEMPT_ROLE_LIMIT: usize = 20;
/// The maximum number of exempt channels of a rule.
const EXEMPT_CHANNEL_LIMIT: usize = 50;
/// The maximum number of keywords, and the maximum number of characters in each.
const KEYWORD_LIMITS: (usize, usize) = (1000, 60);
/// The maximum number of regular expressions, and the maximum number of characters in each.
const REGEX_LIMITS: (usize, usize) = (10, 260);
/// The maximum number of allowed words, and the maximum number of characters in each.
/// Keyword preset rules allow 1000 words; the check uses that higher limit.
const ALLOW_LIST_LIMITS: (usize, usize) = (1000, 60);
/// The maximum number of unique mentions in a mention spam rule.
const MENTION_TOTAL_LIMIT: u64 = 50;

/// `GET /guilds/{guild.id}/auto-moderation/rules`: the auto moderation rules of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetAutoModerationRules {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetAutoModerationRules {
    type Response = Vec<AutoModerationRule>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/auto-moderation/rules",
            format!("/guilds/{}/auto-moderation/rules", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/auto-moderation/rules/{auto_moderation_rule.id}`: an auto
/// moderation rule.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetAutoModerationRule {
    client: Client,
    guild_id: GuildId,
    rule_id: AutoModerationRuleId,
}

impl Endpoint for GetAutoModerationRule {
    type Response = AutoModerationRule;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/auto-moderation/rules/{rule_id}",
            format!(
                "/guilds/{}/auto-moderation/rules/{}",
                self.guild_id, self.rule_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// The fields of a rule that a create or an edit request can set.
#[derive(Clone, Debug, Default, Serialize)]
struct RuleBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_type: Option<AutoModerationEventType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trigger_type: Option<AutoModerationTriggerType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trigger_metadata: Option<AutoModerationTriggerMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actions: Option<Vec<AutoModerationAction>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exempt_roles: Option<Vec<RoleId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exempt_channels: Option<Vec<ChannelId>>,
}

impl RuleBody {
    fn check(&self) -> Result<(), ValidationError> {
        if let Some(roles) = &self.exempt_roles {
            limits::count("exempt_roles", roles.len(), EXEMPT_ROLE_LIMIT)?;
        }
        if let Some(channels) = &self.exempt_channels {
            limits::count("exempt_channels", channels.len(), EXEMPT_CHANNEL_LIMIT)?;
        }
        if let Some(metadata) = &self.trigger_metadata {
            check_texts(
                "trigger_metadata.keyword_filter",
                metadata.keyword_filter.as_deref(),
                KEYWORD_LIMITS,
            )?;
            check_texts(
                "trigger_metadata.regex_patterns",
                metadata.regex_patterns.as_deref(),
                REGEX_LIMITS,
            )?;
            check_texts(
                "trigger_metadata.allow_list",
                metadata.allow_list.as_deref(),
                ALLOW_LIST_LIMITS,
            )?;
            limits::optional_range(
                "trigger_metadata.mention_total_limit",
                metadata.mention_total_limit.map(u64::from),
                0,
                MENTION_TOTAL_LIMIT,
            )?;
        }
        Ok(())
    }
}

/// Checks the number of texts in a list and the length of each text.
fn check_texts(
    field: &str,
    texts: Option<&[String]>,
    (max_count, max_length): (usize, usize),
) -> Result<(), ValidationError> {
    let Some(texts) = texts else {
        return Ok(());
    };
    limits::count(field, texts.len(), max_count)?;
    for (index, text) in texts.iter().enumerate() {
        limits::length(&format!("{field}[{index}]"), text, 0, max_length)?;
    }
    Ok(())
}

/// Adds the setters of the shared rule fields to a request type with a `body: RuleBody` field.
macro_rules! rule_setters {
    () => {
        /// Sets the data of the trigger. Some trigger types need it.
        pub fn trigger_metadata(mut self, metadata: AutoModerationTriggerMetadata) -> Self {
            self.body.trigger_metadata = Some(metadata);
            self
        }

        /// Sets the actions when the rule triggers.
        pub fn actions(mut self, actions: Vec<AutoModerationAction>) -> Self {
            self.body.actions = Some(actions);
            self
        }

        /// Turns the rule on or off.
        pub fn enabled(mut self, enabled: bool) -> Self {
            self.body.enabled = Some(enabled);
            self
        }

        /// Sets the roles that the rule does not affect, up to 20.
        pub fn exempt_roles(mut self, role_ids: Vec<RoleId>) -> Self {
            self.body.exempt_roles = Some(role_ids);
            self
        }

        /// Sets the channels that the rule does not affect, up to 50.
        pub fn exempt_channels(mut self, channel_ids: Vec<ChannelId>) -> Self {
            self.body.exempt_channels = Some(channel_ids);
            self
        }

        /// Sets the reason for the audit log.
        pub fn reason(mut self, reason: impl Into<String>) -> Self {
            self.reason = Some(reason.into());
            self
        }
    };
}

/// `POST /guilds/{guild.id}/auto-moderation/rules`: makes an auto moderation rule.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateAutoModerationRule {
    client: Client,
    guild_id: GuildId,
    body: RuleBody,
    reason: Option<String>,
}

impl CreateAutoModerationRule {
    rule_setters!();
}

impl Endpoint for CreateAutoModerationRule {
    type Response = AutoModerationRule;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/guilds/{guild_id}/auto-moderation/rules",
            format!("/guilds/{}/auto-moderation/rules", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || self.body.check())?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `PATCH /guilds/{guild.id}/auto-moderation/rules/{auto_moderation_rule.id}`: changes an auto
/// moderation rule.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditAutoModerationRule {
    client: Client,
    guild_id: GuildId,
    rule_id: AutoModerationRuleId,
    body: RuleBody,
    reason: Option<String>,
}

impl EditAutoModerationRule {
    rule_setters!();

    /// Sets the name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.body.name = Some(name.into());
        self
    }

    /// Sets when the rule runs.
    pub fn event_type(mut self, event_type: AutoModerationEventType) -> Self {
        self.body.event_type = Some(event_type);
        self
    }
}

impl Endpoint for EditAutoModerationRule {
    type Response = AutoModerationRule;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/auto-moderation/rules/{rule_id}",
            format!(
                "/guilds/{}/auto-moderation/rules/{}",
                self.guild_id, self.rule_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || self.body.check())?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `DELETE /guilds/{guild.id}/auto-moderation/rules/{auto_moderation_rule.id}`: deletes an
/// auto moderation rule.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteAutoModerationRule {
    client: Client,
    guild_id: GuildId,
    rule_id: AutoModerationRuleId,
    reason: Option<String>,
}

impl DeleteAutoModerationRule {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for DeleteAutoModerationRule {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/guilds/{guild_id}/auto-moderation/rules/{rule_id}",
            format!(
                "/guilds/{}/auto-moderation/rules/{}",
                self.guild_id, self.rule_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

super::into_future!(
    GetAutoModerationRules,
    GetAutoModerationRule,
    CreateAutoModerationRule,
    EditAutoModerationRule,
    DeleteAutoModerationRule,
);

impl Client {
    /// Gets the auto moderation rules of a guild. Needs `MANAGE_GUILD`.
    pub fn auto_moderation_rules(&self, guild_id: GuildId) -> GetAutoModerationRules {
        GetAutoModerationRules {
            client: self.clone(),
            guild_id,
        }
    }

    /// Gets an auto moderation rule. Needs `MANAGE_GUILD`.
    pub fn auto_moderation_rule(
        &self,
        guild_id: GuildId,
        rule_id: AutoModerationRuleId,
    ) -> GetAutoModerationRule {
        GetAutoModerationRule {
            client: self.clone(),
            guild_id,
            rule_id,
        }
    }

    /// Makes an auto moderation rule. Needs `MANAGE_GUILD`.
    ///
    /// The rule is off until [`CreateAutoModerationRule::enabled`] turns it on.
    pub fn create_auto_moderation_rule(
        &self,
        guild_id: GuildId,
        name: impl Into<String>,
        event_type: AutoModerationEventType,
        trigger_type: AutoModerationTriggerType,
    ) -> CreateAutoModerationRule {
        CreateAutoModerationRule {
            client: self.clone(),
            guild_id,
            body: RuleBody {
                name: Some(name.into()),
                event_type: Some(event_type),
                trigger_type: Some(trigger_type),
                actions: Some(Vec::new()),
                ..RuleBody::default()
            },
            reason: None,
        }
    }

    /// Changes an auto moderation rule. Needs `MANAGE_GUILD`.
    pub fn edit_auto_moderation_rule(
        &self,
        guild_id: GuildId,
        rule_id: AutoModerationRuleId,
    ) -> EditAutoModerationRule {
        EditAutoModerationRule {
            client: self.clone(),
            guild_id,
            rule_id,
            body: RuleBody::default(),
            reason: None,
        }
    }

    /// Deletes an auto moderation rule. Needs `MANAGE_GUILD`.
    pub fn delete_auto_moderation_rule(
        &self,
        guild_id: GuildId,
        rule_id: AutoModerationRuleId,
    ) -> DeleteAutoModerationRule {
        DeleteAutoModerationRule {
            client: self.clone(),
            guild_id,
            rule_id,
            reason: None,
        }
    }
}
