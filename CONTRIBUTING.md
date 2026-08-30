# Contributing

Thanks for helping improve Mirror's Edge Save Manager. This is a Windows Rust
application for organizing and safely switching the original game's PC saves.

## Project Shape

The game keeps one live, account-named `.dat` file, called **Current** in the
application. Copies kept by the manager are **Stored Saves**:

- **Preset** is a reusable starting point.
- **Stash** is a backup or history entry, created automatically before Apply.

Presets and Stashes use the same immutable payload format. Save bytes are
opaque: the manager copies, hashes, compresses, restores, and verifies them;
it does not edit progress fields or generate new saves. Product behavior and
data-safety decisions are defined in [`docs/design.md`](docs/design.md).

## Development Setup

Use Windows 10 or 11 x64 with a current stable Rust toolchain:

```powershell
cargo run
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

## Safety Rules

- Test destructive operations with temporary save trees or separately backed-up
  profiles, never against the only copy of a real Current.
- Preserve the account-named Current filename when applying a save.
- Keep the game-process check, application lock, transaction journal, staging,
  rollback, and startup recovery intact for every mutating operation.
- Do not add format-editing or generation logic to the copy-and-restore path.

## UI and Localization

Filesystem and transaction work belongs in Rust modules; Slint bindings should
stay thin. Mark user-visible Slint strings with `@tr()` and add translations to
`translations/<locale>/`. When changing `.slint`, `.po`, embedded resources, or
`build.rs`, verify the visible result in the built executable, not only in
source or generated files.

Keep the Current-centered layout and its blocked-state guidance unless the
product design is updated first. Do not change UI merely to make a domain test
pass.

## Documentation

Keep user instructions in [`README.md`](README.md) and [`README.zh-CN.md`](README.zh-CN.md).
These two files are maintained as a synchronized pair: whenever one changes,
update the corresponding content in the other language in the same change.
Keep headings, feature descriptions, user workflows, built-in Preset details,
and safety guidance aligned while allowing natural language differences.
Update [`docs/design.md`](docs/design.md) when changing product behavior,
storage semantics, or safety guarantees. Update [`docs/status.md`](docs/status.md)
and [`docs/roadmap.md`](docs/roadmap.md) when milestone state changes.

## Verification

Run focused checks first, then the complete set before requesting review:

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

For an apparently stale incremental build, clean only this package and rebuild:

```powershell
cargo clean -p mirrors-edge-save-manager
cargo build
```

Run `target\debug\mirrors-edge-save-manager.exe` directly after resource or UI
changes. Do not create alternate package directories or copied executables.

## Releases

The maintainer release workflow, tag timing, package contents, Draft Release
inspection, and final publication command are documented in
[`docs/releasing.md`](docs/releasing.md). The automated workflow never publishes
a release without an explicit maintainer confirmation.

## Pull Requests

Describe the user-visible or safety behavior changed, list verification
commands, and call out Windows-only or manual acceptance steps. Use
Conventional Commit style for commit titles:

```text
<type>[(scope)]: <imperative summary>
```

Common types are `feat`, `fix`, `test`, `docs`, and `chore`. Add a short body
to every commit after a blank line. The body must explain why the change is
needed, summarize its major parts, and record relevant verification. Do not use
subject-only commits unless a maintainer explicitly requests one. Do not rewrite
already-pushed history.
