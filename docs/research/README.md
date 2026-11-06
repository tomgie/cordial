# Research Notes

Research date: 2026-11-05. These notes record the Discord API facts that the design uses.

## Sources

- Docs site: <https://docs.discord.com/developers/> (`discord.com/developers/docs` redirects there).
- Docs source: <https://github.com/discord/discord-api-docs>, folder `developers/`, commit `da52492` (2026-11-02).
- OpenAPI 3.1 specification (public preview): <https://github.com/discord/discord-api-spec>.
- DAVE protocol: <https://github.com/discord/dave-protocol>, <https://github.com/discord/libdave>.

## Files

| File | Topic |
|---|---|
| [01-gateway.md](01-gateway.md) | Gateway connection, opcodes, close codes, intents, events, sharding |
| [02-http.md](02-http.md) | HTTP basics, rate limits, errors, OAuth2, permissions, CDN, endpoint groups |
| [endpoints.txt](endpoints.txt) | All 245 documented HTTP operations |
| [03-interactions.md](03-interactions.md) | Interactions, commands, components, modals, monetization, application |
| [04-voice.md](04-voice.md) | Voice gateway, UDP, encryption, DAVE |
| [05-resources.md](05-resources.md) | Data objects: enums, flags, partial forms, limits |
| [06-changelog.md](06-changelog.md) | API changes 2024-2026 and announced changes |

## Rules

- Mark facts from community sources as **[UNOFFICIAL]**.
- Keep an "Open points" list at the end of each file.
- Update a file when a live test gives a different result.
