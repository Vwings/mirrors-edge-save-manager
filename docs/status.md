# Project Status and Handoff

Last updated: 2026-08-09

This is a temporary handoff document for development sessions. Update it when
a milestone changes, a major decision is made, or the next task changes. Remove
it when the first-version implementation is stable and normal issue tracking
has replaced it.

The complete path to the first release is maintained in `docs/roadmap.md`.
This document should contain only current implementation state, verification,
and immediate ordered work.

## Current Stage

The safe Apply engine, fingerprint-based startup recovery, controlled fault
coverage, complete application operations, and built-in Presets are complete.
All four bundled saves passed in-game verification, closing Phase 7. The next
stage is the production UI and application-layer integration; the domain code is
not connected to the Slint prototype yet.

## Completed

- Product, domain, safety, storage, and UI direction documented in
  `docs/design.md`.
- Repository guidance and data-safety invariants documented in `AGENTS.md`.
- Windows Documents and LocalAppData known-folder resolution.
- Read-only Current discovery for the Windows account-named `.dat`, ignoring
  unrelated backup `.dat` files.
- Fixed-size validation for the observed `9,134,256`-byte save format.
- SHA-256 fingerprinting over the complete opaque save bytes.
- Minimal `StoredSave` metadata model with Preset and Stash classifications.
- Stash-to-Preset promotion without rewriting save content.
- Gzip capture into a hidden staging directory.
- Decompression and fingerprint verification before StoredSave commit.
- Schema-versioned JSON metadata in LocalAppData-compatible storage.
- Duplicate-content warnings without rejecting duplicate entries.
- StoredSave listing and payload integrity verification.
- Read-only `MirrorsEdge.exe` detection through the Windows process snapshot
  API, with case-insensitive exact executable-name matching.
- Non-blocking, session-local Windows named mutex preventing concurrent save
  mutations across manager instances.
- One mutation guard that owns the operation lock and separately reports a
  running game, lock contention, and platform API failures.
- Apply transaction schema, explicit phases, Windows replacement API, durable
  update order, artifact naming, and fingerprint-based startup recovery matrix.
- Create-new, same-directory StoredSave staging with decompression, flush,
  complete fingerprint verification, and partial-file cleanup on failure.
- Safe Apply orchestration with automatic Stash capture, durable journal phase
  updates, process and Current rechecks, `ReplaceFileW`, rollback retention,
  replacement verification, and commit cleanup.
- Current discovery exclusion for exact application-owned transaction artifacts
  without hiding similarly named user `.dat` files.
- Tests for discovery, validation, hashing, capture, duplicate content,
  corrupted payloads, storage-path failures, process-name matching, operation
  lock contention, mutation blocking, staging safety, successful Apply, and a
  Current changed immediately before replacement.
- Startup scanning for unfinished apply journals under the application data
  root, guarded by the same process and operation lock as other mutations.
- Strict recovery journal validation for schema, operation, canonical UUIDs,
  native Current directory, and transaction-derived artifact paths.
- Fingerprint-based recovery for every safe state in the documented matrix,
  including missing-Current restoration, completed replacement cleanup, and
  completed rollback cleanup.
- Blocked recovery that preserves malformed journals, contradictory content,
  invalid artifacts, and unsupported states for diagnosis.
- Apply refusal while any unfinished journal remains unresolved.
- Tests for all seven automatic startup recovery states, contradictory artifact
  fingerprints, and malformed journals.
- Apply fault injection at journal publication, replacement, post-replacement
  verification, rollback, and commit cleanup boundaries.
- Immediate verified rollback when replacement validation fails, retaining a
  durable `RollingBack` journal until the original Current is restored.
- Startup recovery tests for injected journal-update, replacement, rollback,
  rollback-cleanup, and journal-cleanup failures.
- Manual Current-to-Stash capture as an application operation using the shared
  mutation guard, unfinished-transaction check, discovery rules, and verified
  StoredSave repository capture.
- Tests that manual capture leaves Current unchanged and reports missing and
  recovery-blocked states without creating a StoredSave.
- Current-to-Preset capture through the same guarded and verified Current
  capture path used by manual Stash creation.
- Guarded external `.dat` import as a Preset, including case-insensitive
  extension validation, fixed-size and payload verification, and duplicate hash
  warnings without rejection.
- Tests for Current-to-Preset capture, valid external import, duplicate imports,
  invalid extensions and content, and unfinished-transaction blocking.
- Atomic StoredSave metadata replacement using a flushed UUID-named temporary
  file and write-through rename after verifying the existing payload.
- Guarded Stash-to-Preset promotion and alias/description editing without
  rewriting compressed payload bytes.
- Tests for persisted promotion and metadata edits, exact payload-byte
  preservation, and unfinished-transaction blocking.
- Alias normalization requiring a non-empty value of at most 80 Unicode
  characters, with local timestamp defaults for Current captures and source-stem
  defaults for imports.
- Windows account-name discovery and strict filename-stem validation for a
  user-confirmed first-activation `<account>.dat` suggestion.
- Unit-test process-state injection so filesystem transaction tests remain
  deterministic while production builds retain real game-process checks.
- Current discovery now checks only the Windows account-named `<username>.dat`;
  unrelated `.dat` history and backup files are ignored even when Current is
  missing.
- Removed the obsolete ambiguous-Current state from discovery and application
  operations, with focused tests for account-named discovery and backup-only
  directories.
- Crash-safe first activation for a confirmed account-named Current, using a
  verified same-directory staging file, durable activation journal, create-only
  atomic move, and final fingerprint verification.
- Startup recovery for activation journals that finishes a staged activation,
  confirms an already-published Current, or aborts a transaction whose staging
  file was lost; contradictory states remain blocked.
- Tests that activation preserves unrelated backup `.dat` files, refuses an
  unconfirmed filename or existing Current, and recovers every safe activation
  state.
- Slint-independent application error classification for every supported
  operation, preserving concrete diagnostic errors while exposing stable
  operation and user-action values for future UI guidance.
- Conservative transaction guidance that distinguishes automatic recovery from
  manually blocked recovery without performing cleanup or retries during error
  conversion.
- Context-sensitive file guidance for invalid imports, changed Current saves,
  inaccessible paths, and damaged StoredSave data.
- Versioned, read-only embedded Presets for New Game, completed campaign, 69%
  speedrun, and a clean all-time-trials-unlocked save, with stable UUID
  identities, exact manifests, and complete decompression verification.
- Direct Apply support for embedded Presets through the same staging,
  automatic-Stash, replacement, and rollback path used by user StoredSaves.
- Schema-versioned hidden built-in state in `settings.json`, preserving hidden
  logical IDs across resource upgrades and temporary resource removal.
- Guarded hide and restore operations that cannot modify embedded bytes and are
  blocked by the same game, operation lock, and unfinished recovery rules as
  other mutations.
- Asset provenance notice recording unknown community origins, separate asset
  licensing status, exact hashes, and the maintainer's accepted distribution
  risk.
- In-game verification of New Game, completed campaign, 69% speedrun, and clean
  all-time-trials-unlocked Presets using their exact embedded fingerprints. All
  four loaded without corruption and presented their expected progress state;
  the all-time-trials resource had unlocked courses without existing PB or
  Ghost data.

## Last Verification

The following checks passed after the Save Manager rename, four-resource
built-in Preset update, and in-game verification record:

```text
cargo fmt --check
cargo test                         # 65 passed
cargo clippy --all-targets -- -D warnings
cargo build --release
```

The storage and process-detection code is not yet referenced by the GUI binary,
so the current release build is not the final linked application-size
measurement.

## Current Worktree Note

`scratch/` is intentionally ignored and contains local experiments and binary
research samples. Automated tooling must not overwrite those inputs. Only the
four reviewed compressed resources under `resources/built-in/` are
distribution assets; their fingerprints and provenance notes are in the
adjacent `NOTICE.md`.

## Next Tasks

Complete these in order unless the design document is updated first:

- [x] Add read-only detection for `MirrorsEdge.exe`.
- [x] Define one mutation guard that combines game-process state and the
  application-level operation lock.
- [x] Implement the application-level lock for concurrent manager instances.
- [x] Finalize the transaction journal states and Windows atomic replacement
  API in `docs/design.md`.
- [x] Decompress a selected StoredSave into a same-directory staging file.
- [x] Implement apply: capture Current as Stash, recheck safety conditions,
  replace Current, verify, and retain rollback data until commit.
- [x] Recover interrupted apply transactions at startup.
- [x] Add controlled fault injection for journal, replacement, verification,
  rollback, and cleanup boundaries.
- [x] Add tests for replacement failures and in-process rollback; process locks,
  changed Current fingerprints, staging failures, and startup recovery also have
  focused coverage.
- [x] Implement manual Current capture as a Stash through the shared mutation
  guard.
- [x] Implement saving Current as a Preset and importing an external save.
- [x] Persist Stash-to-Preset promotion and alias/description metadata edits
  without rewriting payload bytes.
- [x] Finalize alias validation, default naming, and missing-Current activation
  behavior before exposing those choices in application operations.
- [x] Specify and implement a crash-safe first-activation creation transaction
  before writing a missing Current.
- [x] Consolidate domain and platform failures into actionable application
  states before UI binding.
- [x] Decide built-in Preset resource versioning, provenance, and licensing.
- [x] Embed and fingerprint New Game, completed-campaign, 69% speedrun, and a
  clean all-time-trials-unlocked resource.
- [x] Implement read-only built-in listing, Apply, hiding, restoration, and
  upgrade-stable hidden state.
- [x] Load each bundled save in Mirror's Edge and verify the expected menus and
  progress before release.
- [x] Record the confirmed save-container findings and pause deeper format
  research; offline editing is outside version-one scope.
- [ ] Bind the stable application layer to a new UI design without modifying
  the existing prototype merely to expose incomplete domain work.

## Separate Research Track

Dynamic save generation is not part of version one. Deeper format research is
paused and remains isolated from capture, storage, and restore code. Preserve
the ignored samples and concise research notes so work can resume for a
concrete product requirement.

## Open Product Decisions

The authoritative list remains in `docs/design.md`. No current open decision
blocks the copy, restore, or UI integration paths.
