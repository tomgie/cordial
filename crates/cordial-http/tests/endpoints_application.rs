//! Tests of the application, role connection metadata, and application command endpoints.

use std::collections::HashMap;

use cordial_http::request::FileUpload;
use cordial_http::testing::{FakeResponse, FakeTransport, RecordedRequest};
use cordial_http::{Client, Error, ImageData, Token};
use cordial_model::application::{
    ApplicationFlags, ApplicationIntegrationType, EventWebhookStatus, IntegrationTypeConfig,
    RoleConnectionMetadata,
};
use cordial_model::command::{
    CommandDefinition, CommandOption, CommandOptionType, CommandPermission,
};
use cordial_model::id::{ApplicationId, CommandId, GuildId};
use cordial_model::interaction::InteractionContextType;
use serde_json::{Value, json};

const APPLICATION: ApplicationId = ApplicationId::new_const(172_150_183_260_323_840);
const GUILD: GuildId = GuildId::new_const(290_926_798_626_357_260);
const COMMAND: CommandId = CommandId::new_const(1_000);
const BASE: &str = "https://discord.com/api/v10";

fn client(transport: &FakeTransport) -> Client {
    Client::builder()
        .token(Token::bot("abc"))
        .transport(transport.clone())
        .build()
}

fn only_request(transport: &FakeTransport) -> RecordedRequest {
    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    requests[0].clone()
}

/// The example application object of the Discord docs.
fn application_json() -> Value {
    json!({
        "bot_public": true,
        "bot_require_code_grant": false,
        "cover_image": "31deabb7e45b6c8ecfef77d2f99c81a5",
        "description": "Test",
        "flags": 8192,
        "flags_new": "8192",
        "guild_id": "290926798626357260",
        "icon": null,
        "id": "172150183260323840",
        "integration_types_config": {
            "0": {"oauth2_install_params": {"scopes": ["applications.commands", "bot"], "permissions": "2048"}},
            "1": {"oauth2_install_params": {"scopes": ["applications.commands"], "permissions": "0"}}
        },
        "name": "Baba O-Riley",
        "interactions_endpoint_url": null,
        "role_connections_verification_url": null,
        "event_webhooks_url": null,
        "event_webhooks_status": 1,
        "owner": {"avatar": null, "discriminator": "1738", "flags": 1024, "id": "172150183260323840", "username": "i own a bot"},
        "primary_sku_id": "172150183260323840",
        "slug": "test",
        "team": {
            "name": "A team",
            "icon": "dd9b7dcfdf5351b9c3de0fe167bacbe1",
            "id": "531992624043786253",
            "owner_user_id": "511972282709709995",
            "members": [{
                "membership_state": 2,
                "permissions": ["*"],
                "team_id": "531992624043786253",
                "user": {"avatar": "d9e261cd35999608eb7e3de1fae3688b", "discriminator": "0001", "id": "511972282709709995", "username": "Mr Owner"}
            }]
        },
        "verify_key": "1e0a356058d627ca38a5c8c9648818061d49e49bd9da9e3ab17d98ad4d6bg2u8"
    })
}

fn command_json() -> Value {
    json!({
        "id": "1000",
        "application_id": "172150183260323840",
        "version": "1001",
        "default_member_permissions": null,
        "type": 1,
        "name": "blep",
        "description": "Send a random adorable animal photo",
        "guild_id": "290926798626357260",
        "options": [{
            "type": 3,
            "name": "animal",
            "description": "The type of animal",
            "required": true,
            "choices": [{"name": "Dog", "value": "animal_dog"}, {"name": "Cat", "value": "animal_cat"}]
        }]
    })
}

fn permissions_json() -> Value {
    json!({
        "id": "1000",
        "application_id": "172150183260323840",
        "guild_id": "290926798626357260",
        "permissions": [{"id": "290926798626357260", "type": 1, "permission": false}]
    })
}

fn blep() -> CommandDefinition {
    CommandDefinition::chat_input("blep", "Send a random adorable animal photo").option(
        CommandOption::new(CommandOptionType::String, "animal", "The type of animal")
            .required(true)
            .choice("Dog", "animal_dog"),
    )
}

#[tokio::test]
async fn gets_current_application() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &application_json()));

    let application = client(&transport).current_application().await.unwrap();

    assert_eq!(application.id, APPLICATION);
    assert_eq!(application.name, "Baba O-Riley");
    assert_eq!(
        application.event_webhooks_status,
        Some(EventWebhookStatus::Disabled)
    );
    assert!(
        application
            .flags()
            .contains(ApplicationFlags::GATEWAY_PRESENCE_LIMITED)
    );
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, format!("{BASE}/applications/@me"));
}

#[tokio::test]
async fn gets_application_by_id() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &application_json()));

    let application = client(&transport).application(APPLICATION).await.unwrap();

    assert_eq!(application.slug.as_deref(), Some("test"));
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840")
    );
}

#[tokio::test]
async fn edits_current_application() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &application_json()));
    let mut config = HashMap::new();
    config.insert(
        ApplicationIntegrationType::UserInstall,
        IntegrationTypeConfig::default(),
    );

    let application = client(&transport)
        .edit_current_application()
        .description("Test")
        .flags(ApplicationFlags::GATEWAY_PRESENCE_LIMITED)
        .clear_icon()
        .cover_image(ImageData::from_data_uri("data:image/png;base64,AAAA"))
        .tags(["fun", "games"])
        .integration_types_config(config)
        .event_webhooks_url("https://example.com/events")
        .event_webhooks_status(EventWebhookStatus::Enabled)
        .event_webhooks_types(["ENTITLEMENT_CREATE"])
        .await
        .unwrap();

    assert_eq!(application.description, "Test");
    let request = only_request(&transport);
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.url, format!("{BASE}/applications/@me"));
    assert_eq!(
        request.json(),
        json!({
            "description": "Test",
            "integration_types_config": {"1": {}},
            "flags": 8192,
            "icon": null,
            "cover_image": "data:image/png;base64,AAAA",
            "tags": ["fun", "games"],
            "event_webhooks_url": "https://example.com/events",
            "event_webhooks_status": 2,
            "event_webhooks_types": ["ENTITLEMENT_CREATE"]
        })
    );
}

#[tokio::test]
async fn edits_application_by_id() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &application_json()));

    client(&transport)
        .edit_application(APPLICATION)
        .interactions_endpoint_url("https://example.com/interactions")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PATCH");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840")
    );
    assert_eq!(
        request.json(),
        json!({"interactions_endpoint_url": "https://example.com/interactions"})
    );
}

#[tokio::test]
async fn refuses_too_many_application_tags() {
    let transport = FakeTransport::new();

    let result = client(&transport)
        .edit_current_application()
        .tags(["a", "b", "c", "d", "e", "f"])
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn gets_activity_instance() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(
        200,
        &json!({
            "application_id": "1215413995645968394",
            "instance_id": "i-1276580072400224306-gc-912952092627435520-912954213460484116",
            "launch_id": "1276580072400224306",
            "location": {
                "id": "gc-912952092627435520-912954213460484116",
                "kind": "gc",
                "channel_id": "912954213460484116",
                "guild_id": "912952092627435520"
            },
            "users": ["205519959982473217"]
        }),
    ));

    let instance = client(&transport)
        .activity_instance(
            ApplicationId::new_const(1_215_413_995_645_968_394),
            "i-1276580072400224306-gc-912952092627435520-912954213460484116",
        )
        .await
        .unwrap();

    assert_eq!(instance.launch_id, "1276580072400224306");
    assert_eq!(instance.users.len(), 1);
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!(
            "{BASE}/applications/1215413995645968394/activity-instances/i-1276580072400224306-gc-912952092627435520-912954213460484116"
        )
    );
}

#[tokio::test]
async fn uploads_application_attachment() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(
        200,
        &json!({"attachment": {
            "id": "55", "filename": "art.png", "size": 3,
            "url": "https://cdn.discordapp.com/attachments/1/55/art.png",
            "proxy_url": "https://media.discordapp.net/attachments/1/55/art.png"
        }}),
    ));

    let uploaded = client(&transport)
        .upload_application_attachment(
            APPLICATION,
            FileUpload::new("art.png", &b"PNG"[..]).content_type("image/png"),
        )
        .await
        .unwrap();

    assert_eq!(uploaded.attachment.filename, "art.png");
    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/attachment")
    );
    assert!(
        request
            .header("content-type")
            .unwrap()
            .starts_with("multipart/form-data; boundary=")
    );
    let body = String::from_utf8(request.body.to_vec()).unwrap();
    assert!(
        body.contains(
            "name=\"file\"; filename=\"art.png\"\r\nContent-Type: image/png\r\n\r\nPNG\r\n"
        )
    );
}

fn metadata_json() -> Value {
    json!([{
        "type": 2,
        "key": "cookies_eaten",
        "name": "Cookies Eaten",
        "description": "Cookies Eaten Greater Than"
    }])
}

#[tokio::test]
async fn gets_role_connection_metadata() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &metadata_json()));

    let records = client(&transport)
        .role_connection_metadata(APPLICATION)
        .await
        .unwrap();

    assert_eq!(records[0].key, "cookies_eaten");
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/role-connections/metadata")
    );
}

#[tokio::test]
async fn sets_role_connection_metadata() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &metadata_json()));
    let records: Vec<RoleConnectionMetadata> = serde_json::from_value(metadata_json()).unwrap();

    let result = client(&transport)
        .set_role_connection_metadata(APPLICATION, records)
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    let request = only_request(&transport);
    assert_eq!(request.method, "PUT");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/role-connections/metadata")
    );
    assert_eq!(request.json(), metadata_json());
}

#[tokio::test]
async fn refuses_invalid_role_connection_key() {
    let transport = FakeTransport::new();
    let records: Vec<RoleConnectionMetadata> = serde_json::from_value(json!([{
        "type": 7, "key": "Has Cookies", "name": "Cookies", "description": "Has cookies"
    }]))
    .unwrap();

    let result = client(&transport)
        .set_role_connection_metadata(APPLICATION, records)
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn gets_global_commands_with_localizations() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &json!([command_json()])));

    let commands = client(&transport)
        .global_commands(APPLICATION)
        .with_localizations(true)
        .await
        .unwrap();

    assert_eq!(commands[0].name, "blep");
    assert_eq!(commands[0].options.len(), 1);
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/commands?with_localizations=true")
    );
}

#[tokio::test]
async fn creates_global_command() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(201, &command_json()));

    let command = client(&transport)
        .create_global_command(
            APPLICATION,
            blep().contexts([InteractionContextType::Guild]),
        )
        .await
        .unwrap();

    assert_eq!(command.id, COMMAND);
    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/commands")
    );
    assert_eq!(
        request.json(),
        json!({
            "type": 1,
            "name": "blep",
            "description": "Send a random adorable animal photo",
            "options": [{
                "type": 3, "name": "animal", "description": "The type of animal", "required": true,
                "choices": [{"name": "Dog", "value": "animal_dog"}]
            }],
            "contexts": [0]
        })
    );
}

#[tokio::test]
async fn refuses_invalid_command_without_sending() {
    let transport = FakeTransport::new();

    let result = client(&transport)
        .create_global_command(APPLICATION, CommandDefinition::chat_input("Blep", "x"))
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn gets_global_command() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &command_json()));

    let command = client(&transport)
        .global_command(APPLICATION, COMMAND)
        .await
        .unwrap();

    assert_eq!(command.description, "Send a random adorable animal photo");
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/commands/1000")
    );
}

#[tokio::test]
async fn edits_global_command_without_type() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &command_json()));

    client(&transport)
        .edit_global_command(
            APPLICATION,
            COMMAND,
            CommandDefinition::message("Report").nsfw(true),
        )
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PATCH");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/commands/1000")
    );
    assert_eq!(request.json(), json!({"name": "Report", "nsfw": true}));
}

#[tokio::test]
async fn deletes_global_command() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_global_command(APPLICATION, COMMAND)
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/commands/1000")
    );
}

#[tokio::test]
async fn sets_global_commands() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &json!([command_json()])));

    let commands = client(&transport)
        .set_global_commands(
            APPLICATION,
            vec![blep(), CommandDefinition::user("High Five")],
        )
        .await
        .unwrap();

    assert_eq!(commands.len(), 1);
    let request = only_request(&transport);
    assert_eq!(request.method, "PUT");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/commands")
    );
    let body = request.json();
    assert_eq!(body[0]["name"], "blep");
    assert_eq!(body[1], json!({"type": 2, "name": "High Five"}));
}

#[tokio::test]
async fn bulk_overwrite_names_the_bad_command() {
    let transport = FakeTransport::new();

    let error = client(&transport)
        .set_global_commands(
            APPLICATION,
            vec![blep(), CommandDefinition::chat_input("ok", "")],
        )
        .await
        .unwrap_err();

    let Error::Validation(validation) = error else {
        panic!("expected a validation error");
    };
    assert_eq!(validation.field, "commands[1].description");
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn gets_guild_commands() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &json!([command_json()])));

    let commands = client(&transport)
        .guild_commands(APPLICATION, GUILD)
        .with_localizations(false)
        .await
        .unwrap();

    assert_eq!(commands[0].guild_id, Some(GUILD));
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!(
            "{BASE}/applications/172150183260323840/guilds/290926798626357260/commands?with_localizations=false"
        )
    );
}

#[tokio::test]
async fn creates_guild_command() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &command_json()));

    client(&transport)
        .create_guild_command(APPLICATION, GUILD, CommandDefinition::user("High Five"))
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/guilds/290926798626357260/commands")
    );
    assert_eq!(request.json(), json!({"type": 2, "name": "High Five"}));
}

#[tokio::test]
async fn gets_guild_command() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &command_json()));

    let command = client(&transport)
        .guild_command(APPLICATION, GUILD, COMMAND)
        .await
        .unwrap();

    assert_eq!(command.name, "blep");
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/guilds/290926798626357260/commands/1000")
    );
}

#[tokio::test]
async fn edits_guild_command() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &command_json()));

    client(&transport)
        .edit_guild_command(APPLICATION, GUILD, COMMAND, blep())
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PATCH");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/guilds/290926798626357260/commands/1000")
    );
    let body = request.json();
    assert_eq!(body["name"], "blep");
    assert!(body.get("type").is_none());
}

#[tokio::test]
async fn deletes_guild_command() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_guild_command(APPLICATION, GUILD, COMMAND)
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/guilds/290926798626357260/commands/1000")
    );
}

#[tokio::test]
async fn sets_guild_commands() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &json!([])));

    let commands = client(&transport)
        .set_guild_commands(APPLICATION, GUILD, Vec::new())
        .await
        .unwrap();

    assert!(commands.is_empty());
    let request = only_request(&transport);
    assert_eq!(request.method, "PUT");
    assert_eq!(
        request.url,
        format!("{BASE}/applications/172150183260323840/guilds/290926798626357260/commands")
    );
    assert_eq!(request.json(), json!([]));
}

#[tokio::test]
async fn gets_guild_command_permissions() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &json!([permissions_json()])));

    let permissions = client(&transport)
        .guild_command_permissions(APPLICATION, GUILD)
        .await
        .unwrap();

    assert_eq!(permissions[0].guild_id, GUILD);
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!(
            "{BASE}/applications/172150183260323840/guilds/290926798626357260/commands/permissions"
        )
    );
}

#[tokio::test]
async fn gets_command_permissions() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &permissions_json()));

    let permissions = client(&transport)
        .command_permissions(APPLICATION, GUILD, COMMAND)
        .await
        .unwrap();

    assert!(!permissions.permissions[0].permission);
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.url,
        format!(
            "{BASE}/applications/172150183260323840/guilds/290926798626357260/commands/1000/permissions"
        )
    );
}

#[tokio::test]
async fn edits_command_permissions_with_bearer_token() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &permissions_json()));
    let client = Client::builder()
        .token(Token::bearer("oauth"))
        .transport(transport.clone())
        .build();
    let permissions: Vec<CommandPermission> = serde_json::from_value(json!([
        {"id": "290926798626357260", "type": 1, "permission": false}
    ]))
    .unwrap();

    let result = client
        .edit_command_permissions(APPLICATION, GUILD, COMMAND, permissions)
        .await
        .unwrap();

    assert_eq!(result.permissions.len(), 1);
    let request = only_request(&transport);
    assert_eq!(request.method, "PUT");
    assert_eq!(
        request.url,
        format!(
            "{BASE}/applications/172150183260323840/guilds/290926798626357260/commands/1000/permissions"
        )
    );
    assert_eq!(request.header("authorization"), Some("Bearer oauth"));
    assert_eq!(
        request.json(),
        json!({"permissions": [{"id": "290926798626357260", "type": 1, "permission": false}]})
    );
}
