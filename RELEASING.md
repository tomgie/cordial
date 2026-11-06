# Releasing

All crates of the workspace have one version. Release them together.

## Before the release

1. Make sure that CI is green on the release commit.
2. Run the checks:

   | Command | Checks |
   |---|---|
   | `cargo test --workspace --all-features` | All tests. |
   | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Lints. |
   | `cargo deny check` | Licenses, advisories, and sources. |
   | `cargo xtask spec-check fixtures/spec/openapi.json --strict` | All Discord endpoints exist. |
   | `cargo xtask release-check` | One version everywhere, crate metadata, and the changelog. |

3. Update `fixtures/spec/openapi.json` from `discord/discord-api-spec`, and run `spec-check` again.
4. Read the Discord changelog since the last release. Add the changes that the library does not have yet to `docs/ROADMAP.md`.

## Version

Follow `AGENTS.md`:

- Correction or internal change: increase the patch number.
- New feature, or a change that breaks the public API: increase the minor number.
- Keep the major number at 0 until the first stable release.

Change the version in the root `Cargo.toml`: `workspace.package.version`, and each `cordial-*` entry in `workspace.dependencies`. Add a section `## <version>` to `CHANGELOG.md`.

## Publish

1. Run `cargo xtask release-check`. It prints the publish order: each crate comes after its dependencies.
2. For each crate in that order, run `cargo publish -p <crate> --dry-run`, and then `cargo publish -p <crate>`.
3. Wait until crates.io shows a crate before you publish the next crate.
4. Tag the commit: `git tag v<version>`, and push the tag.

## After the release

- Make sure that docs.rs built all crates.
- Start a new `## Unreleased` section in `CHANGELOG.md` only when the next change arrives.
