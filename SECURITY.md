# Security

## Report a vulnerability

Do not open a public issue for a vulnerability. Send the details to the maintainers in private. Give the affected crate, the version, and the steps to reproduce.

## Security review (version 0.7.0)

The table gives the areas of the review and the result.

| Area | Result |
|---|---|
| Bot token in memory | `cordial_http::Token` clears its memory on drop (`zeroize`). `Debug` and `Display` show `<redacted>`. |
| Tokens in model types | Interaction, webhook, voice server, Identify, and Resume tokens use `cordial_model::Secret`. Its `Debug` shows `<redacted>`. Fixed in 0.7.0: before, `Debug` of `Identify` showed the bot token. |
| Tokens in URLs | Webhook and interaction routes put a token in the path. Logs use the route template, never the path. The `Debug` of `Route` shows the template for these routes. Transport errors drop the URL. Fixed in 0.7.0: before, a `reqwest` error showed the full URL. |
| Tokens in rate limit keys | The limiter keys webhook routes by a hash of the token, not by the token. |
| Request signatures (HTTP interactions) | Ed25519 with `verify_strict` (rejects weak keys and non-canonical signatures). The timestamp check (default 5 minutes) limits replays. A request with a missing or bad header gets status 401 before the body is decoded. |
| Request size (HTTP interactions) | The `axum` adapter keeps the default body limit of `axum` (2 MB). Interaction bodies are much smaller. |
| Gateway messages | One message, compressed or decompressed, has a limit of 256 MiB (`MAX_MESSAGE_SIZE`). Before 0.7.0 the zlib inflater kept frames without limit when the end marker did not come. A larger message closes the connection, and the shard reconnects. |
| JSON depth | `serde_json` stops at a depth of 128. |
| Voice packets | The receive buffer is 2048 bytes. The parsers check all lengths before they read. Fuzz targets cover the packet, RTP, Ogg, and DAVE frame parsers. |
| DAVE | Our own OpenMLS provider for ciphersuite 2 only, on `hpke` 0.15 and `p256` 0.14. Known-answer tests from RFC 5869, RFC 6979, and RFC 9180. AES-GCM tags are compared in constant time. Replayed frames are rejected. |
| Long-running memory | The rate limiter removes idle buckets. Cooldowns remove old entries. Collectors remove closed waiters. The cache stores no data for guilds that it does not keep. |
| Dependencies | `cargo deny check` runs in CI: licenses, advisories, bans, and sources. |

## Known limits

- The live service is not part of the tests. The DAVE layer needs a test with Discord clients.
- `Secret` does not clear its memory on drop. Interaction and voice tokens are short-lived.
- A handler that logs a full `Interaction` with `Debug` still shows user data, such as message content. Log only the fields that you need.
