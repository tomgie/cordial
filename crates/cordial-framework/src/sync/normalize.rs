//! The comparison of a local definition with a remote command.
//!
//! Both sides become JSON values in one normal form. Two commands are equal when their normal
//! forms are equal.

use cordial_model::command::{Command, CommandDefinition};
use serde_json::{Map, Value};

/// Fields that only Discord sets.
const DISCORD_ONLY_FIELDS: [&str; 6] = [
    "id",
    "application_id",
    "guild_id",
    "version",
    "name_localized",
    "description_localized",
];

/// Deprecated fields. `CommandDefinition` cannot set them, so the comparison ignores them.
const DEPRECATED_FIELDS: [&str; 2] = ["dm_permission", "default_permission"];

/// Fields that the comparison uses only when the local definition sets them.
///
/// An edit request does not send an absent field, and Discord keeps the current value. Thus an
/// absent local value means "keep what Discord has", not a default value.
/// - `integration_types`: Discord uses the installation contexts of the application at the
///   time of creation. The command alone does not tell that value.
/// - `contexts`: the command object has no documented default.
/// - `handler`: Discord sets it on the Entry Point command that it makes for an activity.
const LOCAL_ONLY_FIELDS: [&str; 3] = ["integration_types", "contexts", "handler"];

/// Boolean fields where `false` is the default.
const FALSE_DEFAULT_FIELDS: [&str; 3] = ["nsfw", "required", "autocomplete"];

/// The default of `integration_types` in the bulk overwrite documentation: `[GUILD_INSTALL]`.
const DEFAULT_INTEGRATION_TYPES: [u8; 1] = [0];

/// The default of `contexts` in the bulk overwrite documentation: `[GUILD, BOT_DM,
/// PRIVATE_CHANNEL]`. Discord also gives `null` for old commands that work in all contexts.
const DEFAULT_CONTEXTS: [u8; 3] = [0, 1, 2];

/// Tells if a local definition and a remote command have the same content.
pub(super) fn is_same(local: &CommandDefinition, remote: &Command) -> bool {
    // The model types always encode, because all their map keys are strings.
    let local = serde_json::to_value(local).unwrap_or(Value::Null);
    let remote = serde_json::to_value(remote).unwrap_or(Value::Null);
    let (Value::Object(mut local), Value::Object(mut remote)) = (local, remote) else {
        return false;
    };

    for field in DISCORD_ONLY_FIELDS.iter().chain(&DEPRECATED_FIELDS) {
        local.remove(*field);
        remote.remove(*field);
    }

    for field in LOCAL_ONLY_FIELDS {
        if local.get(field).is_none_or(is_empty) {
            local.remove(field);
            remote.remove(field);
        }
    }

    fill_type(&mut local);
    fill_type(&mut remote);

    let list_defaults: [(&str, &[u8]); 2] = [
        ("integration_types", &DEFAULT_INTEGRATION_TYPES),
        ("contexts", &DEFAULT_CONTEXTS),
    ];
    // A remote command gives these lists as absent or null for the default.
    for (field, default) in list_defaults {
        if local.contains_key(field) {
            fill_list(&mut local, field, default);
            fill_list(&mut remote, field, default);
        }
    }

    let mut local = Value::Object(local);
    let mut remote = Value::Object(remote);
    clean(&mut local);
    clean(&mut remote);

    local == remote
}

/// Fills the documented default type: `CHAT_INPUT`.
fn fill_type(command: &mut Map<String, Value>) {
    if command.get("type").is_none_or(Value::is_null) {
        command.insert("type".to_owned(), Value::from(1));
    }
}

/// Fills an absent or empty list with its default, and sorts it. The order of these lists is
/// not visible to users.
fn fill_list(command: &mut Map<String, Value>, field: &str, default: &[u8]) {
    let value = command.entry(field).or_insert(Value::Null);

    if is_empty(value) {
        *value = Value::from(default.to_vec());
    }

    if let Value::Array(items) = value {
        items.sort_by(|left, right| number(left).total_cmp(&number(right)));
        items.dedup();
    }
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or(f64::NAN)
}

/// Removes default values from all levels, so that absent, null, and empty values are equal.
///
/// The order of `options` and `choices` stays the same, because users see that order.
fn clean(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for item in map.values_mut() {
                clean(item);
            }

            map.retain(|key, item| !is_default(key, item));
        }
        Value::Array(items) => {
            for item in items {
                clean(item);
            }
        }
        Value::Number(number) => {
            // A number limit or a choice value can come back as `5` or as `5.0`.
            if let Some(float) = number.as_f64() {
                *value = Value::from(float);
            }
        }
        Value::Null | Value::Bool(_) | Value::String(_) => {}
    }
}

fn is_default(key: &str, value: &Value) -> bool {
    if is_empty(value) {
        return true;
    }

    match value {
        Value::Bool(false) => FALSE_DEFAULT_FIELDS.contains(&key),
        // User and message commands have an empty description.
        Value::String(text) => key == "description" && text.is_empty(),
        _ => false,
    }
}

fn is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Array(items) => items.is_empty(),
        Value::Object(map) => map.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(value: Value) -> Command {
        serde_json::from_value(value).unwrap()
    }

    fn base_remote() -> Value {
        serde_json::json!({
            "id": "1",
            "application_id": "2",
            "version": "3",
            "type": 1,
            "name": "ping",
            "description": "Check the bot",
        })
    }

    #[test]
    fn number_forms_are_equal() {
        let mut integer = serde_json::json!({"min_value": 5});
        let mut float = serde_json::json!({"min_value": 5.0});
        clean(&mut integer);
        clean(&mut float);

        assert_eq!(integer, float);
    }

    #[test]
    fn absent_local_contexts_match_any_remote_contexts() {
        let local = CommandDefinition::chat_input("ping", "Check the bot");
        let mut value = base_remote();
        value["contexts"] = serde_json::json!([0]);

        assert!(is_same(&local, &remote(value)));
    }

    #[test]
    fn null_remote_contexts_equal_all_contexts() {
        use cordial_model::interaction::InteractionContextType;

        let local = CommandDefinition::chat_input("ping", "Check the bot").contexts([
            InteractionContextType::PrivateChannel,
            InteractionContextType::Guild,
            InteractionContextType::BotDm,
        ]);
        let mut value = base_remote();
        value["contexts"] = Value::Null;

        assert!(is_same(&local, &remote(value)));
    }

    #[test]
    fn absent_remote_integration_types_equal_guild_install() {
        use cordial_model::application::ApplicationIntegrationType;

        let local = CommandDefinition::chat_input("ping", "Check the bot")
            .integration_types([ApplicationIntegrationType::GuildInstall]);

        assert!(is_same(&local, &remote(base_remote())));
    }

    #[test]
    fn different_contexts_are_different() {
        use cordial_model::interaction::InteractionContextType;

        let local = CommandDefinition::chat_input("ping", "Check the bot")
            .contexts([InteractionContextType::Guild]);
        let mut value = base_remote();
        value["contexts"] = serde_json::json!([0, 1]);

        assert!(!is_same(&local, &remote(value)));
    }

    #[test]
    fn administrator_only_differs_from_everyone() {
        let local = CommandDefinition::chat_input("ping", "Check the bot")
            .default_member_permissions(cordial_model::Permissions::empty());
        let mut value = base_remote();
        value["default_member_permissions"] = Value::Null;

        assert!(!is_same(&local, &remote(value)));
    }
}
