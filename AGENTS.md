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
- Every Apply first ensures a verified StoredSave exists for Current, creating
  a Stash only when no identical verified Preset or Stash already exists.
  Applying never changes the source StoredSave; capturing leaves Current in
  place.
- Prevent new duplicate Presets within Presets, and prevent a new Stash when any
  verified StoredSave already preserves the same content. Promotion is a
  metadata-only no-op when an identical verified Preset already exists.
- Do not open capture metadata or Apply confirmation when the requested content
  already exists in the destination or already equals Current. Keep final
  storage and transaction checks authoritative after UI preflight.
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
- Add focused tests for discovery, hashing, duplicate prevention, locks, staging,
  replacement, rollback, and startup recovery.
- Use `apply_patch` for edits. Do not commit, branch, reset, or create copied
  executable/package directories unless explicitly requested.

## Verification

During routine development and small UI iterations, use the debug build and
debug executable for logic and visible-result verification. Do not rebuild the
release profile for every incremental change. Reserve the full sequence below,
including `cargo build --release`, for release checkpoints, packaging changes,
or an explicit user request:

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

For UI, translation, build-resource, or icon changes, run the built executable
and verify the visible result. Use the debug executable during normal iteration.
If build artifacts appear stale, clean only this package with
`cargo clean -p mirrors-edge-save-manager` and rebuild.

Use Conventional Commits when the user explicitly requests a commit. Every
agent-created commit must include a non-empty body after the subject. The body
must explain the motivation, summarize the major changes, and record relevant
verification. Subject-only commits are prohibited unless the user explicitly
requests one. Do not rewrite existing history unless the user explicitly asks
to amend the latest commit.
