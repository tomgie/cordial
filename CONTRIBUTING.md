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
