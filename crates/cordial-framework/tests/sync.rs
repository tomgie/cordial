//! Tests of the command sync: the plan and the requests that the sync sends.

use cordial_framework::sync::{
    Action, PlanError, SyncCommand, SyncError, SyncOptions, SyncOutcome, SyncScope, SyncStrategy,
    plan, sync,
};
use cordial_http::testing::{FakeResponse, FakeTransport, RecordedRequest};
use cordial_http::{Client, Token};
use cordial_model::command::{
    Command, CommandDefinition, CommandOption, CommandOptionType, CommandType,
};
use cordial_model::id::{ApplicationId, CommandId, GuildId};
use cordial_model::{Locale, Patch};
use serde_json::{Value, json};

const BASE: &str = "https://discord.com/api/v10";
const APPLICATION: ApplicationId = ApplicationId::new_const(172_150_183_260_323_840);
const GUILD: GuildId = GuildId::new_const(613_425_648_685_547_541);

// Helpers.

/// Makes a remote command as Discord gives it from a list with localizations.
fn remote_json(id: u64, kind: u8, name: &str, description: &str) -> Value {
    json!({
        "id": id.to_string(),
        "application_id": APPLICATION.to_string(),
        "version": "1239456789012345678",
        "default_member_permissions": null,
        "type": kind,
        "name": name,
        "name_localizations": null,
        "description": description,
        "description_localizations": null,
        "dm_permission": true,
        "contexts": null,
        "integration_types": [0],
        "nsfw": false,
    })
}

/// The `blep` command of the Discord documentation, as Discord gives it back.
fn blep_json() -> Value {
    let mut command = remote_json(1001, 1, "blep", "Send a random adorable animal photo");
    command["options"] = json!([
        {
            "type": 3,
            "name": "animal",
            "name_localizations": null,
            "description": "The type of animal",
            "description_localizations": null,
            "required": true,
            "choices": [
                {"name": "Dog", "name_localizations": null, "value": "animal_dog"},
                {"name": "Cat", "name_localizations": null, "value": "animal_cat"},
                {"name": "Penguin", "name_localizations": null, "value": "animal_penguin"}
            ]
        },
        {
            "type": 5,
            "name": "only_smol",
            "name_localizations": null,
            "description": "Whether to show only baby animals",
            "description_localizations": null,
            "required": false,
            "channel_types": []
        }
    ]);
    command
}

fn animal_option() -> CommandOption {
    CommandOption::new(CommandOptionType::String, "animal", "The type of animal")
        .required(true)
        .choice("Dog", "animal_dog")
        .choice("Cat", "animal_cat")
        .choice("Penguin", "animal_penguin")
}

fn smol_option() -> CommandOption {
    CommandOption::new(
        CommandOptionType::Boolean,
        "only_smol",
        "Whether to show only baby animals",
    )
}

/// The local definition of the `blep` command.
fn blep() -> CommandDefinition {
    CommandDefinition::chat_input("blep", "Send a random adorable animal photo")
        .option(animal_option())
        .option(smol_option())
}

fn command(value: Value) -> Command {
    serde_json::from_value(value).expect("the remote command is valid")
}

fn local(definitions: impl IntoIterator<Item = CommandDefinition>) -> Vec<SyncCommand> {
    definitions.into_iter().map(SyncCommand::new).collect()
}

fn plan_one(definition: CommandDefinition, remote: Value) -> Action {
    let plan = plan(
        &local([definition]),
        &[command(remote)],
        &SyncOptions::new(),
    )
    .unwrap();
    assert_eq!(plan.actions().len(), 1);
    plan.actions()[0].clone()
}

fn id(value: u64) -> CommandId {
    CommandId::new_const(value)
}

fn client(transport: &FakeTransport) -> Client {
    Client::builder()
        .token(Token::bot("abc"))
        .transport(transport.clone())
        .build()
}

fn commands_path() -> String {
    format!("{BASE}/applications/{APPLICATION}/commands")
}

fn command_path(command_id: u64) -> String {
    format!("{BASE}/applications/{APPLICATION}/commands/{command_id}")
}

fn list_path() -> String {
    format!("{}?with_localizations=true", commands_path())
}

/// Gives the method and the URL of each request.
fn routes(transport: &FakeTransport) -> Vec<(String, String)> {
    transport
        .requests()
        .iter()
        .map(|request: &RecordedRequest| (request.method.clone(), request.url.clone()))
        .collect()
}

fn route(method: &str, url: String) -> (String, String) {
    (method.to_owned(), url)
}

/// Makes a transport that gives the remote commands to the first request.
fn transport_with(remote: &[Value]) -> FakeTransport {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &Value::from(remote.to_vec())));
    transport
}

// Normalization.

#[test]
fn identical_command_is_unchanged() {
    let action = plan_one(blep(), blep_json());

    assert_eq!(
        action,
        Action::Unchanged {
            id: id(1001),
            kind: CommandType::ChatInput,
            name: "blep".to_owned(),
        }
    );
}

#[test]
fn absent_null_and_empty_values_are_equal() {
    let mut remote = blep_json();
    remote["name_localizations"] = json!({});
    remote["options"][0]["description_localizations"] = json!({});
    remote["options"][1]["choices"] = json!([]);
    remote["default_member_permissions"] = Value::Null;

    let mut definition = blep();
    definition.description_localizations = Some(Default::default());
    definition.default_member_permissions = Patch::Clear;

    assert!(!plan_one(definition, remote).is_change());
}

#[test]
fn local_without_type_matches_remote_chat_input() {
    let definition: CommandDefinition = serde_json::from_value(json!({
        "name": "blep",
        "description": "Send a random adorable animal photo",
        "options": [
            {
                "type": 3,
                "name": "animal",
                "description": "The type of animal",
                "required": true,
                "choices": [
                    {"name": "Dog", "value": "animal_dog"},
                    {"name": "Cat", "value": "animal_cat"},
                    {"name": "Penguin", "value": "animal_penguin"}
                ]
            },
            {"type": 5, "name": "only_smol", "description": "Whether to show only baby animals"}
        ]
    }))
    .unwrap();

    assert!(!plan_one(definition, blep_json()).is_change());
}

#[test]
fn remote_without_optional_fields_matches() {
    let remote = json!({
        "id": "1001",
        "application_id": APPLICATION.to_string(),
        "version": "1",
        "name": "ping",
        "description": "Check the bot",
    });

    let action = plan_one(
        CommandDefinition::chat_input("ping", "Check the bot"),
        remote,
    );

    assert!(!action.is_change());
}

#[test]
fn user_command_with_empty_description_is_unchanged() {
    let remote = remote_json(1002, 2, "High Five", "");

    let action = plan_one(CommandDefinition::user("High Five"), remote);

    assert!(!action.is_change());
}

#[test]
fn integer_limits_match_in_both_number_forms() {
    let mut remote = remote_json(1003, 1, "roll", "Roll a die");
    remote["options"] = json!([
        {"type": 10, "name": "sides", "description": "The sides", "min_value": 2.0, "max_value": 100}
    ]);
    let mut option = CommandOption::new(CommandOptionType::Number, "sides", "The sides");
    option.min_value = Some(cordial_model::command::NumberLimit::Integer(2));
    option.max_value = Some(cordial_model::command::NumberLimit::Number(100.0));
    let definition = CommandDefinition::chat_input("roll", "Roll a die").option(option);

    assert!(!plan_one(definition, remote).is_change());
}

#[test]
fn changed_description_gives_update() {
    let definition = CommandDefinition::chat_input("blep", "A new description")
        .option(animal_option())
        .option(smol_option());

    let action = plan_one(definition, blep_json());

    let Action::Update {
        id: command_id,
        definition,
    } = action
    else {
        panic!("expected an update, got {action:?}");
    };
    assert_eq!(command_id, id(1001));
    assert_eq!(definition.description, "A new description");
    assert_eq!(definition.id, None);
}

#[test]
fn changed_option_order_gives_update() {
    let definition = CommandDefinition::chat_input("blep", "Send a random adorable animal photo")
        .option(smol_option())
        .option(animal_option());

    assert!(matches!(
        plan_one(definition, blep_json()),
        Action::Update { .. }
    ));
}

#[test]
fn changed_choice_order_gives_update() {
    let animal = CommandOption::new(CommandOptionType::String, "animal", "The type of animal")
        .required(true)
        .choice("Cat", "animal_cat")
        .choice("Dog", "animal_dog")
        .choice("Penguin", "animal_penguin");
    let definition = CommandDefinition::chat_input("blep", "Send a random adorable animal photo")
        .option(animal)
        .option(smol_option());

    assert!(matches!(
        plan_one(definition, blep_json()),
        Action::Update { .. }
    ));
}

#[test]
fn different_localizations_give_update() {
    let mut remote = blep_json();
    remote["name_localizations"] = json!({"de": "blep"});
    remote["description_localizations"] = json!({"de": "Ein Tierfoto"});
    let definition = blep().localize(Locale::German, "blep", "Ein anderes Tierfoto");

    assert!(matches!(
        plan_one(definition, remote),
        Action::Update { .. }
    ));
}

#[test]
fn same_localizations_are_unchanged() {
    let mut remote = blep_json();
    remote["name_localizations"] = json!({"de": "blep", "fr": "blep"});
    remote["description_localizations"] = json!({"fr": "Une photo", "de": "Ein Tierfoto"});
    let definition = blep()
        .localize(Locale::French, "blep", "Une photo")
        .localize(Locale::German, "blep", "Ein Tierfoto");

    assert!(!plan_one(definition, remote).is_change());
}

#[test]
fn update_resets_removed_localizations_and_permissions() {
    let mut remote = blep_json();
    remote["name_localizations"] = json!({"de": "blep"});
    remote["default_member_permissions"] = json!("8");

    let action = plan_one(blep(), remote);

    let Action::Update { definition, .. } = action else {
        panic!("expected an update, got {action:?}");
    };
    assert_eq!(definition.name_localizations, Some(Default::default()));
    assert_eq!(definition.default_member_permissions, Patch::Clear);
}

#[test]
fn deprecated_remote_fields_are_ignored() {
    let mut remote = blep_json();
    remote["dm_permission"] = json!(false);
    remote["default_permission"] = json!(true);

    assert!(!plan_one(blep(), remote).is_change());
}

// Matching and actions.

#[test]
fn rename_uses_previous_name() {
    let commands = [SyncCommand::new(blep()).previous_name("blip")];
    let mut remote = blep_json();
    remote["name"] = json!("blip");

    let plan = plan(&commands, &[command(remote)], &SyncOptions::new()).unwrap();

    let Action::Rename {
        id: command_id,
        from,
        definition,
    } = &plan.actions()[0]
    else {
        panic!("expected a rename, got {:?}", plan.actions());
    };
    assert_eq!(*command_id, id(1001));
    assert_eq!(from, "blip");
    assert_eq!(definition.name, "blep");
    assert_eq!(plan.renamed(), 1);
    assert_eq!(plan.actions().len(), 1);
}

#[test]
fn current_name_wins_over_previous_name() {
    let commands = [SyncCommand::new(blep()).previous_name("blip")];
    let mut old = blep_json();
    old["id"] = json!("1002");
    old["name"] = json!("blip");

    let plan = plan(
        &commands,
        &[command(old), command(blep_json())],
        &SyncOptions::new(),
    )
    .unwrap();

    assert_eq!(plan.unchanged(), 1);
    assert_eq!(plan.deleted(), 1);
    assert_eq!(plan.renamed(), 0);
}

#[test]
fn previous_name_matches_only_the_same_type() {
    let commands = [SyncCommand::new(CommandDefinition::user("Wave")).previous_name("Hello")];
    let remote = command(remote_json(1004, 3, "Hello", ""));

    let plan = plan(&commands, &[remote], &SyncOptions::new()).unwrap();

    assert_eq!(plan.created(), 1);
    assert_eq!(plan.deleted(), 1);
}

#[test]
fn new_command_gives_create_and_missing_command_gives_delete() {
    let commands = local([CommandDefinition::chat_input("ping", "Check the bot")]);
    let remote = command(blep_json());

    let plan = plan(&commands, &[remote], &SyncOptions::new()).unwrap();

    assert_eq!(
        plan.actions(),
        [
            Action::Delete {
                id: id(1001),
                kind: CommandType::ChatInput,
                name: "blep".to_owned(),
            },
            Action::Create {
                definition: CommandDefinition::chat_input("ping", "Check the bot"),
            },
        ]
    );
}

#[test]
fn same_name_with_other_type_is_another_command() {
    let commands = local([CommandDefinition::message("blep")]);
    let remote = command(blep_json());

    let plan = plan(&commands, &[remote], &SyncOptions::new()).unwrap();

    assert_eq!(plan.created(), 1);
    assert_eq!(plan.deleted(), 1);
}

#[test]
fn entry_point_command_is_kept() {
    let mut entry_point = remote_json(1005, 4, "launch", "Launch Racing with Friends");
    entry_point["handler"] = json!(2);
    entry_point["integration_types"] = json!([0, 1]);
    entry_point["contexts"] = json!([0, 1, 2]);
    let remote = [command(entry_point)];

    let kept = plan(&[], &remote, &SyncOptions::new()).unwrap();
    let deleted = plan(&[], &remote, &SyncOptions::new().delete_entry_point(true)).unwrap();

    assert_eq!(kept.unchanged(), 1);
    assert!(!kept.has_changes());
    assert_eq!(kept.bulk_definitions().len(), 1);
    assert_eq!(kept.bulk_definitions()[0].id, Some(id(1005)));
    assert_eq!(deleted.deleted(), 1);
    assert!(deleted.bulk_definitions().is_empty());
}

#[test]
fn local_entry_point_without_handler_matches_remote() {
    let mut entry_point = remote_json(1005, 4, "launch", "Launch Racing with Friends");
    entry_point["handler"] = json!(2);
    let mut definition = CommandDefinition::chat_input("launch", "Launch Racing with Friends");
    definition.kind = CommandType::PrimaryEntryPoint;

    assert!(!plan_one(definition, entry_point).is_change());
}

#[test]
fn duplicate_local_commands_give_error() {
    let commands = local([
        CommandDefinition::chat_input("ping", "One"),
        CommandDefinition::chat_input("ping", "Two"),
    ]);

    let error = plan(&commands, &[], &SyncOptions::new()).unwrap_err();

    assert_eq!(
        error,
        PlanError::DuplicateCommand {
            kind: CommandType::ChatInput,
            name: "ping".to_owned(),
        }
    );
}

#[test]
fn plan_orders_deletes_renames_updates_creates() {
    let commands = [
        SyncCommand::new(CommandDefinition::chat_input("new", "A new command")),
        SyncCommand::new(CommandDefinition::chat_input("changed", "A new text")),
        SyncCommand::new(CommandDefinition::chat_input("renamed", "Same")).previous_name("old"),
        SyncCommand::new(CommandDefinition::chat_input("same", "Same")),
    ];
    let remote = [
        command(remote_json(1, 1, "same", "Same")),
        command(remote_json(2, 1, "old", "Same")),
        command(remote_json(3, 1, "changed", "The old text")),
        command(remote_json(4, 1, "gone", "Gone")),
    ];

    let plan = plan(&commands, &remote, &SyncOptions::new()).unwrap();

    let order: Vec<String> = plan.actions().iter().map(ToString::to_string).collect();
    assert_eq!(
        order,
        [
            "delete slash command `gone`",
            "rename slash command `old` to `renamed`",
            "update slash command `changed`",
            "create slash command `new`",
            "keep slash command `same`",
        ]
    );
    assert_eq!(
        plan.to_string(),
        "created 1, updated 1, renamed 1, deleted 1, unchanged 1"
    );
    assert_eq!(plan.change_count(), 4);
}

// Requests.

fn changes_scenario() -> (Vec<SyncCommand>, Vec<Value>) {
    let commands = vec![
        SyncCommand::new(CommandDefinition::chat_input("new", "A new command")),
        SyncCommand::new(CommandDefinition::chat_input("changed", "A new text")),
        SyncCommand::new(CommandDefinition::chat_input("renamed", "Same")).previous_name("old"),
        SyncCommand::new(CommandDefinition::chat_input("same", "Same")),
    ];
    let remote = vec![
        remote_json(1, 1, "same", "Same"),
        remote_json(2, 1, "old", "Same"),
        remote_json(3, 1, "changed", "The old text"),
        remote_json(4, 1, "gone", "Gone"),
    ];
    (commands, remote)
}

#[tokio::test]
async fn dry_run_sends_no_change() {
    let (commands, remote) = changes_scenario();
    let transport = transport_with(&remote);

    let report = sync(
        &client(&transport),
        APPLICATION,
        SyncScope::Global,
        &commands,
        &SyncOptions::new().dry_run(true),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, SyncOutcome::DryRun);
    assert_eq!(report.plan.change_count(), 4);
    assert_eq!(routes(&transport), [route("GET", list_path())]);
}

#[tokio::test]
async fn no_changes_send_nothing() {
    let transport = transport_with(&[blep_json()]);

    let report = sync(
        &client(&transport),
        APPLICATION,
        SyncScope::Global,
        &local([blep()]),
        &SyncOptions::new().strategy(SyncStrategy::Bulk),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, SyncOutcome::NoChanges);
    assert_eq!(routes(&transport), [route("GET", list_path())]);
}

#[tokio::test]
async fn granular_sends_one_request_for_each_change() {
    let (commands, remote) = changes_scenario();
    let transport = transport_with(&remote);
    transport.push(FakeResponse::empty(204));
    transport.push(FakeResponse::json(
        200,
        &remote_json(2, 1, "renamed", "Same"),
    ));
    transport.push(FakeResponse::json(
        200,
        &remote_json(3, 1, "changed", "A new text"),
    ));
    transport.push(FakeResponse::json(
        201,
        &remote_json(5, 1, "new", "A new command"),
    ));

    let report = sync(
        &client(&transport),
        APPLICATION,
        SyncScope::Global,
        &commands,
        &SyncOptions::new(),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, SyncOutcome::Granular);
    assert_eq!(
        routes(&transport),
        [
            route("GET", list_path()),
            route("DELETE", command_path(4)),
            route("PATCH", command_path(2)),
            route("PATCH", command_path(3)),
            route("POST", commands_path()),
        ]
    );

    let requests = transport.requests();
    let rename = requests[2].json();
    assert_eq!(rename["name"], "renamed");
    assert_eq!(rename["default_member_permissions"], Value::Null);
    assert!(rename.get("type").is_none());
    assert_eq!(requests[4].json()["name"], "new");
}

#[tokio::test]
async fn granular_uses_guild_routes() {
    let transport = transport_with(&[remote_json(4, 1, "gone", "Gone")]);
    transport.push(FakeResponse::empty(204));
    transport.push(FakeResponse::json(201, &remote_json(5, 1, "ping", "Pong")));

    sync(
        &client(&transport),
        APPLICATION,
        SyncScope::Guild(GUILD),
        &local([CommandDefinition::chat_input("ping", "Pong")]),
        &SyncOptions::new(),
    )
    .await
    .unwrap();

    let guild_commands = format!("{BASE}/applications/{APPLICATION}/guilds/{GUILD}/commands");
    assert_eq!(
        routes(&transport),
        [
            route("GET", format!("{guild_commands}?with_localizations=true")),
            route("DELETE", format!("{guild_commands}/4")),
            route("POST", guild_commands),
        ]
    );
}

#[tokio::test]
async fn bulk_sends_one_put() {
    let (commands, remote) = changes_scenario();
    let mut entry_point = remote_json(6, 4, "launch", "Launch");
    entry_point["handler"] = json!(2);
    let mut remote = remote;
    remote.push(entry_point);
    let transport = transport_with(&remote);
    transport.push(FakeResponse::json(200, &json!([])));

    let report = sync(
        &client(&transport),
        APPLICATION,
        SyncScope::Global,
        &commands,
        &SyncOptions::new().strategy(SyncStrategy::Bulk),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, SyncOutcome::Bulk);
    assert_eq!(
        routes(&transport),
        [route("GET", list_path()), route("PUT", commands_path())]
    );

    let body = transport.requests()[1].json();
    let sent: Vec<(Option<&str>, &str)> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|item| (item["id"].as_str(), item["name"].as_str().unwrap()))
        .collect();
    assert_eq!(
        sent,
        [
            (None, "new"),
            (Some("3"), "changed"),
            (Some("2"), "renamed"),
            (Some("1"), "same"),
            (Some("6"), "launch"),
        ]
    );
}

#[tokio::test]
async fn auto_uses_bulk_above_threshold() {
    let (commands, remote) = changes_scenario();
    let transport = transport_with(&remote);
    transport.push(FakeResponse::json(200, &json!([])));

    let report = sync(
        &client(&transport),
        APPLICATION,
        SyncScope::Global,
        &commands,
        &SyncOptions::new().strategy(SyncStrategy::Auto { threshold: 3 }),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, SyncOutcome::Bulk);
    assert_eq!(transport.requests().len(), 2);
}

#[tokio::test]
async fn auto_uses_granular_at_threshold() {
    let transport = transport_with(&[remote_json(4, 1, "gone", "Gone")]);
    transport.push(FakeResponse::empty(204));

    let report = sync(
        &client(&transport),
        APPLICATION,
        SyncScope::Global,
        &[],
        &SyncOptions::new().strategy(SyncStrategy::Auto { threshold: 1 }),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, SyncOutcome::Granular);
    assert_eq!(
        routes(&transport),
        [route("GET", list_path()), route("DELETE", command_path(4))]
    );
}

#[tokio::test]
async fn failure_stops_and_reports_applied_actions() {
    let (commands, remote) = changes_scenario();
    let transport = transport_with(&remote);
    transport.push(FakeResponse::empty(204));
    transport.push(FakeResponse::json(
        400,
        &json!({"code": 50035, "message": "Invalid Form Body"}),
    ));

    let error = sync(
        &client(&transport),
        APPLICATION,
        SyncScope::Global,
        &commands,
        &SyncOptions::new(),
    )
    .await
    .unwrap_err();

    let SyncError::Granular {
        applied, failed, ..
    } = error
    else {
        panic!("expected a granular error, got {error:?}");
    };
    assert_eq!(
        applied,
        [Action::Delete {
            id: id(4),
            kind: CommandType::ChatInput,
            name: "gone".to_owned(),
        }]
    );
    assert!(matches!(*failed, Action::Rename { .. }));
    assert_eq!(transport.requests().len(), 3);
}

#[tokio::test]
async fn failed_fetch_sends_no_change() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(
        401,
        &json!({"code": 0, "message": "401: Unauthorized"}),
    ));

    let error = sync(
        &client(&transport),
        APPLICATION,
        SyncScope::Global,
        &local([blep()]),
        &SyncOptions::new(),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, SyncError::Fetch(_)));
    assert_eq!(transport.requests().len(), 1);
}
