# Agent Guide

## Project Context

Mirror's Edge Save Switcher is a Windows Rust application for safely storing
and replacing the original Mirror's Edge PC save file. The repository began as
a Slint, FemtoVG, Windows packaging, and binary-size prototype. The current UI
is not the final product design.

Read these files before making changes:

- `README.md` for repository context and development commands.
- `docs/status.md` for the current milestone, verification state, and next task.
- `docs/design.md` for the current product and domain decisions.
- `docs/roadmap.md` for the ordered path to a complete first version.
- `ui/app-window.slint` only when working on the existing prototype UI.

## Design-First Workflow

- Treat `docs/design.md` as the source of truth for product behavior.
- Discuss and update the design before implementing behavior that is not
  already specified.
- Keep unresolved decisions in the document's Open Decisions section instead
  of silently choosing behavior with data-loss implications.
- Do not modify the Slint prototype UI while implementing the storage domain,
  save discovery, or transaction engine.
- Do not begin dynamic save-file generation until the format research has
  identified the relevant fields and integrity checks.

## Domain Invariants

- `CurrentSave` is the one live, writable `.dat` file used by the game.
- `StoredSave` is an immutable saved copy that can be applied to Current.
- `Preset` and `Stash` are classifications of the same StoredSave concept, not
  separate binary formats or storage engines.
- Applying any StoredSave must automatically capture the existing Current as a
  Stash first.
- Applying a StoredSave never consumes or modifies the source StoredSave.
- Capturing Current leaves Current in place.
- Promoting a Stash to a Preset changes its classification without rewriting
  the save bytes.
- Stash entries are not automatically deleted in the first version.
- Duplicate content is allowed, with a hash-based warning.

## Save Discovery and Safety

- The native path is the Windows Documents known folder followed by:
  `EA Games\Mirror's Edge\TdGame\Savefiles\`.
- Do not assume Documents is physically under `%USERPROFILE%\Documents`.
- The first version expects exactly one `.dat` file in the save directory.
- Zero `.dat` files is a missing-Current state.
- More than one `.dat` file is ambiguous and must block replacement.
- Preserve the active filename when applying another save.
- Never delete the original Current before a verified replacement and rollback
  path exist.
- Detect `MirrorsEdge.exe` and block all mutating actions while it is running.
- Recheck both the process state and Current fingerprint immediately before
  replacement.
- Use an application-level lock so multiple switcher instances cannot mutate
  the same save concurrently.
- Keep an operation journal and make interrupted operations recoverable at
  startup.

The application must treat save bytes as opaque until format research proves
otherwise. Copying, hashing, compressing, decompressing, and restoring opaque
files is valid; changing unknown offsets is not.

## Storage Direction

The planned user data root is:

```text
%LOCALAPPDATA%\Mirror's Edge Save Switcher\
```

The current design favors one compressed payload and one metadata file per
StoredSave. Avoid adding content-addressed storage, deduplication, or other
abstractions unless implementation evidence shows they are necessary.

Metadata must include an explicit schema version. Hashes verify exact content;
filesystem timestamps do not identify a StoredSave.

Built-in saves may be shipped as compressed verified resources. The provided
samples are fixed at `9,134,256` bytes but compress to roughly 9--25 KiB. User
imports belong in LocalAppData. Hiding a built-in Preset must not destroy its
embedded source.

## Save Format Research

Sample files are under `savefile_examples/`. Current observations are recorded
in `docs/design.md`; they are not a complete format specification. Preserve
the original samples and never overwrite them.

Controlled before/after samples are required before attempting to generate the
one-star time-trial save. Format-generation experiments must remain separate
from the copy-and-restore path so a reverse-engineering failure cannot damage
user saves.

## Implementation Guidance

- Prefer small domain modules with explicit data flow over broad abstractions.
- Keep UI bindings thin; filesystem and transaction behavior belongs in Rust
  domain/application code.
- Use structured metadata serialization and validate all paths and identifiers
  at the boundary.
- Make failure states explicit and actionable for users.
- Add tests for missing or ambiguous discovery, hashing, duplicate warnings,
  process locks, staging failures, replacement failures, rollback, and startup
  transaction recovery.
- Do not add compatibility code without a concrete persisted-data or external
  consumer requirement.

## Verification

Run the focused checks relevant to the change:

```powershell
cargo fmt --check
cargo test
cargo build --release
```

For changes to `ui/app-window.slint`, also run `cargo run` and inspect the
prototype manually. Do not change the UI merely to make a domain test pass.

## Commit Messages

Use Conventional Commits for new commits:

```text
<type>[(scope)]: <imperative summary>
```

A body is optional for small, self-explanatory commits. Add a short body for
changes involving data safety, persisted formats, transaction behavior,
compatibility, or significant design decisions. Explain motivation, guarantees,
or important tradeoffs rather than repeating the changed-file list. Separate
the body from the summary with a blank line and normally keep it to one to three
sentences.

The scope is optional. Prefer the smallest useful scope, such as `discovery`,
`storage`, `safety`, or `ui`.

Examples:

```text
feat(storage): persist compressed stored saves
feat(safety): block mutations while the game is running
fix(storage): clean staged files after capture failure
test(storage): cover corrupted payload recovery
docs: update transaction design
chore: update development configuration
```

Use `feat` for user-visible capabilities, `fix` for defects, `test` for
test-only changes, `docs` for documentation-only changes, and `chore` for
maintenance. Do not rewrite already-pushed history solely to rename older
commits.
