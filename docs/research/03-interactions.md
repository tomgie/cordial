# Research: Interactions, Commands, Components, Modals, Monetization

Research date: 2026-11-05. Source: `discord/discord-api-docs` at commit `da52492`, folder `developers/`.
Live docs: `https://docs.discord.com/developers/<path>`. **[?]** marks a point that is not certain.

## 1. Interaction object (`interactions/receiving-and-responding`)

Fields:

- `id`, `application_id`, `type`, `token`, `version` (always 1).
- `data?`: present for all types except PING.
- `guild?` (partial), `guild_id?`, `channel?` (partial, has `id` and `type`), `channel_id?`.
- `member?` (in a guild, with `permissions`) or `user?` (in a DM).
- `message?`: for components and for modals opened from components.
- `app_permissions` (string), `locale?`, `guild_locale?`.
- `entitlements[]`, `authorizing_integration_owners`, `context?`, `attachment_size_limit` (bytes).

| Interaction type | Value |
|---|---|
| PING | 1 |
| APPLICATION_COMMAND | 2 |
| MESSAGE_COMPONENT | 3 |
| APPLICATION_COMMAND_AUTOCOMPLETE | 4 |
| MODAL_SUBMIT | 5 |

| Context | Value | Meaning |
|---|---|---|
| GUILD | 0 | In servers |
| BOT_DM | 1 | DM with the bot |
| PRIVATE_CHANNEL | 2 | Group DMs and DMs with other users |

- Integration types: GUILD_INSTALL = 0, USER_INSTALL = 1.
- `authorizing_integration_owners`: map with keys `"0"` and `"1"`. Value for `"0"` is the guild ID, or `"0"` in the bot DM. Value for `"1"` is the user ID.

`data` by type:

- Command (types 2, 4): `id`, `name`, `type`, `resolved?`, `options?`, `guild_id?`, `target_id?`.
- Component: `custom_id`, `component_type`, `values?`, `resolved?`. Examples also show `id` **[?]**.
- Modal submit: `custom_id`, `components`, `resolved?`.
- Resolved: maps keyed by snowflake for `users`, `members`, `roles`, `channels`, `messages`, `attachments`.
  - Partial members have no `user`, `deaf`, `mute`.
  - Partial channels have a fixed field subset. Since 2026-07-16 they have `app_permissions` when the bot is in the guild.
- Option: `name`, `type`, `value?` (string, int, double, bool), `options?`, `focused?`.

Message metadata:

- `message.interaction` is deprecated. Use `interaction_metadata`.
- `interaction_metadata` has three variants: command (`target_user?`, `target_message_id?`), component (`interacted_message_id`), modal submit (`triggering_interaction_metadata`).

## 2. Receiving

- Two modes. They are mutually exclusive:
  - Gateway `INTERACTION_CREATE` (default).
  - HTTP "Interactions Endpoint URL". Setting it disables gateway delivery.
- Responses always go over HTTP.
- Ed25519 check on every HTTP request:
  - Headers `X-Signature-Ed25519` (hex) and `X-Signature-Timestamp`.
  - Verify `timestamp + raw_body` with the app `verify_key`.
  - On failure, return 401. Discord sends invalid signatures on purpose to test this.
- PING: reply 200 with `{type: 1}`.
- Answer an HTTP interaction in one of two ways:
  - Reply 200 with the response body.
  - Call the callback endpoint, then reply 202 with no body.
- Errors: 10062 Unknown interaction, 40060 Already acknowledged, 40094 Too many follow-ups.

## 3. Responding

| Callback type | Value | Notes |
|---|---|---|
| PONG | 1 | |
| CHANNEL_MESSAGE_WITH_SOURCE | 4 | |
| DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE | 5 | Loading state |
| DEFERRED_UPDATE_MESSAGE | 6 | Components and modal submit only |
| UPDATE_MESSAGE | 7 | Components and modal submit only |
| APPLICATION_COMMAND_AUTOCOMPLETE_RESULT | 8 | |
| MODAL | 9 | Not for MODAL_SUBMIT or PING |
| PREMIUM_REQUIRED | 10 | Deprecated 2024-06-17 |
| LAUNCH_ACTIVITY | 12 | Activity apps only |

- Message data: `tts?`, `content?`, `embeds?` (max 10), `allowed_mentions?`, `flags?`, `components?`, `attachments?`, `poll?`.
- Allowed flags: SUPPRESS_EMBEDS (1<<2), EPHEMERAL (1<<6), SUPPRESS_NOTIFICATIONS (1<<12), IS_VOICE_MESSAGE (1<<13), IS_COMPONENTS_V2 (1<<15).
- With type 5, only EPHEMERAL is valid. Set IS_COMPONENTS_V2 later through Edit Original Response.
- Autocomplete: `choices` (max 25). Modal: `custom_id` (1..100), `title` (max 45), `components` (1..5).
- Deadlines: initial response in 3 s. Token valid for 15 min.

Endpoints:

- `POST /interactions/{id}/{token}/callback`. Returns 204, or 200 with `?with_response=true`.
  - Body: `{interaction: {id, type, activity_instance_id?, response_message_id?, response_message_loading?, response_message_ephemeral?}, resource?: {type, activity_instance?, message?}}`.
- `GET/PATCH/DELETE /webhooks/{app}/{token}/messages/@original`.
- `POST /webhooks/{app}/{token}` (follow-up). `wait` is always true. No `thread_id`, `avatar_url`, `username`.
- `GET/PATCH/DELETE /webhooks/{app}/{token}/messages/{m}`.
- These endpoints are not under the global rate limit.
- User-install only interactions: max 5 follow-ups.
- IS_COMPONENTS_V2 cannot be removed on edit. When you set it, clear `content`, `embeds`, `poll`.

## 4. Application commands (`interactions/application-commands`)

Command object:

- `id`, `type?` (default 1), `application_id`, `guild_id?`, `version`.
- `name` (1..32), `name_localizations?`, `description` (1..100; empty for USER and MESSAGE), `description_localizations?`.
- `options?` (max 25, CHAT_INPUT only).
- `default_member_permissions` (nullable string; `"0"` = admins only).
- `dm_permission?` (deprecated), `default_permission?` (to be deprecated), `nsfw?`.
- `integration_types?`, `contexts?` (global commands only).
- `handler?` (PRIMARY_ENTRY_POINT only).

| Command type | Value |
|---|---|
| CHAT_INPUT | 1 |
| USER | 2 |
| MESSAGE | 3 |
| PRIMARY_ENTRY_POINT | 4 |

- Name regex for CHAT_INPUT and options: `^[-_ʼ\p{L}\p{N}\p{sc=Deva}\p{sc=Thai}]{1,32}$`. Use lowercase.

Option object:

- `type`, `name`, `name_localizations?`, `description`, `description_localizations?`.
- `required?` (required options first), `choices?` (max 25), `options?` (max 25), `channel_types?`.
- `min_value?`/`max_value?`, `min_length?` (0..6000)/`max_length?` (1..6000).
- `autocomplete?` (not with `choices`).
- `file_types?` (2026-08-05, ATTACHMENT only, max 10 entries).

| Option type | Value |
|---|---|
| SUB_COMMAND | 1 |
| SUB_COMMAND_GROUP | 2 |
| STRING | 3 |
| INTEGER | 4 (±2^53-1) |
| BOOLEAN | 5 |
| USER | 6 |
| CHANNEL | 7 |
| ROLE | 8 |
| MENTIONABLE | 9 |
| NUMBER | 10 |
| ATTACHMENT | 11 |

- Choice: `name` (1..100), `name_localizations?`, `value` (string max 100, int, or double).
- Nesting: command > subcommand, or command > group > subcommand. No group in a group. A command with subcommands cannot run itself.
- Size limit: 8000 characters in total (old docs said 4000) **[?]**.
- Counts: 100 CHAT_INPUT, 15 USER, 15 MESSAGE (2026-03-03), 1 PRIMARY_ENTRY_POINT (global only). Same counts per guild.
- 200 command creates per day per guild.
- POST is an upsert by name (201 new, 200 replaced).
- Autocomplete sends partial values. The typed option has `focused: true`.
- Locales: id, da, de, en-GB, en-US, es-ES, es-419, fr, hr, it, lt, hu, nl, no, pl, pt-BR, ro, fi, sv-SE, vi, tr, cs, el, bg, ru, uk, hi, th, zh-CN, ja, zh-TW, ko.

Endpoints:

- Global and guild: GET list, POST, PUT (bulk overwrite), GET one, PATCH, DELETE.
- Permissions: `GET .../guilds/{g}/commands/permissions`, `GET/PUT .../commands/{id}/permissions`.
  - PUT needs a Bearer token with `applications.commands.permissions.update`. Max 100 entries.
  - Entry: `{id, type (ROLE 1, USER 2, CHANNEL 3), permission: bool}`.
  - Constants: `@everyone` = guild_id. All channels = guild_id - 1.
  - Batch PUT is disabled.

## 5. Components (`components/reference`)

| ID | Name | Where |
|---|---|---|
| 1 | Action Row | Message |
| 2 | Button | Message |
| 3 | String Select | Message, Modal |
| 4 | Text Input | Modal |
| 5 | User Select | Message, Modal |
| 6 | Role Select | Message, Modal |
| 7 | Mentionable Select | Message, Modal |
| 8 | Channel Select | Message, Modal |
| 9 | Section | Message |
| 10 | Text Display | Message, Modal |
| 11 | Thumbnail | Message |
| 12 | Media Gallery | Message |
| 13 | File | Message |
| 14 | Separator | Message |
| 17 | Container | Message |
| 18 | Label | Modal |
| 19 | File Upload | Modal |
| 21 | Radio Group | Modal (2026-02-12) |
| 22 | Checkbox Group | Modal (2026-02-12) |
| 23 | Checkbox | Modal (2026-02-12) |

- Every component has `type` and `id?` (i32, unique in the message, auto-assigned).
- Interactive components need `custom_id` (1..100, unique in the message).
- Action Row: up to 5 buttons or one select. In modals it is deprecated. Use Label.
- Button: `style`, `label?` (max 80), `emoji?`, `custom_id?`, `sku_id?`, `url?` (max 512), `disabled?`.
  - Styles: Primary 1, Secondary 2, Success 3, Danger 4 (need `custom_id`); Link 5 (`url`); Premium 6 (`sku_id` only).
- String Select: `custom_id`, `options` (max 25), `placeholder?` (max 150), `min_values?`, `max_values?`, `required?` (modal), `disabled?` (message).
  - Option: `label`, `value`, `description?` (each max 100), `emoji?`, `default?`.
- Auto-populated selects: same without `options`, plus `default_values?` `[{id, type}]`. Channel Select adds `channel_types?`.
- Text Input: `custom_id`, `style` (Short 1, Paragraph 2), `min_length?`, `max_length?` (max 4000), `required?`, `value?`, `placeholder?` (max 100). `label` is deprecated.
- Section: 1..3 Text Display plus `accessory` (Button or Thumbnail).
- Text Display: `content` (markdown).
- Thumbnail: `media`, `description?` (max 1024), `spoiler?`.
- Media Gallery: 1..10 items `{media, description?, spoiler?}`.
- File: `file` (`attachment://` only), `spoiler?`.
- Separator: `divider?` (default true), `spacing?` (1 small, 2 large).
- Container: `components`, `accent_color?`, `spoiler?`. Children: Action Row, Text Display, Section, Media Gallery, Separator, File.
- Unfurled media item: only `url` is settable. Response fields: `proxy_url`, `height`, `width`, `placeholder`, `placeholder_version`, `content_type`, `flags`, `attachment_id`, `content_scan_metadata{flags}`.
  - Content scan flags (2026-10-28): IS_EXPLICIT 1<<0, IS_GORE 1<<1, IS_SELF_HARM 1<<2.

IS_COMPONENTS_V2 (1<<15):

- Cannot be removed after it is set.
- `content`, `embeds`, `sticker_ids`, `poll` cause 400.
- Attachments show only if a component refers to them.
- Max 40 components in total, nested ones included.
- Legacy messages: up to 5 Action Rows with `content` and `embeds`. Not deprecated.
- Webhooks not owned by the app need `?with_components=true` and only non-interactive components.

## 6. Modals

- Callback type 9: `{custom_id, title, components (1..5)}`.
- Top-level children: Label and Text Display. Action Row + Text Input still works but is deprecated.
- No component in a modal can be `disabled`.
- Label (18): `label` (max 45), `description?` (max 100), `component`. Child: Text Input, the five selects, File Upload, Radio Group, Checkbox Group, Checkbox.
- File Upload (19): `custom_id`, `min_values?`, `max_values?` (max 10), `required?`, `file_types?`. Submit gives attachment IDs in `values` and the files in `resolved.attachments`.
- Radio Group (21): `custom_id`, `options` (2..10), `required?`. Submit `value` (string or null).
- Checkbox Group (22): `custom_id`, `options` (1..10), `min_values?`, `max_values?`, `required?`. Submit `values`.
- Checkbox (23): `custom_id`, `default?`. Submit `value: bool`. Cannot be required.
- Submit `data.components` mirrors the layout. Label gives `{type: 18, id, component: {type, id, custom_id, value | values}}`.

## 7. Monetization

SKU: `{id, type, application_id, name, slug, flags}`.

- Types: DURABLE 2, CONSUMABLE 3, SUBSCRIPTION 5, SUBSCRIPTION_GROUP 6.
- Flags: AVAILABLE 1<<2, GUILD_SUBSCRIPTION 1<<7, USER_SUBSCRIPTION 1<<8.

Entitlement: `{id, sku_id, application_id, user_id?, type, deleted, starts_at?, ends_at?, guild_id?, consumed?}`. Examples also show `subscription_id`, `promotion_id`, `gift_code_flags` **[?]**.

| Entitlement type | Value |
|---|---|
| PURCHASE | 1 |
| PREMIUM_SUBSCRIPTION | 2 |
| DEVELOPER_GIFT | 3 |
| TEST_MODE_PURCHASE | 4 |
| FREE_PURCHASE | 5 |
| USER_GIFT | 6 |
| PREMIUM_PURCHASE | 7 |
| APPLICATION_SUBSCRIPTION | 8 |

- Endpoints: list (filters `user_id`, `sku_ids`, `before`, `after`, `limit`, `guild_id`, `exclude_ended`, `exclude_deleted`), get, consume, create test, delete test.
- Subscription: `{id, user_id, sku_ids, entitlement_ids, renewal_sku_ids?, current_period_start, current_period_end, status, canceled_at?, country?}`. Status: ACTIVE 0, INACTIVE 1, ENDING 2.
- Grant perks from entitlements, not from subscription status.
- Events: ENTITLEMENT_* and SUBSCRIPTION_*. Active subscription entitlements have `ends_at = null`. ENTITLEMENT_UPDATE fires when a subscription ends.
- Purchase prompts: Premium button, store deep links.

## 8. Activities

- Launch through the Entry Point command or callback LAUNCH_ACTIVITY (12).
- Entry Point command: type 4, global, max one. `handler`: APP_HANDLER 1 or DISCORD_LAUNCH_ACTIVITY 2.
- A bulk overwrite must keep the Entry Point command **[?]**.
- `GET /applications/{app}/activity-instances/{id}` returns `{application_id, instance_id, launch_id, location, users}`.

## 9. Application

- Fields: `id`, `name`, `icon`, `description`, `bot_public`, `bot_require_code_grant`, `bot?`, `owner?`, `verify_key`, `team?`, `guild_id?`, `flags?`, `flags_new?` (2026-05, string, response only), approximate counts, `redirect_uris?`, `interactions_endpoint_url?`, `role_connections_verification_url?`, `event_webhooks_url?`, `event_webhooks_status?`, `event_webhooks_types?`, `tags?`, `install_params?`, `integration_types_config?`, `custom_install_url?`.

| Application flag | Value |
|---|---|
| AUTO_MODERATION_RULE_CREATE_BADGE | 1<<6 |
| GATEWAY_PRESENCE | 1<<12 |
| GATEWAY_PRESENCE_LIMITED | 1<<13 |
| GATEWAY_GUILD_MEMBERS | 1<<14 |
| GATEWAY_GUILD_MEMBERS_LIMITED | 1<<15 |
| VERIFICATION_PENDING_GUILD_LIMIT | 1<<16 |
| EMBEDDED | 1<<17 |
| GATEWAY_MESSAGE_CONTENT | 1<<18 |
| GATEWAY_MESSAGE_CONTENT_LIMITED | 1<<19 |
| APPLICATION_COMMAND_BADGE | 1<<23 |

- `PATCH /applications/@me`: all fields optional. Only the three `*_LIMITED` flags can change.
- Application emojis: max 2000. Endpoints under `/applications/{app}/emojis`. GET list returns `{items}`.
- Role connection metadata: max 5 records. Types 1..8 (integer, datetime, boolean comparisons).

Webhook events (2024-10):

- Same Ed25519 signature check as interactions.
- PING is `type: 0`. Reply 204 with no body.
- Acknowledge events with 204 in 3 s. Discord retries for up to 10 min.
- Envelope: `{version, application_id, type (0 PING, 1 Event), event?: {type, timestamp, data?}}`.
- Event types: APPLICATION_AUTHORIZED, APPLICATION_DEAUTHORIZED, ENTITLEMENT_CREATE/UPDATE/DELETE, QUEST_USER_ENROLLMENT, LOBBY_MESSAGE_CREATE/UPDATE/DELETE, GAME_DIRECT_MESSAGE_CREATE/UPDATE/DELETE.

## Open points

- Date of the command size change (4000 to 8000).
- Text Display character limit.
- Entitlement fields that appear only in examples.
- Component interaction `data.id`.
- Bulk overwrite without the Entry Point command.
- Webhooks: `files[n]` with IS_COMPONENTS_V2 gives 400.
