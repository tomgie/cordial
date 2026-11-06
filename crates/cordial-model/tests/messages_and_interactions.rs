//! Fixture tests for messages, components, interactions, and commands.

mod common;

use std::path::PathBuf;

use common::{assert_round_trip, assert_value_round_trip, read_fixture};
use cordial_model::command::CommandDefinition;
use cordial_model::interaction::{
    CommandData, Interaction, InteractionMessage, InteractionResponse,
};
use cordial_model::message::{AllowedMentions, Message, SharedClientTheme};

fn fixture_names(directory: &str) -> Vec<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/docs")
        .join(directory);
    let mut names: Vec<String> = std::fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .map(|path| {
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            format!("{directory}/{stem}")
        })
        .collect();
    names.sort();
    names
}

#[test]
fn messages() {
    let message: Message = assert_round_trip("resources/message/example-message");
    assert!(message.reactions.is_some());

    let crossposted: Message = assert_round_trip("resources/message/example-crossposted-message");
    assert!(crossposted.message_reference.is_some());

    let theme = read_fixture("resources/message/example-shared-client-theme");
    assert_value_round_trip::<SharedClientTheme>(
        "shared-client-theme",
        &theme["shared_client_theme"],
    );
}

#[test]
fn allowed_mentions() {
    for name in fixture_names("resources/message") {
        let is_mentions = name.contains("allowed-mentions");
        if !is_mentions {
            continue;
        }
        // Each example has the shape `{"allowed_mentions": {...}}`, sometimes with content.
        let value = read_fixture(&name);
        if let Some(mentions) = value.get("allowed_mentions") {
            assert_value_round_trip::<AllowedMentions>(&name, mentions);
        }
    }
}

#[test]
fn component_examples() {
    let directories = [
        "components/reference",
        "components/using-message-components",
        "components/using-modal-components",
    ];
    for directory in directories {
        for name in fixture_names(directory) {
            let value = read_fixture(&name);
            let is_response = value.get("data").is_some() && value.get("type").is_some();
            if is_response {
                assert_value_round_trip::<InteractionResponse>(&name, &value);
            } else {
                assert_value_round_trip::<InteractionMessage>(&name, &value);
            }
        }
    }
}

#[test]
fn command_examples() {
    for name in fixture_names("interactions/application-commands") {
        let mut value = read_fixture(&name);
        if name.ends_with("/autocomplete") {
            // A partial autocomplete interaction. `data.version` is not in the docs table.
            let data = value["data"].as_object_mut().unwrap();
            data.remove("version");
            assert_value_round_trip::<CommandData>(&name, &value["data"]);
        } else if value.get("token").is_some() {
            // One docs example has no `application_id`. Real interactions always have it.
            let object = value.as_object_mut().unwrap();
            object.entry("application_id").or_insert_with(|| "1".into());
            assert_value_round_trip::<Interaction>(&name, &value);
        } else if value.get("name").is_some() {
            assert_value_round_trip::<CommandDefinition>(&name, &value);
        } else if value.get("data").is_some() {
            assert_value_round_trip::<InteractionResponse>(&name, &value);
        }
    }
}
