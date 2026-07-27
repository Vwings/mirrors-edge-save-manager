# Project Status and Handoff

Last updated: 2026-07-27

This is a temporary handoff document for development sessions. Update it when
a milestone changes, a major decision is made, or the next task changes. Remove
it when the first-version implementation is stable and normal issue tracking
has replaced it.

## Current Stage

The project has moved beyond the original Slint and packaging prototype into
the storage-domain foundation. The domain and repository code is not connected
to the Slint prototype, and no implemented operation can replace the game's
Current save yet.

Latest completed foundation commit:

```text
d47a3ad Add save discovery and storage foundation
```

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
- Tests for discovery, validation, hashing, capture, duplicate content,
  corrupted payloads, and storage-path failures.

## Last Verification

The following checks passed after the storage repository work:

```text
cargo fmt --check
cargo test                         # 16 passed
cargo clippy --all-targets -- -D warnings
cargo build --release
```

The current GUI executable measured `7,470,592` bytes. Storage code is not yet
referenced by the GUI binary, so this is not the final linked storage-size
measurement.

## Current Worktree Note

`savefile_examples/` is intentionally untracked. It contains binary research
samples and must not be committed until their provenance and distribution
status are explicitly decided. Never overwrite the original samples.

## Next Tasks

Complete these in order unless the design document is updated first:

- [ ] Add read-only detection for `MirrorsEdge.exe`.
- [ ] Define one mutation guard that combines game-process state and the
  application-level operation lock.
- [ ] Implement the application-level lock for concurrent switcher instances.
- [ ] Finalize the transaction journal states and Windows atomic replacement
  API in `docs/design.md`.
- [ ] Decompress a selected StoredSave into a same-directory staging file.
- [ ] Implement apply: capture Current as Stash, recheck safety conditions,
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
- Detailed transaction and rollback states.
- Built-in resource upgrades and hidden-state behavior across versions.
- Binary save provenance and distribution notes.
