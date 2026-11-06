# Research: API Change Log, 2024-01 to 2026-11

Research date: 2026-11-05. Source: `developers/change-log.mdx` and git history of `discord/discord-api-docs`.

Tags: [EP] new endpoint, [F] new field, enum, or flag, [GW] gateway, [BRK] breaking change, [DEP] deprecation, [DOC] documentation only.
Social SDK and Activities entries with no bot effect are left out.

## 2024

| Date | Change | Tags |
|---|---|---|
| 02-12 | `enforce_nonce` on Create Message | F |
| 03-15 | Prune needs MANAGE_GUILD and KICK_MEMBERS | BRK |
| 03-18 | User-installable apps preview: `integration_types`, `contexts`, `interaction_metadata` | F, DEP |
| 04-24 | One-time purchases: SKU and entitlement types, Consume Entitlement | EP, F |
| 05-31 | AutoMod MEMBER_PROFILE trigger and BLOCK_MEMBER_INTERACTION action | F |
| 06-17 | Premium button style; PREMIUM_REQUIRED callback deprecated | F, DEP |
| 06-27 | User-installed apps GA; USE_EXTERNAL_APPS; 5 follow-up limit | BRK, F |
| 07-09 | `banner` in `GET /users/@me/guilds` | F |
| 07-15 | Message forwarding: `message_reference.type`, `message_snapshots` | F |
| 07-16 | Member banners | F |
| 07-18 | Application emojis | EP |
| 08-05 | Voice state HTTP endpoints | EP |
| 08-09 | `approximate_user_install_count` | F |
| 08-12 | Get Guild Role | EP |
| 08-13 | Voice gateway v8; versions below 4 rejected from 2024-11-18 | GW, BRK |
| 08-13 | New voice encryption modes; old modes removed 2024-11-18 | BRK |
| 08-26 | Entry Point commands; LAUNCH_ACTIVITY callback | F |
| 08-28 | Entitlement migration; Subscription API and events | BRK, EP, GW |
| 09-04 | Polls in Edit Original Interaction Response | F |
| 09-17 | DAVE E2EE voice opcodes | GW |
| 09-20 | Soundboard API and events | EP, GW |
| 09-26 | Activities GA; Get Activity Instance | EP |
| 10-25 | Webhook Events (HTTP) | new transport |
| 12-12 | Multiple subscription tiers | DOC |
| 12-16 | Upload limit 25 to 10 MiB (from 2025-01-16) | BRK |

## 2025

| Date | Change | Tags |
|---|---|---|
| 04-03 | Per-attachment upload limits; `attachment_size_limit` | F |
| 04-15 | Create Guild by apps deprecated (removed 07-15) | DEP |
| 04-22 | Components V2 (types 9..14, 17; flag 1<<15) | F |
| 04-29 | Component limit 40 in total | F |
| 06-25 | Paginated pins endpoints; old pin routes deprecated | EP, DEP |
| 07-02 | Gradient role `colors`; guild tags `primary_guild` | F, DEP |
| 07-28 | Create Guild removed | BRK |
| 08-14 | Request Guild Members limit; RATE_LIMITED dispatch (rollout 10-01) | GW, BRK |
| 08-20 | PIN_MESSAGES split | BRK |
| 08-25 | Label component; selects in modals; Action Row in modals deprecated | F, DEP |
| 09-02 | Non-E2EE voice ends 2026-03-01 | BRK |
| 09-10 | Modify Current Member: banner, avatar, bio | F |
| 09-10 | All selects and Text Display in modals | F |
| 10-15 | File Upload component | F |
| 11-20 | CREATE_GUILD_EXPRESSIONS, CREATE_EVENTS required from 2026-02-23 | BRK |
| 11-24 | BYPASS_SLOWMODE split | F |
| 12-09 | Get Guild Role Member Counts | EP |

## 2026

| Date | Change | Tags |
|---|---|---|
| 01-13 | Community invites: target users, invite roles, job status | EP, F |
| 02-05 | Target-user CSV header fixed to `user_id`; partial roles | BRK |
| 02-10 | Docs moved to Mintlify; repo folder `docs/` became `developers/` | DOC |
| 02-12 | Radio Group, Checkbox Group, Checkbox components | F |
| 02-20 | Lobby moderation metadata endpoints | EP |
| 03-03 | Context-menu command limit 5 to 15 per type | behavior |
| 03-19 | Search Guild Messages; error 110000 | EP |
| 04-14 | Forwarding requires content access (160014) | BRK |
| 04-20 | Voice channel status: endpoint, permission bit 48, op 43, events, audit 192/193 | EP, GW, F |
| 05-05 | `premium_type` needs `identify.premium` | BRK |
| 05-07 | Application `flags_new` | F |
| 06-03 | Lobby endpoints | EP |
| 06-10 | Privileged intent review at 10k users, yearly | policy |
| 06-24 | Attachment edits: `is_spoiler`, attachment request object | F |
| 07-07 | Riot and LoL connections removed | BRK |
| 07-16 | Resolved channel `app_permissions` | F |
| 08-05 | Channel `application_id` nullable | BRK |
| 08-05 | `file_types` filter | F |
| 08-12 | Channel obfuscation; mandatory 2026-11-16 | BRK, GW |
| 08-14 | Battle.net connections removed | BRK |
| 08-26 | Application Identity Profile API | EP |
| 08-27 | PRUNE_REQUIRES_ADMIN feature | BRK |
| 09-03 | Upload limit 10 to 20 MiB | behavior |
| 09-15 | `shard` required on `GET /users/@me/guilds` for large bots | BRK |
| 09-17 | Custom status hidden by profile privacy | GW |
| 09-18 | Invite target-user single and bulk endpoints | EP |
| 10-05 | Embed type `components` | F |
| 10-14 | OAuth2 responses include `iss` | F |
| 10-28 | Content scan flags | F |
| 10-29 | Audit log position updates and entry merging | F |
| 10-29 | SET_VOICE_CHANNEL_STATUS without CONNECT when connected | behavior |
| 10-30 | Guild and channel permission endpoints | EP |

Changes in the docs with no change log entry:

- ACTIVE_DEVELOPER user flag removed (2026-03-31).
- `shared_client_theme`, member collectibles (2026-03-17).
- Attachment clip fields and flags (2026-04-03).
- MESSAGE_CREATE `channel_type` (2026-06-03).
- Pin cap 250 (2026-06-24).
- IS_SPOILER_CHANNEL (2026-07-16).
- STREAM_REQUEST activity type, VR client status (2026-08-03).
- DEFERRED_UPDATE_MESSAGE and UPDATE_MESSAGE allowed for MODAL_SUBMIT (2026-09-21).
- Error codes 160009 and 160010 (2026-09-12).
- IS_COMPONENTS_V2 allowed on forum thread start (2026-11-02).

## Announced, not yet in effect

1. **2026-11-16: channel obfuscation is mandatory.** The cache and the permission calculator must handle it.
2. Planned for a future API version (no date):
   - `message_reference.type` becomes required.
   - Start Thread without Message `type` becomes required.
   - `enforce_nonce` behavior becomes the default.
3. Deprecated, still working, no removal date:
   - Old pin routes, role `color`, message `interaction` and `stickers`.
   - Command `dm_permission`, PREMIUM_REQUIRED callback.
   - Action Row + Text Input in modals.
   - Stage PUBLIC privacy, guild `region`, IS_REMIX flag.
4. GUILD_MEDIA (16) is still in development.
5. No API v11 is announced. v10 is current.
