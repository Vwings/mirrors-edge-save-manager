# Project Status and Handoff

Last updated: 2026-08-25

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
All four bundled saves passed in-game verification, closing Phase 7. Phase 8 is
complete: the complete application operations now use a compact,
Current-centered single-column UI. Preset and Stash rows expose their actions
directly; Apply, edit, and permanent user-save deletion use focused in-window
modals instead of a permanent preview column. A full-content safety overlay
blocks interaction while the game runs, with lightweight process polling and
automatic refresh after it closes. Built-in visibility controls remain outside
version one. Work now moves to Phase 9 release hardening and real-environment
safety verification.

The first Phase 9 pass has confirmed the native known-folder path, account-named
Current discovery in a directory containing unrelated `.dat` history, Current
size and full hashing, clean transaction state, ordinary ACL ownership, and
read-only startup of two manager instances. Temporary-directory testing now
covers a real Windows sharing violation at `ReplaceFileW`; real-process checks
cover the cross-process mutation mutex and running-game overlay without changing
Current or adding StoredSaves. Environment-specific OneDrive redirection,
Controlled Folder Access, antivirus delay, and Windows-version checks remain.

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
- Slint-independent application overview combining game-process state, guarded
  startup recovery, Current discovery and complete fingerprinting, and unified
  StoredSave listing.
- Current-centered production UI foundation replacing the circular technical
  prototype, with live Preset and Stash collections, Current modification time,
  concise verification, process state, recovery state, and actionable guidance.
- Background overview loading and refresh through the Slint event loop so save
  scanning, hashing, recovery, and filesystem access do not block the UI thread.
- Read-only Phase 8 integration boundary that deliberately exposes no mutation
  control before its apply preview, confirmations, and blocked-state guidance
  are implemented.
- Selectable Preset and Stash cards with persistent selection styling and a
  non-mutating Apply preview that shows Current-to-automatic-Stash followed by
  selected-StoredSave-to-Current.
- Apply preview guidance that blocks the ordinary two-flow path when Current is
  missing, distinguishes first activation as a separate no-Stash transaction,
  and surfaces game-process and recovery prerequisites before confirmation.
- Production visual direction researched from DICE and EA descriptions of the
  original game's bright, flat, low-noise art style and functional Runner Vision
  color system, with the resulting constraints recorded in `docs/design.md`.
- Red-and-white production UI revision removing the dark dashboard theme,
  decorative vertical accents, modal preview, and competing blue emphasis.
- Persistent three-step guidance for checking Current, choosing one Preset or
  Stash, and reviewing the operation in place before any confirmation.
- Stretch-based workspace filling the supported window surface without Slint
  layout binding loops, absolute desktop coordinates, module overlap, or text
  escaping its owning panel.
- Fixed `1080x720` logical-pixel native window centered on the primary display,
  removing unnecessary resize and maximize behavior while retaining native
  dragging, minimizing, closing, keyboard, and accessibility behavior.
- Primary workflow simplified around aliases, descriptions, modification times,
  and actionable state; raw save paths, SHA-256 values, and source filenames no
  longer compete with normal user decisions.
- Removed numbered top-level workflow labels while retaining compact
  Preset/Stash category tabs suited to the narrow library pane.
- Added explicit hover, pressed, pointer, and selected feedback for interactive
  rows and controls without redundant chevrons or Select labels.
- Current now presents its role and modification time instead of promoting the
  account-derived `.dat` filename and redundant verification copy.
- Locked the lower library and operation-preview columns to fixed outer geometry
  so conditional preview content cannot resize either panel after selection.
- Added restrained functional color hierarchy with a cool-gray Current surface,
  warm-gray library, white preview, and state-specific status colors.
- Removed the redundant red selection stripe; selected rows rely on their stable
  border and full-row background instead of layered decorative indicators.
- Connected ordinary Apply and first activation to their tested transaction
  services through background workers, preserving UI responsiveness during all
  filesystem work.
- Added in-place two-step confirmation, including the exact account-derived
  filename for first activation and the automatic-Stash consequence for Apply.
- Added mutation progress, operation-specific success, actionable classified
  failure guidance, selection locking, and automatic overview refresh after an
  operation finishes.
- Connected Current-to-Stash and Current-to-Preset capture using validated
  timestamp aliases, preserving Current while the verified copy is stored.
- Added a native Windows `.dat` picker and connected external import through the
  guarded repository service without adding a cross-platform dialog dependency.
- Added duplicate-content success warnings for capture and import while keeping
  duplicate StoredSaves as required by the product rules.
- Connected guarded Stash-to-Preset promotion and user StoredSave alias and
  description editing without modifying payload bytes.
- Centralized every application action code into localized UI guidance, including
  disabled Game, recovery, Current, and library states without exposing raw
  diagnostic errors as normal user instructions.
- Added operation-specific progress for Apply, activation, capture, import,
  promotion, and metadata editing, with an explicit tested numeric boundary
  between Rust action values and Slint guidance.
- Kept the tested built-in visibility storage capability out of the version-one
  UI so the primary library remains limited to Preset and Stash.
- Reworked selected-save management into an explicit read-only and Edit details
  mode with labeled fields plus Save/Cancel controls, eliminating invisible
  always-editable inputs.
- Compressed the automatic-Stash and Apply safety preview into two informational
  lines and pinned the real Apply action within the visible panel instead of
  placing it below scrollable management content.
- Replaced the transitional two-column library and permanent operation preview
  with a compact `680x720` single-column workspace that shows all four bundled
  Presets without scrolling.
- Added direct per-row Apply, Edit, Make Preset, and Delete actions according to
  StoredSave origin and classification; built-in Presets expose only Apply.
- Added unified vector edit, delete, and refresh icons with delayed tooltips and
  accessible labels rather than Emoji or unexplained glyphs.
- Added dedicated in-window Apply, Edit, and Delete modals. Apply communicates
  backup, replacement, and verification as a three-step timeline; deletion is a
  named permanent confirmation for user StoredSaves only.
- Added guarded permanent user StoredSave deletion using same-directory atomic
  tombstoning, best-effort physical cleanup, the shared mutation guard, and the
  unfinished-transaction block.
- Added a full-content running-game safety overlay and lightweight background
  process polling that performs a complete refresh only after the game closes.
- Added automatic overview refresh when the native window regains focus, while
  avoiding refresh races during a running mutation or open modal.
- Completed the initial Phase 9 read-only audit against the native save location
  without modifying Current: account-named discovery, ignored history files,
  exact size/hash, ACL ownership, path attributes, and clean journals.
- Verified two manager processes can load read-only state concurrently, while a
  separately owned real Windows mutation mutex blocks `Save as Stash` before any
  StoredSave is created and surfaces the expected guidance.
- Verified a temporary exact-name `MirrorsEdge.exe` process activates the global
  safety overlay, intercepts mutation input, and automatically restores the
  workspace after exit without creating a StoredSave.
- Added a real Windows sharing-violation test that permits Current reads but
  denies replacement sharing, verifies Current remains byte-identical, and
  confirms startup recovery aborts and cleans the failed replacement.
- Finalized the application icon from the maintainer-supplied image by removing
  its watermark, tightening the red border, balancing the square composition,
  and retaining the original graphic unchanged. The optimized PNG and 16--256
  px ICO are used by Windows Explorer and the executable. The Slint window uses
  the verified 32 px ICO layer directly instead of asking Windows to reduce a
  256 px bitmap at runtime, while a dedicated high-quality downscale keeps the
  same graphic clearer in the title header.
- Embedded package-derived file and product versions plus descriptive Windows
  metadata in the executable. The debug executable's associated icon and
  version properties were read back from the built PE for verification.

## Last Verification

The following checks passed after adding guarded deletion and replacing the
permanent preview column with the compact single-column UI:

```text
cargo fmt --check
cargo test                         # 70 passed (69 library, 1 UI helper)
cargo clippy --all-targets -- -D warnings
cargo build --release
```

The previous compact debug window passed window-only inspection at simulated
100%, 125%, and 150% Slint scale factors. The revised `680x720` logical window
restores full visibility for the four bundled Presets; scaling inspection must
be repeated before release acceptance is complete.

The release binary now links every operation exposed by the version-one UI. It
is still not the final size measurement because manual Windows scaling
inspection remains.

The Windows metadata and finalized icon passed `cargo fmt --check`, all 70
tests, strict Clippy, and a dev build. A release rebuild remains intentionally
deferred until semantic versioning and packaging are finalized.

## Current Worktree Note

`scratch/` is intentionally ignored and contains local experiments and binary
research samples. Automated tooling must not overwrite those inputs. Only the
four reviewed compressed resources under `resources/built-in/` are
distribution assets; their fingerprints and provenance notes are in the
adjacent `NOTICE.md`.

## Planned Productization Work

The following scope was confirmed on 2026-08-25 and has not started. Preserve
this order only where one item is a dependency of another; do not turn the list
into unrelated release ceremony.

- [ ] Improve the no-game/no-save-directory state with an explicit instruction
  to launch Mirror's Edge once, and disable Apply before a misleading modal can
  open. Keep the tested account-named Current and ignored-backup behavior.
- [ ] Add English and Simplified Chinese localization. Choose the first-run
  language from the Windows display language, expose a compact top-bar selector,
  and persist an explicit user choice in application settings.
- [ ] Audit the typography hierarchy because the current body and metadata text
  can read too small. Adjust it as one responsive UI pass rather than isolated
  font-size changes.
- [ ] Display the application version unobtrusively in the top area.
- [ ] Add a localized, visually distinct `Built-in` tag to embedded Preset rows.
- [ ] Review every repository document for current behavior. Keep the English
  user README separate from contributor instructions, add `CONTRIBUTING.md`,
  and provide a maintained Simplified Chinese README.
- [ ] Review the flat Rust module layout and the single Slint file. Modularize
  only along real domain, application, component, or localization boundaries;
  do not refactor solely to reduce the number of top-level files.
- [ ] Design and document the GitHub Release process, including final artifacts,
  version/tag relationship, checksums, release notes, and whether publication is
  initially manual or automated.

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
- [x] Replace the technical prototype with a Current-centered production shell
  and bind startup recovery, Current fingerprinting, process state, and unified
  StoredSave listing through a background worker.
- [x] Add StoredSave selection and the Apply preview showing both automatic
  Stash and StoredSave-to-Current flows.
- [x] Bind Apply and first activation with explicit confirmation and complete
  blocked-state guidance.
- [x] Bind manual Stash and Preset capture plus external `.dat` import.
- [x] Bind promotion plus alias and description editing; keep built-in visibility
  controls out of the version-one UI.
- [x] Complete localized application-error guidance, operation progress, and
  refresh-after-mutation behavior.
- [x] Inspect the fixed window, list viewport, row actions, and Apply modal at
  simulated 100%, 125%, and 150% desktop scaling.

## Separate Research Track

Dynamic save generation is not part of version one. Deeper format research is
paused and remains isolated from capture, storage, and restore code. Preserve
the ignored samples and concise research notes so work can resume for a
concrete product requirement.

## Open Product Decisions

The authoritative list remains in `docs/design.md`. No current open decision
blocks the copy, restore, or UI integration paths.
