# Project Status and Handoff

Last updated: 2026-07-30

This is a temporary handoff document for development sessions. Update it when
a milestone changes, a major decision is made, or the next task changes. Remove
it when the first-version implementation is stable and normal issue tracking
has replaced it.

The complete path to the first release is maintained in `docs/roadmap.md`.
This document should contain only current implementation state, verification,
and immediate ordered work.

## Current Stage

The safe Apply engine is complete, including automatic Stash capture, durable
journaling, Current rechecks, atomic Windows replacement, verification, and
commit cleanup. Startup transaction recovery and fault coverage are the current
stage. The domain code is not connected to the Slint prototype yet.

## Completed

- Product, domain, safety, storage, and UI direction documented in
  `docs/design.md`.
- Repository guidance and data-safety invariants documented in `AGENTS.md`.
- Windows Documents and LocalAppData known-folder resolution.
- Read-only Current discovery with explicit missing, found, and ambiguous
  states.
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
  mutations across switcher instances.
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

## Last Verification

The following checks passed after the safe Apply work:

```text
cargo fmt --check
cargo test                         # 29 passed
cargo clippy --all-targets -- -D warnings
cargo build --release
```

The storage and process-detection code is not yet referenced by the GUI binary,
so the current release build is not the final linked application-size
measurement.

## Current Worktree Note

`savefile_examples/` is intentionally untracked. It contains binary research
samples and must not be committed until their provenance and distribution
status are explicitly decided. Never overwrite the original samples.

## Next Tasks

Complete these in order unless the design document is updated first:

- [x] Add read-only detection for `MirrorsEdge.exe`.
- [x] Define one mutation guard that combines game-process state and the
  application-level operation lock.
- [x] Implement the application-level lock for concurrent switcher instances.
- [x] Finalize the transaction journal states and Windows atomic replacement
  API in `docs/design.md`.
- [x] Decompress a selected StoredSave into a same-directory staging file.
- [x] Implement apply: capture Current as Stash, recheck safety conditions,
  replace Current, verify, and retain rollback data until commit.
- [ ] Recover interrupted apply transactions at startup.
- [ ] Add tests for process locks, changed Current fingerprints, staging
  failures, replacement failures, rollback, and startup recovery.
- [ ] Decide built-in Preset resource versioning, provenance, and licensing.
- [ ] Bind the stable application layer to a new UI design without modifying
  the existing prototype merely to expose incomplete domain work.

## Separate Research Track

Dynamic save generation is not part of the switching implementation. The
one-star time-trial Preset requires controlled before/after samples and a
verified format specification. Keep those experiments isolated from capture,
storage, and restore code.

## Open Product Decisions

The authoritative list remains in `docs/design.md`. Current open items include:

- First activation behavior when Current is missing.
- Alias validation and default naming.
- Built-in resource upgrades and hidden-state behavior across versions.
- Binary save provenance and distribution notes.
