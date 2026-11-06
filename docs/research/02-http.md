# Research: HTTP (REST) API

Research date: 2026-11-05. Source: `discord/discord-api-docs` at commit `da52492` (2026-11-02), folder `developers/`.
The full endpoint list is in [endpoints.txt](endpoints.txt). A script extracted it from every `<Route>` tag.

## 1. Reference basics (`developers/reference.mdx`)

- Base URL: `https://discord.com/api/v10`. A request without a version goes to v6. Always send `/v10`.
- Versions: v10 and v9 available. v8, v7, v6 deprecated. v5, v4, v3 discontinued.
- Authentication: `Authorization: Bot <token>` or `Bearer <token>`. `Basic` only on OAuth2 token endpoints.
- User-Agent: `DiscordBot ($url, $versionNumber)`. A bad value can cause a Cloudflare block (error 40333).
- Snowflakes:
  - Sent as JSON strings.
  - Timestamp = `(id >> 22) + 1420070400000`.
  - Worker = bits 21..17. Process = bits 16..12. Increment = bits 11..0.
  - Some IDs can arrive as JSON integers (for example `not_found` in GUILD_MEMBERS_CHUNK). Accept both forms.
- Field rules: `field?` means optional (absent). `?T` means nullable. `field?: ?T` means both. Model three states.
- Events can arrive 0, 1, or N times. Make handlers idempotent.
- Query strings: booleans as `true`/`false`. Arrays repeat the key.
- Content-Type: only `application/json`, `application/x-www-form-urlencoded`, or `multipart/form-data`.
- `X-Audit-Log-Reason`: 1..512 characters, URL-encoded UTF-8. It replaces the old `reason` param.
- File uploads:
  - `multipart/form-data` with `files[n]` parts and a `payload_json` part.
  - `attachments` array entries `{id: n, filename, description, is_spoiler}`. `id` matches `files[n]`.
  - Embeds can refer to `attachment://filename`.
  - On PATCH, only listed attachments stay. Since 2026-06-24, `description` and `is_spoiler` of existing attachments can change.
  - Size limit per file: 20 MiB default (since 2026-09-03). Boosts can raise it. Interactions carry `attachment_size_limit`.
  - File type filters (2026-08-05) for File Upload components and ATTACHMENT options: up to 10 entries, groups or `.ext`.
- Signed attachment URLs: `?ex=&is=&hm=`. Discord refreshes them in embeds and webhook avatars.
- Image data: `data:image/{jpeg|png|gif};base64,...`.
- Message formatting tokens include `<@$GAME_ID>` and timestamp styles `s` and `S`.

## 2. Rate limits (`developers/topics/rate-limits.mdx`)

Headers:

| Header | Notes |
|---|---|
| X-RateLimit-Limit | |
| X-RateLimit-Remaining | |
| X-RateLimit-Reset | Epoch seconds, with decimals |
| X-RateLimit-Reset-After | Seconds, with decimals. Use this one (no clock drift). |
| X-RateLimit-Bucket | Opaque. Does not include major IDs. |
| X-RateLimit-Global | On 429 only |
| X-RateLimit-Scope | On 429 only: `user`, `global`, `shared` |
| Retry-After | Seconds |

- 429 body: `{message, retry_after (float s), global, code?}`.
- Major params: only `channel_id`, `guild_id`, `webhook_id` (+ `webhook_token`).
- Limits can depend on the HTTP method. Routes can share a bucket. Do not hard-code limits.
- Global: 50 requests/s per bot. Interaction endpoints are exempt.
- Invalid requests: 10,000 per 10 minutes per IP. 401, 403, and 429 count. 429 with scope `shared` does not count.
- Emoji routes: per guild, and quota headers can be wrong.
- Some limits arrive as JSON error codes: 20016 slowmode, 20028/20029 write limits, 30046 old message edits, 40062, 110000 search.

Client model:

1. Route key = method + route template + major param values.
2. Map route key to the bucket hash from `X-RateLimit-Bucket`.
3. Store bucket state under bucket hash + major params.
4. Before the hash is known, run one request at a time on a temporary bucket.
5. Update `remaining` and reset time from `Reset-After`.
6. Add a global token bucket (50/s, configurable). Skip interaction routes.
7. On 429: `global` pauses all; `user` pauses the bucket; `shared` waits but is not a client fault.
8. Count 401/403/429 against the 10,000 budget. Treat 401 as fatal for the token.
9. Remove idle buckets after reset.

## 3. Errors

- Body: `{code, message, errors?}`. `errors` mirrors the request shape. Array indices are string keys.
- Leaves: `{"_errors": [{code: "BASE_TYPE_REQUIRED", message}]}`. Whole-request errors at `errors._errors`.
- HTTP codes: 200, 201, 204, 304, 400, 401, 403, 404, 405, 429, 502 (retry), 5xx.
- 245 JSON error codes:

| Range | Count | Meaning |
|---|---|---|
| 0 | 1 | General |
| 10xxx | 47 | Unknown resource |
| 20xxx | 13 | Action limits |
| 30xxx | 35 | Maximums reached |
| 40xxx | 26 | Auth and other |
| 50xxx | 73 | Invalid or no permission |
| 60003 | 1 | 2FA required |
| 80004 | 1 | No user with that tag |
| 9000x | 2 | Reactions |
| 11000x | 2 | Index or app not ready |
| 130000 | 1 | Resource overloaded |
| 150006 | 1 | Stage already open |
| 16000x | 8 | Threads and references |
| 17000x | 7 | Stickers |
| 18000x | 2 | Scheduled events |
| 20000x | 2 | AutoMod blocked |
| 22000x | 4 | Forum webhooks |
| 240000 | 1 | Harmful links |
| 340015 | 1 | Join limit |
| 35000x | 2 | Onboarding |
| 400001 | 1 | Uploads limited |
| 500000 | 1 | Ban failed |
| 52000x | 6 | Polls |
| 53000x | 7 | Social SDK / OIDC |

- Notable: 10003, 10008, 10062 (unknown interaction), 40060 (already acknowledged), 40005, 50001, 50013, 50035, 50045, 30034, 160014.

## 4. OAuth2 (`developers/topics/oauth2.mdx`)

- Authorize: `https://discord.com/oauth2/authorize`.
- Token: `https://discord.com/api/oauth2/token`. Revoke: `.../token/revoke`. Form encoding only.
- Discovery: `/.well-known/oauth-authorization-server` and `/.well-known/openid-configuration`.
- Flows:
  - Authorization code. Params `response_type=code, client_id, scope, state, redirect_uri, prompt, integration_type`. Redirect has `code`, `state`, `iss` (RFC 9207, 2026-10-14).
  - Refresh: `grant_type=refresh_token`.
  - Implicit: discouraged. Use code + PKCE.
  - Client credentials: Basic auth. Team apps limited to `identify` and `applications.commands.update`.
  - Bot authorization: `scope=bot`, `permissions`, `guild_id`, `disable_guild_select`, `integration_type`.
  - Advanced bot authorization: bot + other scopes. Token response has a `guild`.
  - Webhook: `scope=webhook.incoming`. Token response has a `webhook`.
  - PKCE (S256): implied by SDK guides. Not in the core parameter table. Keep it optional.
  - Device code grant: Social SDK guides only.
- Token response: `{access_token, token_type, expires_in (604800), refresh_token, scope}`.
- Scopes (29): `activities.read`, `activities.write`, `applications.builds.read`, `applications.builds.upload`, `applications.commands`, `applications.commands.update`, `applications.commands.permissions.update`, `applications.entitlements`, `applications.store.update`, `bot`, `connections`, `dm_channels.read`, `email`, `gdm.join`, `guilds`, `guilds.join`, `guilds.members.read`, `identify`, `identify.premium` (2026-05), `messages.read`, `relationships.read`, `role_connections.write`, `rpc`, `rpc.activities.write`, `rpc.notifications.read`, `rpc.voice.read`, `rpc.voice.write`, `voice`, `webhook.incoming`.
- Other scopes: `sdk.social_layer`, `sdk.social_layer_presence`, `application_identities.write`.
- Endpoints: `GET /oauth2/applications/@me`, `GET /oauth2/@me`.

## 5. Permissions (`developers/topics/permissions.mdx`)

- Serialized as decimal strings. Use `u64` internally. Application `flags_new` is a string (2026-05).

| Bit | Name | Bit | Name |
|---|---|---|---|
| 0 | CREATE_INSTANT_INVITE | 27 | MANAGE_NICKNAMES |
| 1 | KICK_MEMBERS | 28 | MANAGE_ROLES |
| 2 | BAN_MEMBERS | 29 | MANAGE_WEBHOOKS |
| 3 | ADMINISTRATOR | 30 | MANAGE_GUILD_EXPRESSIONS |
| 4 | MANAGE_CHANNELS | 31 | USE_APPLICATION_COMMANDS |
| 5 | MANAGE_GUILD | 32 | REQUEST_TO_SPEAK |
| 6 | ADD_REACTIONS | 33 | MANAGE_EVENTS |
| 7 | VIEW_AUDIT_LOG | 34 | MANAGE_THREADS |
| 8 | PRIORITY_SPEAKER | 35 | CREATE_PUBLIC_THREADS |
| 9 | STREAM | 36 | CREATE_PRIVATE_THREADS |
| 10 | VIEW_CHANNEL | 37 | USE_EXTERNAL_STICKERS |
| 11 | SEND_MESSAGES | 38 | SEND_MESSAGES_IN_THREADS |
| 12 | SEND_TTS_MESSAGES | 39 | USE_EMBEDDED_ACTIVITIES |
| 13 | MANAGE_MESSAGES | 40 | MODERATE_MEMBERS |
| 14 | EMBED_LINKS | 41 | VIEW_CREATOR_MONETIZATION_ANALYTICS |
| 15 | ATTACH_FILES | 42 | USE_SOUNDBOARD |
| 16 | READ_MESSAGE_HISTORY | 43 | CREATE_GUILD_EXPRESSIONS |
| 17 | MENTION_EVERYONE | 44 | CREATE_EVENTS |
| 18 | USE_EXTERNAL_EMOJIS | 45 | USE_EXTERNAL_SOUNDS |
| 19 | VIEW_GUILD_INSIGHTS | 46 | SEND_VOICE_MESSAGES |
| 20 | CONNECT | 47 | (not assigned) |
| 21 | SPEAK | 48 | SET_VOICE_CHANNEL_STATUS |
| 22 | MUTE_MEMBERS | 49 | SEND_POLLS |
| 23 | DEAFEN_MEMBERS | 50 | USE_EXTERNAL_APPS |
| 24 | MOVE_MEMBERS | 51 | PIN_MESSAGES (2025-08) |
| 25 | USE_VAD | 52 | BYPASS_SLOWMODE (2025-11) |
| 26 | CHANGE_NICKNAME | | |

- Splits enforced since 2026-02-23: PIN_MESSAGES, CREATE_GUILD_EXPRESSIONS, CREATE_EVENTS, BYPASS_SLOWMODE.
- Computation:
  1. Owner gets all.
  2. Base = @everyone role OR each member role.
  3. ADMINISTRATOR gives all.
  4. Apply the @everyone overwrite (`&= !deny; |= allow`).
  5. OR all role overwrites of the member, then apply them.
  6. Apply the member overwrite.
- Implicit rules:
  - No VIEW_CHANNEL removes all.
  - No SEND_MESSAGES removes MENTION_EVERYONE, SEND_TTS_MESSAGES, ATTACH_FILES, EMBED_LINKS.
  - No CONNECT removes other voice permissions. A connected user keeps SET_VOICE_CHANNEL_STATUS (2026-10-29).
  - Threads inherit the parent. Sending needs SEND_MESSAGES_IN_THREADS.
  - Timed-out members keep only VIEW_CHANNEL and READ_MESSAGE_HISTORY.
- Server-side computed permissions (2026-10-30): `GET /guilds/{g}/permissions/{member|role}/{id}`, `GET /channels/{c}/permissions/{member|role}/{id}` (`implicit` param).
- Hierarchy: a bot can only act on roles and members below its highest role.
- Role: `color` deprecated. `colors {primary_color, secondary_color?, tertiary_color?}` (2025-07). `flags` (IN_PROMPT = 1<<0).
- Role tags: for `premium_subscriber`, `available_for_purchase`, `guild_connections`, null means true and absent means false.

## 6. CDN (`developers/reference.mdx#image-formatting`)

- Base: `https://cdn.discordapp.com/`. Format by extension. `?size=` power of two, 16..4096.
- Formats: JPEG, PNG, WebP, GIF, Lottie. Animated hashes start with `a_`. Prefer `.webp?animated=true`.

| Asset | Path |
|---|---|
| Custom emoji | `emojis/{emoji_id}.{ext}` |
| Guild icon | `icons/{guild_id}/{hash}` |
| Guild splash | `splashes/{guild_id}/{hash}` |
| Discovery splash | `discovery-splashes/{guild_id}/{hash}` |
| Guild banner | `banners/{guild_id}/{hash}` |
| User banner | `banners/{user_id}/{hash}` |
| Default avatar | `embed/avatars/{index}.png`, index = `(id >> 22) % 6` or `discriminator % 5` |
| User avatar | `avatars/{user_id}/{hash}` |
| Member avatar | `guilds/{g}/users/{u}/avatars/{hash}` |
| Member banner | `guilds/{g}/users/{u}/banners/{hash}` |
| Avatar decoration | `avatar-decoration-presets/{asset}.png` |
| Application icon / cover | `app-icons/{app_id}/{hash}` |
| Application asset | `app-assets/{app_id}/{asset_id}` |
| Achievement icon | `app-assets/{app_id}/achievements/{ach_id}/icons/{hash}` |
| Store asset | `app-assets/{app_id}/store/{asset_id}` |
| Sticker pack banner | `app-assets/710982414301790216/store/{asset_id}` |
| Team icon | `team-icons/{team_id}/{hash}` |
| Sticker | `stickers/{id}.{png|json|gif}` (GIF from `media.discordapp.net`) |
| Role icon | `role-icons/{role_id}/{hash}` |
| Scheduled event cover | `guild-events/{event_id}/{hash}` |
| Guild tag badge | `guild-tag-badges/{guild_id}/{hash}` |
| Soundboard sound | `soundboard-sounds/{sound_id}` |

- Upload limits: emoji 256 KiB; sticker 512 KiB; sound 512 KB and 5.2 s.

## 7. Endpoint groups

245 documented operations. See [endpoints.txt](endpoints.txt).

| Group | Count | Notes |
|---|---|---|
| Application | 3 | |
| Application Identity Profile | 5 | New 2026-08. Social SDK. |
| Application Role Connection Metadata | 2 | |
| Application Commands | 16 | Permissions PUT needs Bearer |
| Audit Log | 1 | New action types 16, 33 |
| Auto Moderation | 5 | |
| Channel | 26 | Computed permissions (2026-10), voice status |
| Emoji | 10 | Guild and application emojis |
| Entitlement | 5 | |
| Guild | 47 | Create Guild removed for apps (2025-07) |
| Guild Scheduled Event | 6 | |
| Guild Template | 6 | |
| Interaction responses | 8 | Exempt from the global limit |
| Invite | 9 | Target users (CSV) 2026 |
| Lobby | 16 | Social SDK. Bot and Bearer. |
| Message | 20 | Search (2026-03). New pin routes (2025-06). Old pin routes deprecated. |
| OAuth2 | 2 | |
| Poll | 2 | |
| SKU | 1 | |
| Soundboard | 7 | |
| Stage Instance | 4 | |
| Sticker | 8 | |
| Subscription | 2 | |
| User | 12 | `/users/@me/guilds` needs `shard` for large bots |
| Voice | 5 | |
| Webhook | 15 | |
| Gateway | 2 | |

## Open points

- PKCE enforcement for confidential clients.
- Nameplate CDN path.
- AVIF as output format.
- Whether Delete Guild and Modify MFA Level still work.
- Whether Batch Edit Command Permissions is still disabled.
