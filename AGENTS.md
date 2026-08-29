# AI Agent Instructions

This file is for coding agents. User-facing project information belongs in
`README.md`; contributor workflow belongs in `CONTRIBUTING.md`.

## Before Editing

Read `README.md`, `docs/design.md`, `docs/status.md`, and `docs/roadmap.md`.
Read `ui/app-window.slint` only for UI work. Preserve unrelated uncommitted
changes and never overwrite files under `scratch/save-format/samples/`.

## Product Invariants

- `Current` is the single writable, account-named `<username>.dat` used by the
  game. Other `.dat` files in the native directory are ignored backups.
- `StoredSave` is immutable; `Preset` and `Stash` are its classifications.
- Every Apply captures Current as a Stash first. Applying never changes the
  source StoredSave; capturing leaves Current in place.
- Promoting a Stash changes metadata only. Duplicate content is allowed and
  receives a hash-based warning.
- Treat save bytes as opaque. Do not edit unknown offsets or generate saves.

## Safety Invariants

- Discover the Windows Documents known folder followed by
  `EA Games\Mirror's Edge\TdGame\Savefiles\`; do not assume a physical path.
- Block mutations while `MirrorsEdge.exe` runs or another manager holds the
  application lock.
- Recheck the process and Current fingerprint immediately before replacement.
- Stage and verify the replacement beside Current, retain rollback data, and
  keep a journal until verification and cleanup commit the operation.
- Never delete the only verified Current. Startup recovery must block when
  fingerprints or artifacts are contradictory.

## Storage and Resources

- User data belongs under `%LOCALAPPDATA%\Mirror's Edge Save Manager\`.
- Keep one compressed payload and one schema-versioned metadata file per
  StoredSave. Do not add deduplication or compatibility behavior without a
  concrete requirement.
- Built-in resources are read-only embedded assets. Preserve their exact
  fingerprints and provenance in `resources/built-in/NOTICE.md`.

## Implementation Rules

- Keep filesystem and transaction behavior in Rust modules; keep Slint bindings
  thin and mark user-visible strings with `@tr()`.
- Keep `README.md` and `README.zh-CN.md` synchronized when changing user-facing
  documentation; compare headings, workflows, features, and safety guidance.
- Validate paths, identifiers, aliases, metadata, and journal schemas at the
  boundary. Make failures explicit and actionable.
- Add focused tests for discovery, hashing, duplicate warnings, locks, staging,
  replacement, rollback, and startup recovery.
- Use `apply_patch` for edits. Do not commit, branch, reset, or create copied
  executable/package directories unless explicitly requested.

## Verification

Run the relevant checks before handoff:

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

For UI, translation, build-resource, or icon changes, run the built executable
and verify the visible result. If build artifacts appear stale, clean only this
package with `cargo clean -p mirrors-edge-save-manager` and rebuild.

Use Conventional Commits for new commits when the user explicitly requests a
commit. Do not rewrite existing history.
