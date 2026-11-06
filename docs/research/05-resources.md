# Research: Resource Data Objects

Research date: 2026-11-05. Source: `discord/discord-api-docs` at commit `da52492`, folder `developers/resources/`.
This file lists the parts that matter for typed objects: enums, bit flags, tagged unions, partial variants, and limits.

## Guild (`guild.mdx`)

| Enum | Values |
|---|---|
| verification_level | NONE 0, LOW 1, MEDIUM 2, HIGH 3, VERY_HIGH 4 |
| default_message_notifications | ALL_MESSAGES 0, ONLY_MENTIONS 1 |
| explicit_content_filter | DISABLED 0, MEMBERS_WITHOUT_ROLES 1, ALL_MEMBERS 2 |
| mfa_level | NONE 0, ELEVATED 1 |
| nsfw_level | DEFAULT 0, EXPLICIT 1, SAFE 2, AGE_RESTRICTED 3 |
| premium_tier | 0..3 |
| onboarding mode | DEFAULT 0, ADVANCED 1 |
| onboarding prompt type | MULTIPLE_CHOICE 0, DROPDOWN 1 |
| integration expire_behavior | remove role 0, kick 1 |

- `system_channel_flags` bits 0..5 (SUPPRESS_JOIN_NOTIFICATIONS and others).
- `features` are strings. Use an open enum with `Unknown(String)`. Recent: `ENHANCED_ROLE_COLORS`, `GUILD_TAGS`, `PRUNE_REQUIRES_ADMIN`, `GUESTS_ENABLED`, `SOUNDBOARD`, `MORE_SOUNDBOARD`.
- Integration `type` is a string: twitch, youtube, discord, guild_subscription.
- `owner` and `permissions` come only from `GET /users/@me/guilds`.
- `incidents_data`: `invites_disabled_until`, `dms_disabled_until`, `dm_spam_detected_at?`, `raid_detected_at?`.

Variants to type separately:

- Unavailable guild `{id, unavailable?}`. GUILD_DELETE with `unavailable: true` is an outage. Absent field means removal.
- GUILD_CREATE extras (see the gateway notes).
- Guild Preview, Widget, invite partial guild, template `serialized_source_guild`.

Endpoints to note:

- Bulk ban: max 200 users, `delete_message_seconds` 0..604800. Returns `{banned_users, failed_users}`.
- Role member counts: map role ID to count.
- Create Guild is removed for apps (2025-07).

## Member

- Fields: `user?`, `nick?`, `avatar?`, `banner?`, `roles`, `joined_at` (nullable for guests), `premium_since?`, `deaf`, `mute`, `flags`, `pending?`, `permissions?`, `communication_disabled_until?`, `avatar_decoration_data?`, `collectibles?`.
- `user` is absent in members inside MESSAGE_CREATE/UPDATE.
- Interaction partial member: no `user`, `deaf`, `mute`. Has `permissions`.
- Member flags: DID_REJOIN 0, COMPLETED_ONBOARDING 1, BYPASSES_VERIFICATION 2 (editable), STARTED_ONBOARDING 3, IS_GUEST 4, STARTED_HOME_ACTIONS 5, COMPLETED_HOME_ACTIONS 6, AUTOMOD_QUARANTINED_USERNAME 7, DM_SETTINGS_UPSELL_ACKNOWLEDGED 9, AUTOMOD_QUARANTINED_GUILD_TAG 10.

## Role

- See the permissions section in [02-http.md](02-http.md).
- Role tags `premium_subscriber`, `available_for_purchase`, `guild_connections`: present and null = true. Needs a custom deserializer.
- Partial role (on invites, 2026-02): `id`, `name`, `position`, `color`, `colors`, `icon`, `unicode_emoji`.

## Channel (`channel.mdx`)

| Type | Value |
|---|---|
| GUILD_TEXT | 0 |
| DM | 1 |
| GUILD_VOICE | 2 |
| GROUP_DM | 3 |
| GUILD_CATEGORY | 4 (max 50 children) |
| GUILD_ANNOUNCEMENT | 5 |
| ANNOUNCEMENT_THREAD | 10 |
| PUBLIC_THREAD | 11 |
| PRIVATE_THREAD | 12 |
| GUILD_STAGE_VOICE | 13 |
| GUILD_DIRECTORY | 14 |
| GUILD_FORUM | 15 |
| GUILD_MEDIA | 16 (still in development) |

- Channel flags: PINNED 1<<1, REQUIRE_TAG 1<<4, HIDE_MEDIA_DOWNLOAD_OPTIONS 1<<15, CHANNEL_OBFUSCATED 1<<17 (gateway only), IS_SPOILER_CHANNEL 1<<21.
- Enums: video_quality_mode AUTO 1, FULL 2. default_sort_order LATEST_ACTIVITY 0, CREATION_DATE 1. default_forum_layout NOT_SET 0, LIST_VIEW 1, GALLERY_VIEW 2. Overwrite type: role 0, member 1.
- `application_id` is nullable (2026-08).
- `topic`: max 4096 for forum and media, else max 1024.
- `rate_limit_per_user`: 0..21600.
- `auto_archive_duration`: 60, 1440, 4320, 10080.
- Forum tag: `id`, `name` (max 20), `moderated`, `emoji_id`/`emoji_name` (at most one).
- Obfuscated channel: name `___hidden___`. Only `id`, `type`, `position`, `parent_id` are reliable. Detect it by the flag.
- Thread metadata: `archived`, `auto_archive_duration`, `archive_timestamp`, `locked`, `invitable?`, `create_timestamp?`.
- Thread member: `id?`, `user_id?`, `join_timestamp`, `flags`, `member?`.
- Design note: one struct with many optional fields, keyed by `type`. Partial and obfuscated forms argue for a lenient shape.

## Message (`message.mdx`)

Message types (the gaps are real):

| Value | Type | Value | Type |
|---|---|---|---|
| 0 | DEFAULT | 21 | THREAD_STARTER_MESSAGE |
| 1 | RECIPIENT_ADD | 22 | GUILD_INVITE_REMINDER |
| 2 | RECIPIENT_REMOVE | 23 | CONTEXT_MENU_COMMAND |
| 3 | CALL | 24 | AUTO_MODERATION_ACTION |
| 4 | CHANNEL_NAME_CHANGE | 25 | ROLE_SUBSCRIPTION_PURCHASE |
| 5 | CHANNEL_ICON_CHANGE | 26 | INTERACTION_PREMIUM_UPSELL |
| 6 | CHANNEL_PINNED_MESSAGE | 27 | STAGE_START |
| 7 | USER_JOIN | 28 | STAGE_END |
| 8 | GUILD_BOOST | 29 | STAGE_SPEAKER |
| 9..11 | GUILD_BOOST_TIER_1..3 | 31 | STAGE_TOPIC |
| 12 | CHANNEL_FOLLOW_ADD | 32 | GUILD_APPLICATION_PREMIUM_SUBSCRIPTION |
| 14..17 | GUILD_DISCOVERY_* | 36..39 | GUILD_INCIDENT_* |
| 18 | THREAD_CREATED | 44 | PURCHASE_NOTIFICATION |
| 19 | REPLY | 46 | POLL_RESULT |
| 20 | CHAT_INPUT_COMMAND | | |

| Message flag | Bit |
|---|---|
| CROSSPOSTED | 0 |
| IS_CROSSPOST | 1 |
| SUPPRESS_EMBEDS | 2 |
| SOURCE_MESSAGE_DELETED | 3 |
| URGENT | 4 |
| HAS_THREAD | 5 |
| EPHEMERAL | 6 |
| LOADING | 7 |
| FAILED_TO_MENTION_SOME_ROLES_IN_THREAD | 8 |
| SUPPRESS_NOTIFICATIONS | 12 |
| IS_VOICE_MESSAGE | 13 |
| HAS_SNAPSHOT | 14 |
| IS_COMPONENTS_V2 | 15 |

- Message activity type: JOIN 1, SPECTATE 2, LISTEN 3, JOIN_REQUEST 5, STREAM_REQUEST 6.
- New fields: `shared_client_theme` (2026-03), `call`, `position`, `resolved`.
- Deprecated: `interaction` (use `interaction_metadata`), `stickers` (use `sticker_items`).
- `referenced_message`: only on types 19, 21, 23. Absent = unknown. Null = deleted.
- `message_reference`: `type?` (DEFAULT 0, FORWARD 1), `message_id?`, `channel_id?`, `guild_id?`, `fail_if_not_exists?`.
  - Forward rules: only types 0, 19, 20, 23. Not with poll, call, or activity. Content access required (160014).
- `message_snapshots`: `[{message: partial}]`. No author and no guild.
- `interaction_metadata`: tagged union on `type` (command, component, modal submit). Modal variant is recursive.
- Reaction: `count`, `count_details {burst, normal}`, `me`, `me_burst`, `emoji`, `burst_colors`.
- Voice message: flag IS_VOICE_MESSAGE, one audio attachment with `duration_secs` and `waveform`, no content, no edit.
- Poll result message (type 46): `poll_result` embed with fixed field names.

Limits and rules:

- `content` max 2000. `nonce` max 25 characters. `enforce_nonce` deduplicates.
- Max 10 embeds. Title 256, description 4096, 25 fields, field name 256, value 1024, footer 2048, author 256. 6000 characters in total.
- Request body max 25 MiB. Per-file default 20 MiB. The attachment count limit is not documented.
- Bulk delete: 2..100 IDs. Fails if a message is older than 14 days or an ID repeats.
- Pins: `GET /channels/{c}/messages/pins?before=&limit=1..50` returns `{items: [{pinned_at, message}], has_more}`. Max 250 pins per channel.
- Create flags: SUPPRESS_EMBEDS, SUPPRESS_NOTIFICATIONS, IS_VOICE_MESSAGE, IS_COMPONENTS_V2.
- On edit, `attachments` must list each attachment to keep.
- `allowed_mentions`: `parse?`, `roles?` (max 100), `users?` (max 100), `replied_user?`. `parse` and an explicit list of the same kind conflict. On edit, resend it, or defaults apply.
- Search (2026-03): `GET /guilds/{g}/messages/search`. Needs MESSAGE_CONTENT. Returns 202 + `retry_after` while indexing. `limit` 1..25, `offset` max 9975.

## Embed and Attachment

- Embed `type` string: rich, image, video, gifv, article, link, poll_result, components (2026-10).
- Embed flags: CONTAINS_EXPLICIT_CONTENT 1<<4, IS_CONTENT_INVENTORY_ENTRY 1<<5, CONTAINS_GORE_CONTENT 1<<6, CONTAINS_SELF_HARM_CONTENT 1<<7.
- Embed media: `content_type`, `placeholder`, `placeholder_version`, `description`, `flags` (IS_ANIMATED 1<<5).
- Attachment fields: `id`, `filename`, `title?`, `description?`, `content_type?`, `size`, `url`, `proxy_url`, `height?`, `width?`, `placeholder`, `placeholder_version`, `ephemeral?`, `duration_secs?`, `waveform?`, `flags?`, `clip_participants`, `clip_created_at`, `application?`.
- Attachment flags: IS_CLIP 0, IS_THUMBNAIL 1, IS_REMIX 2 (deprecated), IS_SPOILER 3, CONTAINS_EXPLICIT_CONTENT 4, IS_ANIMATED 5, CONTAINS_GORE_CONTENT 6, CONTAINS_SELF_HARM_CONTENT 7.
- Attachment request object (separate type, 2026-06): `id`, `filename?`, `title?`, `description?`, `duration_secs?`, `waveform?`, `is_spoiler?`.
- Unfurled media `content_scan_metadata.flags` uses different bits (IS_EXPLICIT 0, IS_GORE 1, IS_SELF_HARM 2). Use a separate flags type.

## User (`user.mdx`)

- Fields: `id`, `username`, `discriminator`, `global_name?`, `avatar`, `bot?`, `system?`, `mfa_enabled?`, `banner?`, `accent_color?`, `locale?`, `verified?`, `email?`, `flags?`, `premium_type?`, `public_flags?`, `avatar_decoration_data?`, `collectibles?`, `primary_guild?`.
- `premium_type`: 0 None, 1 Classic, 2 Nitro, 3 Basic. Returns 0 without `identify.premium` (2026-05).
- User flags: STAFF 0, PARTNER 1, HYPESQUAD 2, BUG_HUNTER_LEVEL_1 3, HOUSE_1..3 6..8, PREMIUM_EARLY_SUPPORTER 9, TEAM_PSEUDO_USER 10, BUG_HUNTER_LEVEL_2 14, VERIFIED_BOT 16, VERIFIED_DEVELOPER 17, CERTIFIED_MODERATOR 18, BOT_HTTP_INTERACTIONS 19, ACTIVE_DEVELOPER 22 (removed from docs 2026-03; keep the bit).
- `avatar_decoration_data {asset, sku_id}`.
- `collectibles {nameplate?: {sku_id, asset, label, palette}}`.
- `primary_guild {identity_guild_id?, identity_enabled?, tag? (max 4), badge?}`.
- Username 2..32 characters. Nickname 1..32.

## Emoji, Sticker, Soundboard

- Emoji: `id` and `name` nullable. `roles?`, `user?`, `require_colons?`, `managed?`, `animated?`, `available?`. Max 256 KiB.
- Sticker type: STANDARD 1, GUILD 2. Format: PNG 1, APNG 2, LOTTIE 3, GIF 4. Max 3 per message. Partial form `sticker_items {id, name, format_type}`.
- Soundboard sound: `name`, `sound_id`, `volume` (0..1), `emoji_id?`, `emoji_name?`, `guild_id?`, `available`, `user?`.

## Poll (`poll.mdx`)

- `question` (text max 300), `answers` (max 10; text max 55, `emoji?`), `expiry?`, `allow_multiselect`, `layout_type` (DEFAULT 1), `results?`.
- Results: `is_finalized`, `answer_counts [{id, count, me_voted}]`.
- Request object uses `duration` in hours (default 24, max 32 days).
- Polls cannot be edited. Apps cannot vote.

## Invite (`invite.mdx`)

- Type: GUILD 0, GROUP_DM 1, FRIEND 2. Target type: STREAM 1, EMBEDDED_APPLICATION 2. Flags: IS_GUEST_INVITE 1<<0.
- Fields: `code`, `guild?`, `channel?`, `inviter?`, `target_user?`, `target_application?`, approximate counts, `expires_at`, `guild_scheduled_event?`, `flags?`, `roles?`.
- Metadata: `uses`, `max_uses`, `max_age`, `temporary`, `created_at`.
- Create: `max_age` 0..604800, `max_uses` 0..100, `target_users_file` or `target_user_ids` (max 1000), `role_ids`.
- Target-user job status: UNSPECIFIED 0, PROCESSING 1, COMPLETED 2, FAILED 3.

## Guild Scheduled Event

- privacy_level GUILD_ONLY 2. entity_type STAGE_INSTANCE 1, VOICE 2, EXTERNAL 3. status SCHEDULED 1, ACTIVE 2, COMPLETED 3, CANCELED 4.
- EXTERNAL: `channel_id` null, `entity_metadata.location`, `scheduled_end_time` required.
- `recurrence_rule`: `start`, `end?`, `frequency`, `interval`, `by_weekday?`, `by_n_weekday?`, `by_month?`, `by_month_day?`, `by_year_day?`, `count?`.
  - Frequency: YEARLY 0, MONTHLY 1, WEEKLY 2, DAILY 3. Weekday MON 0..SUN 6. Month JAN 1..DEC 12.
  - The prose numbers frequencies differently. Trust the table.

## Auto Moderation

- Trigger type (max per guild): KEYWORD 1 (6), SPAM 3 (1), KEYWORD_PRESET 4 (1), MENTION_SPAM 5 (1), MEMBER_PROFILE 6 (1).
- event_type: MESSAGE_SEND 1, MEMBER_UPDATE 2. Presets: PROFANITY 1, SEXUAL_CONTENT 2, SLURS 3.
- trigger_metadata: `keyword_filter` (max 1000), `regex_patterns` (max 10, Rust regex), `allow_list`, `presets`, `mention_total_limit` (max 50), `mention_raid_protection_enabled`.
- Actions: BLOCK_MESSAGE 1 (`custom_message?`), SEND_ALERT_MESSAGE 2 (`channel_id`), TIMEOUT 3 (`duration_seconds`), BLOCK_MEMBER_INTERACTION 4.
- `exempt_roles` max 20. `exempt_channels` max 50.

## Audit Log

- 71 action types: 1, 10..16, 20..28, 30..33, 40..42, 50..52, 60..62, 72..75, 80..85, 90..92, 100..102, 110..112, 121, 130..132, 140..146, 150..151, 163..167, 190..193.
- New: 16 CHANNEL_POSITION_UPDATE, 33 ROLE_POSITION_UPDATE (2026-10), 146 AUTO_MODERATION_QUARANTINE_USER, 192 VOICE_CHANNEL_STATUS_CREATE, 193 VOICE_CHANNEL_STATUS_DELETE.
- Entry: `target_id?`, `changes?`, `user_id?`, `id`, `action_type`, `options?`, `reason?`.
- Some actions merge into an existing entry and fire no new event.
- Change `{key, old_value?, new_value?}` is untyped. Use a JSON value with typed accessors.
- Optional info numbers are strings.

## Webhook, Stage, Template, Voice state, Lobby

- Webhook type: Incoming 1, Channel Follower 2, Application 3. Execute query: `wait`, `thread_id`, `with_components`. Body adds `thread_name`, `applied_tags`, `poll`, `flags`.
- Stage instance: privacy PUBLIC 1 (deprecated), GUILD_ONLY 2. Topic 1..120.
- Template: `code`, `usage_count`, `creator`, `source_guild_id`, `serialized_source_guild`, `is_dirty?`.
- Voice state: `channel_id?`, `request_to_speak_timestamp?`, `self_stream?`.
- Lobby: `id`, `application_id`, `metadata?`, `members`, `linked_channel?`. Member flags: CanLinkLobby 1<<0.

## Open points

- Max attachments per message.
- Field set of the invite partial guild.
- Audit log 192 name (use CREATE).
