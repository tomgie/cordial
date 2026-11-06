# Contributing

## Rules

- Follow the language and code rules in [AGENTS.md](AGENTS.md).
- Follow the structure in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
- Mark a roadmap item `[x]` in the commit that completes it.
- Increase the version as [AGENTS.md](AGENTS.md) tells you.

## Checks

Run these before you push. CI runs the same checks.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
cargo deny check
```

## Discord API facts

- Put each new fact about the Discord API in the correct file in [docs/research/](docs/research/).
- Give the source. Mark community sources as **[UNOFFICIAL]**.
- When a live test gives a different result than the docs, record both.

## Public API rules

The API review of version 0.8.0 set these rules. `crates/cordial/tests/api.rs` checks some of them at compile time.

| Rule | Reason |
|---|---|
| A public struct with public fields, and a public enum, has `#[non_exhaustive]`. | Discord adds fields and values. A new field must not break users. |
| An enum for a Discord value has an `Unknown` variant (`discord_enum!`, `discord_string_enum!`). | Old versions of the library must decode new values. |
| Each public error type is `Error + Send + Sync + 'static`. | Users return errors with `?` across tasks. |
| Each handle (`Client`, `ShardHandle`, `Cache`, `Context`, `Call`) is `Send + Sync` and cheap to clone. | Handlers run in many tasks. |
| No `get_` prefix on methods. A getter has the name of the value. | The Rust API guidelines (C-GETTER). |
| A token or other secret uses `Token` or `Secret`, never a plain `String`. | `Debug` output goes to logs. |
| Traits are open: users can implement `Transport`, `IdentifyQueue`, `E2eeSession`, `AudioSource`, `CommandArg`, `Localizer`, and `Handler`. No trait is sealed. | These traits are extension points. A trait that users must not implement needs a sealed supertrait. |
