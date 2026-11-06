# Test Fixtures

## `docs/`

JSON examples from the Discord developer docs. `cargo xtask fixtures` makes them. Do not edit them by hand.

- Source: `discord/discord-api-docs`, folder `developers/`, commit `da52492` (2026-11-02).
- Each file is one `json` or `jsonc` code block. The name comes from the nearest heading.
- Blocks that are not valid JSON (for example with `...`) are not included.

To update:

1. Clone `https://github.com/discord/discord-api-docs`.
2. Run `cargo xtask fixtures <clone>/developers`.
3. Change the commit above.
