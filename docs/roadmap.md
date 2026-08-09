# Version One Roadmap

This document describes the path from the current storage-domain foundation to
a releasable first version. It sequences work and defines completion criteria;
`docs/design.md` remains the source of truth for product behavior and safety
requirements. `docs/status.md` records the current handoff state and immediate
next tasks.

## Version One Definition

Version one is complete when a Windows user can discover the native Mirror's
Edge save, keep verified Preset and Stash copies, safely apply any StoredSave,
and recover from an interrupted apply operation without silently losing the
previous Current save.

The release must also provide a production UI for the supported operations,
actionable failure states, verified built-in resources where licensing permits,
and a tested Windows release package.

Dynamic generation or editing of proprietary save data is not required for
version one.

## Phase 0: Product and Domain Design

Status: Complete

Deliverables:

- Define CurrentSave and immutable StoredSave semantics.
- Define Preset and Stash as classifications of StoredSave.
- Require an automatic Stash before every apply operation.
- Establish native save discovery and ambiguity behavior.
- Establish opaque-file validation, storage, and replacement safety rules.
- Separate save-format research from the copy-and-restore path.

Completion criteria:

- Core behavior and unresolved product decisions are recorded in
  `docs/design.md`.
- Repository guidance protects the domain and binary research invariants.

## Phase 1: Discovery and StoredSave Foundation

Status: Complete

Deliverables:

- Resolve Windows Documents and LocalAppData known folders.
- Discover the account-named Current without mutation while ignoring backup
  `.dat` files.
- Validate observed save size and calculate a complete SHA-256 fingerprint.
- Persist schema-versioned StoredSave metadata and gzip payloads.
- Verify compressed captures before committing them to the repository.
- List and verify StoredSave entries.
- Warn about duplicate content without rejecting it.
- Support Stash-to-Preset classification changes in the domain model.

Completion criteria:

- Invalid, incomplete, or corrupted captures are not committed.
- Discovery and repository behavior have focused automated tests.

## Phase 2: Mutation Safety Prerequisites

Status: Complete

Deliverables:

- Detect `MirrorsEdge.exe` through the Windows process snapshot API.
- Acquire a non-blocking Windows named mutex across manager instances.
- Combine process state and operation locking in one mutation guard.
- Report game-running, operation-in-progress, and platform failures separately.

Completion criteria:

- Every future mutating application operation can be scoped to one guard.
- Lock contention and process-name behavior have focused automated tests.

## Phase 3: Apply Transaction Specification

Status: Complete

Deliverables:

- Select and document the Windows atomic replacement API.
- Define the transaction journal schema and explicit state transitions.
- Define staging and rollback paths beside Current.
- Define the required file and directory flush order.
- Define cleanup ownership for staging, rollback, and journal files.
- Define startup recovery behavior for every journal state and fingerprint
  combination.
- Define the safe blocked state for an unrecognized or contradictory recovery
  situation.

Completion criteria:

- Every interruption point before commit has one documented recovery outcome.
- No state transition requires deleting the only verified Current copy.
- Remaining data-loss decisions are explicit Open Decisions rather than
  implementation assumptions.

## Phase 4: Safe Apply Engine

Status: Complete

Deliverables:

- Decompress the selected StoredSave into a same-directory staging file.
- Verify staged size and content hash before replacement.
- Acquire the mutation guard and capture Current as an automatic Stash.
- Write and flush the transaction journal before changing Current.
- Recheck the game process and Current fingerprint immediately before replace.
- Atomically replace Current while retaining rollback data.
- Preserve the active Current filename.
- Rediscover and verify the replacement before committing the transaction.
- Remove rollback and temporary files only after verified commit.

Completion criteria:

- Applying either a Preset or Stash leaves its StoredSave unchanged.
- Success produces a verified new Current and an automatic Stash of the old
  Current.
- Any ordinary failure before commit leaves the old Current available.

## Phase 5: Recovery and Fault Coverage

Status: Complete

Deliverables:

- Scan unfinished journals before allowing new mutations at startup.
- Restore or finish interrupted transactions only after fingerprint checks.
- Surface an actionable blocked state when automatic recovery is unsafe.
- Add controlled fault injection at staging, journal, replace, verify, rollback,
  and cleanup boundaries.
- Test Current changes between initial capture and replacement.
- Test process locks, multi-instance contention, replacement failures,
  rollback, and each startup recovery state.

Completion criteria:

- Forced interruption at each journal state has a deterministic tested result.
- Recovery never silently chooses between contradictory file contents.
- No new mutation starts while an unfinished transaction remains unresolved.

## Phase 6: Complete Application Operations

Status: Complete

Deliverables:

- Manually capture Current as a Stash.
- Save Current as a Preset.
- Import and validate an external `.dat` file.
- Persist Stash-to-Preset promotion without rewriting payload bytes.
- Edit StoredSave alias and description metadata.
- Finalize alias validation and default naming.
- Implement confirmed, account-named first activation when Current is missing,
  with crash-safe staging and recovery.
- Map domain and platform errors to actionable application states.

Completion criteria:

- All supported mutations use the same mutation guard and repository rules.
- Application operations are independent of Slint and have focused tests.
- A missing account-named Current cannot accidentally enter replacement even
  when backup `.dat` files exist.

## Phase 7: Built-in Presets

Status: Complete

Deliverables:

- Decide provenance and redistribution permission for each binary save.
- Define built-in resource identity, version, size, and SHA-256 metadata.
- Embed compressed verified resources when distribution is permitted.
- Implement hiding and restoring built-in Presets without destroying sources.
- Define hidden-state behavior when built-in resources change on upgrade.
- Verify each distributed save by loading it in the game.

Completion criteria:

- Every shipped binary resource has documented provenance and verification.
- Application upgrades do not silently duplicate, expose, or lose built-in
  visibility choices.
- A resource without clear distribution permission is not shipped.

## Phase 8: Production UI and Integration

Status: Planned

Deliverables:

- Replace the technical prototype with the Current-centered product design.
- Display Current discovery, fingerprint, game process, and recovery states.
- Present Preset and Stash collections backed by one StoredSave model.
- Expose capture, import, apply, promote, metadata edit, hide, and restore
  operations only when their application services are stable.
- Communicate both automatic-Stash and StoredSave-to-Current flows before apply.
- Keep filesystem work off the UI thread and keep the window responsive.
- Support desktop scaling and the required Windows 10/11 window sizes.

Completion criteria:

- Every mutation is disabled with a visible reason when blocked.
- Errors and recovery actions are understandable without inspecting logs.
- Manual inspection confirms correct behavior on supported scaling settings.

## Phase 9: Release Hardening

Status: Planned

Deliverables:

- Run end-to-end tests against a real Mirror's Edge save location.
- Test redirected and OneDrive-backed Documents folders.
- Test file sharing violations, antivirus delays, controlled-folder access, and
  insufficient permissions.
- Test multiple manager instances and forced termination during apply.
- Finalize application versioning, icons, Windows metadata, and packaging.
- Define persisted-data migration policy before changing any schema.
- Measure the final linked executable and startup behavior.
- Document user-facing backup expectations, known limitations, and licenses.

Completion criteria:

- Formatting, tests, strict Clippy, and release build pass from a clean checkout.
- Supported Windows versions pass the release acceptance checklist.
- Installation, upgrade, operation, recovery, and removal behavior are
  documented.
- No unresolved issue can plausibly cause silent save loss.

## Separate Save-Format Research

Save-format research is paused and does not block the version-one manager. Any
future generated Preset requires controlled evidence for its fields and
integrity checks; otherwise it is omitted without weakening capture, storage,
Apply, or recovery behavior.

## Future Save Tooling

After version one, evaluate a separate save-format tooling track covering the
complete `.dat` structure. The likely sequence is a read-only inspector,
field-level comparison, structural and integrity validation, and only then
narrowly scoped editing or generation where behavior is proven with controlled
samples. This work is distinct from the safe manager and must not modify the
opaque copy-and-restore path or introduce in-place editing of source saves.
