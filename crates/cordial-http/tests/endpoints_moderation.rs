//! Tests of the audit log and auto moderation endpoints.

mod common;

use common::{GUILD, assert_reason, setup_empty, setup_json, single_request, user_json};
use cordial_http::Error;
use cordial_model::audit_log::AuditLogEvent;
use cordial_model::auto_moderation::{
    AutoModerationAction, AutoModerationEventType, AutoModerationTriggerMetadata,
    AutoModerationTriggerType,
};
use cordial_model::id::{
    AuditLogEntryId, AutoModerationRuleId, ChannelId, GenericId, RoleId, UserId,
};
use serde_json::json;

const RULE: AutoModerationRuleId = AutoModerationRuleId::new_const(969_707_018_069_872_670);

#[tokio::test]
async fn get_guild_audit_log_with_all_filters() {
    let (client, transport) = setup_json(&json!({
        "application_commands": [], "auto_moderation_rules": [], "guild_scheduled_events": [],
        "integrations": [], "threads": [], "webhooks": [],
        "users": [user_json("53908099506183680")],
        "audit_log_entries": [{
            "id": "1101", "action_type": 22, "user_id": "53908099506183680",
            "target_id": "80351110224678912", "reason": "spam", "changes": []
        }]
    }));

    let log = client
        .guild_audit_log(GUILD)
        .user_id(UserId::new_const(53_908_099_506_183_680))
        .target_id(GenericId::new_const(80_351_110_224_678_912))
        .action_type(AuditLogEvent::MemberBanAdd)
        .before(AuditLogEntryId::new_const(2000))
        .limit(200)
        .await
        .unwrap();

    single_request(
        &transport,
        "GET",
        "/guilds/100/audit-logs?user_id=53908099506183680&target_id=80351110224678912&action_type=22&before=2000&limit=100",
    );
    assert_eq!(
        log.audit_log_entries[0].action_type,
        AuditLogEvent::MemberBanAdd
    );
    assert_eq!(log.users.len(), 1);
}

#[tokio::test]
async fn get_guild_audit_log_after() {
    let (client, transport) = setup_json(&json!({"audit_log_entries": []}));

    client
        .guild_audit_log(GUILD)
        .after(AuditLogEntryId::new_const(5))
        .await
        .unwrap();

    single_request(&transport, "GET", "/guilds/100/audit-logs?after=5");
}

/// The example rule from the docs.
fn rule_json() -> serde_json::Value {
    json!({
        "id": "969707018069872670",
        "guild_id": "613425648685547541",
        "name": "Keyword Filter 1",
        "creator_id": "423457898095789043",
        "trigger_type": 1,
        "event_type": 1,
        "actions": [
            {"type": 1, "metadata": {"custom_message": "Please keep financial discussions limited to the #finance channel"}},
            {"type": 2, "metadata": {"channel_id": "123456789123456789"}},
            {"type": 3, "metadata": {"duration_seconds": 60}}
        ],
        "trigger_metadata": {
            "keyword_filter": ["cat*", "*dog", "*ana*", "i like c++"],
            "regex_patterns": ["(b|c)at", "^(?:[0-9]{1,3}\\.){3}[0-9]{1,3}$"]
        },
        "enabled": true,
        "exempt_roles": ["323456789123456789", "423456789123456789"],
        "exempt_channels": ["523456789123456789"]
    })
}

#[tokio::test]
async fn list_auto_moderation_rules() {
    let (client, transport) = setup_json(&json!([rule_json()]));

    let rules = client.auto_moderation_rules(GUILD).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/auto-moderation/rules");
    assert_eq!(rules[0].actions.len(), 3);
    assert_eq!(rules[0].exempt_roles.len(), 2);
}

#[tokio::test]
async fn get_auto_moderation_rule() {
    let (client, transport) = setup_json(&rule_json());

    let rule = client.auto_moderation_rule(GUILD, RULE).await.unwrap();

    single_request(
        &transport,
        "GET",
        "/guilds/100/auto-moderation/rules/969707018069872670",
    );
    assert_eq!(rule.name, "Keyword Filter 1");
    assert_eq!(
        rule.trigger_metadata.keyword_filter.as_ref().unwrap().len(),
        4
    );
}

fn keyword_metadata(keywords: Vec<String>) -> AutoModerationTriggerMetadata {
    let mut metadata = AutoModerationTriggerMetadata::default();
    metadata.keyword_filter = Some(keywords);
    metadata
}

#[tokio::test]
async fn create_auto_moderation_rule() {
    let (client, transport) = setup_json(&rule_json());

    let rule = client
        .create_auto_moderation_rule(
            GUILD,
            "Keyword Filter 1",
            AutoModerationEventType::MessageSend,
            AutoModerationTriggerType::Keyword,
        )
        .trigger_metadata(keyword_metadata(vec!["cat*".to_owned()]))
        .actions(vec![
            AutoModerationAction::block_message(None),
            AutoModerationAction::send_alert(ChannelId::new_const(12)),
            AutoModerationAction::timeout(60),
        ])
        .enabled(true)
        .exempt_roles(vec![RoleId::new_const(3)])
        .exempt_channels(vec![ChannelId::new_const(4)])
        .reason("filter")
        .await
        .unwrap();

    let request = single_request(&transport, "POST", "/guilds/100/auto-moderation/rules");
    assert_reason(&request, "filter");
    assert_eq!(
        request.json(),
        json!({
            "name": "Keyword Filter 1", "event_type": 1, "trigger_type": 1,
            "trigger_metadata": {"keyword_filter": ["cat*"]},
            "actions": [
                {"type": 1},
                {"type": 2, "metadata": {"channel_id": "12"}},
                {"type": 3, "metadata": {"duration_seconds": 60}}
            ],
            "enabled": true, "exempt_roles": ["3"], "exempt_channels": ["4"]
        })
    );
    assert_eq!(rule.id, RULE);
}

#[tokio::test]
async fn create_auto_moderation_rule_sends_empty_actions() {
    let (client, transport) = setup_json(&rule_json());

    client
        .create_auto_moderation_rule(
            GUILD,
            "Spam",
            AutoModerationEventType::MessageSend,
            AutoModerationTriggerType::Spam,
        )
        .await
        .unwrap();

    let request = single_request(&transport, "POST", "/guilds/100/auto-moderation/rules");
    assert_eq!(
        request.json(),
        json!({"name": "Spam", "event_type": 1, "trigger_type": 3, "actions": []})
    );
}

#[tokio::test]
async fn create_auto_moderation_rule_refuses_long_keyword() {
    let (client, transport) = setup_json(&rule_json());

    let result = client
        .create_auto_moderation_rule(
            GUILD,
            "Filter",
            AutoModerationEventType::MessageSend,
            AutoModerationTriggerType::Keyword,
        )
        .trigger_metadata(keyword_metadata(vec!["a".repeat(61)]))
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn edit_auto_moderation_rule() {
    let (client, transport) = setup_json(&rule_json());

    client
        .edit_auto_moderation_rule(GUILD, RULE)
        .name("Renamed")
        .event_type(AutoModerationEventType::MemberUpdate)
        .enabled(false)
        .reason("pause")
        .await
        .unwrap();

    let request = single_request(
        &transport,
        "PATCH",
        "/guilds/100/auto-moderation/rules/969707018069872670",
    );
    assert_reason(&request, "pause");
    assert_eq!(
        request.json(),
        json!({"name": "Renamed", "event_type": 2, "enabled": false})
    );
}

#[tokio::test]
async fn edit_auto_moderation_rule_refuses_too_many_exempt_roles() {
    let (client, transport) = setup_json(&rule_json());
    let roles = (1..=21).map(RoleId::new_const).collect();

    let result = client
        .edit_auto_moderation_rule(GUILD, RULE)
        .exempt_roles(roles)
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn delete_auto_moderation_rule() {
    let (client, transport) = setup_empty();

    client
        .delete_auto_moderation_rule(GUILD, RULE)
        .reason("unused")
        .await
        .unwrap();

    let request = single_request(
        &transport,
        "DELETE",
        "/guilds/100/auto-moderation/rules/969707018069872670",
    );
    assert_reason(&request, "unused");
}
