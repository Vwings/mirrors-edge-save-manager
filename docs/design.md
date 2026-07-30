# Mirror's Edge Save Switcher Design

Status: Draft v0.1

This document records the product and domain decisions made before feature
implementation. The current Slint window remains a technical prototype and is
not changed by this design.

## 1. Purpose

Mirror's Edge stores its PC save in a fixed game-specific location as a
`username.dat` file. Replacing this file is useful for speedrun practice and
competition workflows, but a temporary save is changed by playing the game.

The application provides a safe way to keep known starting saves, preserve the
user's active save, and apply a saved copy without losing the previous state.

## 2. Goals

- Discover the native Windows save location.
- Manage one active `username.dat` file.
- Keep reusable, known-good saves.
- Automatically preserve the active save before every replacement.
- Allow any saved copy to be applied to Current.
- Make saved copies named and described with application metadata.
- Recover from interrupted replacement operations.
- Support built-in saves and user-added saves.

## 3. Non-goals for the first version

- Dynamically generating or editing game progress inside a `.dat` file.
- Supporting Mirror's Edge Catalyst.
- Supporting Linux, Proton, or non-Windows save locations.
- Modifying the Slint technical prototype as part of the storage work.
- Automatically deleting old Stash entries.
- Interpreting every field in the proprietary save format.

## 4. Domain Model

### 4.1 CurrentSave

CurrentSave is the one live save file used by the game.

- It is mutable because the game writes to it.
- It has one active path and one filename.
- The filename is preserved when another save is applied.
- Its content is scanned when the application starts and before a mutation.
- It is never represented as a second copy in the application database.

The expected save directory is the Windows Documents known folder followed by:

```text
EA Games\Mirror's Edge\TdGame\Savefiles\
```

The application should discover the Documents known folder rather than assuming
that it is literally under `%USERPROFILE%\Documents`, because Documents can be
redirected or backed up by OneDrive.

The first version expects exactly one `.dat` file. Zero files is a valid
"Current not found" state. More than one `.dat` file is an ambiguous state and
must block replacement rather than selecting a file arbitrarily.

Discovery is read-only and produces one of four explicit states:

```text
SaveDirectoryMissing
CurrentMissing
CurrentFound(CurrentSave)
CurrentAmbiguous(candidate paths)
```

- `SaveDirectoryMissing` means the expected `Savefiles` directory does not
  exist. Discovery must not create it.
- `CurrentMissing` means the directory exists but contains no regular `.dat`
  file.
- `CurrentFound` means exactly one regular `.dat` file was found.
- `CurrentAmbiguous` means multiple regular `.dat` files were found. Their
  paths are reported for diagnosis, but none is selected.

Extension matching is ASCII case-insensitive. Non-files, symlinks, and files
with other extensions are ignored. Failure to enumerate or inspect the
directory is an operational error rather than a missing-Current state.

### 4.2 StoredSave

StoredSave is an immutable copy that can be selected and applied to Current.
It is the only persisted save object needed by the domain model.

```text
StoredSave
  id
  kind: Preset | Stash
  alias
  description
  origin
  created_at
  source_filename
  source_modified_at
  original_size
  content_hash
  compressed_payload
```

`kind` is a product classification, not a different file format or storage
type. Preset and Stash use the same capture, validation, compression, and apply
logic.

All StoredSave payloads are immutable. Editing an alias or description changes
metadata only.

### 4.3 Preset

A Preset is a StoredSave intentionally kept as a reusable starting point.

Initial built-in examples are:

- A completed campaign starting save.
- The 69% speedrun save.
- A completed-campaign and one-star time-trial save, if a valid file can be
  produced and verified.

User-created Presets can come from Current, Stash, or an imported external
`.dat` file.

Built-in Presets are hidden rather than physically deleted. They can be
restored to the visible collection.

### 4.4 Stash

A Stash is a StoredSave created for recovery or personal history.

- A Stash is created automatically before every apply operation.
- A user can manually capture Current as a Stash at any time.
- Stash entries are never automatically deleted in the first version.
- A Stash can be applied to Current.
- A Stash can be promoted to Preset by moving its classification.

Promotion does not require generating or modifying save content.

## 5. Core Operations

### Capture Current

```text
CurrentSave -> new StoredSave
```

The Current file remains in place and usable. The operation records the file's
metadata and stores an immutable copy.

### Apply StoredSave

```text
CurrentSave -> automatic Stash
selected StoredSave -> new Current content
selected StoredSave remains unchanged
```

Applying a Preset and applying a Stash are the same storage operation. The
source is never consumed.

### Save Current as Preset

This captures Current into a new StoredSave with `kind = Preset`.

### Promote Stash

This changes a Stash's classification to Preset. It is a move between product
collections, not a binary rewrite.

### Import External Save

The user selects a `.dat` file, the application validates it, captures it as a
StoredSave, and asks for an alias and optional description.

Identical content may be imported more than once. The application should warn
about the matching hash but must not silently reject the operation.

### Validate Save File

The first version validates an opaque save by requiring a regular file with the
observed fixed size of `9,134,256` bytes and by computing SHA-256 over the exact
content. It does not reject a file based on unknown internal fields. Structural
validation can be added only after format research distinguishes invariants
from ordinary save data.

## 6. Storage Layout

The application owns data below LocalAppData:

```text
%LOCALAPPDATA%\Mirror's Edge Save Switcher\
  stored-saves\<id>\metadata.json
  stored-saves\<id>\payload.dat.gz
  transactions\<id>.json
  settings.json
```

StoredSave payloads use gzip compression. Capture writes into a hidden staging
directory, finishes and flushes the gzip stream, decompresses it again, and
requires the reconstructed bytes to match the source size and SHA-256 before
committing the entry by directory rename.

The first implementation should prefer one self-contained payload per
StoredSave. Content-addressed deduplication is not required; duplicate content
is allowed at the product level and the compressed files are small in the
provided samples.

Application metadata must use an explicit schema version. Filesystem creation
and modification times are captured as source metadata but are not the
identity of a StoredSave.

The first metadata schema stores the StoredSave ID, classification, alias,
description, origin, application creation time, source filename and modification
time, original size, SHA-256, and compression format. Invalid or unsupported
metadata is reported rather than silently skipped.

## 7. Safe Apply Requirements

The game process is a hard prerequisite for every mutation.

- Detect `MirrorsEdge.exe` before any capture or replacement.
- When it is running, disable all mutating actions and show the lock reason.
- Keep the window responsive so the user can inspect the problem.
- Recheck the process immediately before replacing Current.
- Re-scan and re-hash Current before replacement to detect external changes.
- Use an application-level lock to prevent two switcher instances from acting
  concurrently.

All mutating application operations use one mutation guard. The guard first
attempts to acquire a non-blocking, session-local Windows named mutex and then
checks the game process state. Failure to acquire the mutex reports that another
switcher operation is active; a running game reports a separate blocked reason.
The guard owns the mutex for the full mutation and releases it when dropped.
An abandoned mutex is acquired normally because Windows has already released
its previous owner's claim; transaction-journal recovery remains responsible
for resolving any interrupted filesystem operation.

The replacement transaction is:

1. Acquire the application lock.
2. Verify the game is not running.
3. Capture and verify Current as an automatic Stash.
4. Stage the selected payload beside the live `.dat` file.
5. Verify staged size and content hash.
6. Write a transaction journal and flush it.
7. Recheck the process and Current fingerprint.
8. Atomically replace the live file while retaining a rollback copy.
9. Re-scan and verify the new Current.
10. Commit the transaction and remove temporary rollback data.

Any failure before commit must leave the original Current available. Startup
must inspect unfinished journals and either restore the old file or finish a
replacement only after verifying the recorded fingerprints.

Cloud sync, antivirus scanning, file locks, and controlled-folder access may
cause replacement to fail. These must produce an actionable error and never be
handled by deleting the original Current first.

### 7.1 Replacement API and Artifacts

Current replacement uses the Unicode Win32 `ReplaceFileW` API with the current
path as the replaced file, the verified same-directory staging path as the
replacement file, and a same-directory rollback path as the backup file. All
three files are therefore on the same volume. Replacement flags are zero:
`REPLACEFILE_WRITE_THROUGH` is unsupported, and ACL or metadata merge errors
must not be ignored.

`ReplaceFileW` is treated as an operation that may have changed more than one
path even when it reports failure. In particular, documented failure modes can
leave the old Current under the rollback name or can leave inherited attributes
on the staging file. Every return path must classify and fingerprint the actual
artifacts before cleanup or retry.

Each apply transaction derives these names from one UUID:

```text
%LOCALAPPDATA%\Mirror's Edge Save Switcher\transactions\<id>.json
<Current directory>\.mirrors-edge-save-switcher-<id>.replacement.dat
<Current directory>\.mirrors-edge-save-switcher-<id>.rollback.dat
<Current directory>\.mirrors-edge-save-switcher-<id>.failed.dat
```

The replacement, rollback, and failed paths must not exist when the transaction
starts. Files are created with create-new semantics. Recovery validates the UUID
and requires all recorded temporary paths to equal these derived names beside
the recorded Current path; journal data cannot direct the application to an
arbitrary filesystem path.

### 7.2 Apply Journal Schema

An apply journal uses schema version 1 and contains:

```text
schema_version: 1
transaction_id
operation: Apply
phase: Prepared | Replacing | Replaced | Verified | RollingBack
created_at
updated_at
stored_save_id
automatic_stash_id
current_path
replacement_path
rollback_path
failed_replacement_path
original_fingerprint: size + SHA-256
replacement_fingerprint: size + SHA-256
```

The automatic Stash must already be committed and verified before the first
journal is published. Its ID provides a durable additional copy of the original
bytes, but it does not replace the same-directory rollback requirement.

Journal creation and each phase update use a sibling temporary JSON file. The
application writes the complete document, flushes the file, and publishes it
with `MoveFileExW`; updates use `MOVEFILE_REPLACE_EXISTING |
MOVEFILE_WRITE_THROUGH`. A malformed or unsupported journal blocks all new
mutations and is never silently deleted.

The phases mean:

- `Prepared`: the automatic Stash and replacement staging file are verified,
  and Current still has the recorded original fingerprint.
- `Replacing`: process state and Current fingerprint were rechecked, and the
  next filesystem action is `ReplaceFileW`.
- `Replaced`: artifact inspection shows Current has the replacement fingerprint
  and rollback has the original fingerprint.
- `Verified`: Current was reopened and fully validated after replacement.
- `RollingBack`: the application is about to restore the original fingerprint
  from rollback.

There is no persisted `Committed` phase. Deleting the journal is the commit
marker. Cleanup after `Verified` deletes rollback first and the journal last. A
crash after rollback cleanup but before journal deletion is recoverable because
Current has the verified replacement fingerprint and the automatic Stash still
contains the original bytes.

### 7.3 Durable Apply Order

The detailed apply sequence is:

1. Acquire the mutation guard and discover exactly one Current.
2. Validate and fingerprint Current as the expected original.
3. Capture and verify Current as the automatic Stash.
4. Create the same-directory replacement file, finish decompression, flush it,
   and verify its size and SHA-256.
5. Publish the `Prepared` journal.
6. Recheck `MirrorsEdge.exe`, rediscover the same Current path, and require its
   fingerprint to equal the expected original.
7. Publish `Replacing`, then call `ReplaceFileW` with the rollback path.
8. Inspect all transaction artifacts even if `ReplaceFileW` reports failure.
9. When Current is the expected replacement and rollback is the expected
   original, publish `Replaced`.
10. Reopen and validate Current, then publish `Verified`.
11. Delete rollback and then delete the journal to commit.

File contents are flushed before their paths are published. The design relies
on same-volume Windows rename and replacement semantics plus fingerprint-based
startup recovery; it does not claim stronger power-loss guarantees than the
underlying filesystem and hardware provide.

### 7.4 Recovery Classification

Startup recovery acquires the same mutation guard. If the game is running,
recovery and all other mutations remain blocked. The journal phase explains the
last intended action, but recovery decisions use the actual artifact types and
complete fingerprints.

Let `O` be the recorded original fingerprint, `N` the recorded replacement,
`M` a missing path, and `X` any other content, non-regular file, or unreadable
path. Automatic recovery is limited to these cases:

| Current | Staging | Rollback | Failed | Recovery action |
| --- | --- | --- | --- | --- |
| `O` | `N` | `M` | `M` | Replacement did not occur; delete staging, then journal. |
| `O` | `M` | `M` | `M` | Original is live and staging was lost; delete journal. |
| `N` | `M` | `O` | `M` | Replacement succeeded; verify Current, delete rollback, then journal. |
| `N` | `M` | `M` | `M` | Only with phase `Verified`, rollback cleanup finished; delete journal. |
| `O` | `N` or `M` | `O` | `M` | Original is live; remove verified duplicate artifacts, then journal. |
| `M` | `N` or `M` | `O` | `M` | Restore rollback to Current, verify `O`, then clean up. |
| `O` | `M` | `M` | `N` | Only with phase `RollingBack`, rollback finished; delete failed, then journal. |

Restoring over an existing replacement first publishes `RollingBack`, then uses
`ReplaceFileW` with the rollback file as the replacement and the failed path as
the backup destination. Restoring a missing Current moves rollback back to the
recorded Current path with `MoveFileExW` and `MOVEFILE_WRITE_THROUGH`. Restored
Current must be reopened and fingerprinted before any artifact is deleted.

Any combination containing `X`, an unexpected extra artifact, a mismatched
path, or `Current = N` without `Rollback = O` outside the explicitly listed
`Verified` cleanup window enters a blocked recovery state. The application
preserves all artifacts and reports their paths and observed fingerprints. It
must not infer identity from timestamps, automatically use the automatic Stash
as a substitute rollback, or allow another mutation until the state is
explicitly resolved.

## 8. User Interface Direction

Current is the persistent visual anchor of the application, not an ordinary
list item or a third equal page.

- Place Current in a central active node or switchboard core.
- Show its filename, alias/source, modification time, hash state, and game
  process lock state.
- Show Preset and Stash entries as selectable saved-copy tracks around or
  beside Current.
- Applying an entry should visually communicate both flows:
  `Current -> automatic Stash` and `StoredSave -> Current`.
- Manual capture should appear as a Current-to-Stash action.
- Applying a Stash, Preset, or imported save uses the same action and preview.
- The UI may group entries as Presets and Stash while using one StoredSave model.
- Running-game state disables all mutation actions without freezing the window.

The existing circular Slint nodes are only a technical interaction prototype.
This design does not prescribe changing them before the domain layer is tested.

## 9. Save Format Research

The three sample files currently available are:

```text
savefile_examples\Vwings.dat
savefile_examples\game_finished_blank.dat
savefile_examples\69.dat
```

Observed facts:

- All three files are exactly `9,134,256` bytes.
- `Vwings.dat` has only 153 non-zero bytes and compresses to about 9 KiB with
  gzip.
- `Vwings.dat` and `game_finished_blank.dat` differ in 622 bytes.
- `Vwings.dat` and `69.dat` differ in 14,626 bytes.
- The beginning contains repeated indexed records with apparent capacities of
  `300000` and `10000` bytes.
- The 69% file has occupied records in the first group while the fresh and
  completed-blank samples leave those records mostly empty.
- Progress-related differences are concentrated in a compact region around
  `0x538` and a populated serialized region near `0x8B5010`.
- Opaque 16-byte and 20-byte values occur beside data records and may be
  checksums or identifiers.

These observations are not yet a format specification. Directly changing
unknown fields is prohibited until controlled samples identify their meaning
and all integrity fields are understood.

The one-star Preset is a separate research task. It requires controlled
before/after samples where only one unlock, star result, or time changes at a
time. A generated file must be tested by launching the game and loading the
relevant menus before it is distributed as a built-in Preset.

## 10. Built-in Data and Distribution

Built-in saves should be shipped as compressed, verified resources. The sample
files demonstrate that fixed size does not imply large distribution size:

- `69.dat`: approximately 25 KiB gzip-compressed.
- `game_finished_blank.dat`: approximately 9.5 KiB gzip-compressed.
- `Vwings.dat`: approximately 9 KiB gzip-compressed.

The release build must measure decompression code size before finalizing the
format. User-added saves are stored in LocalAppData and are independent of the
program package.

## 11. Testing Boundary

Before implementation is considered safe, tests must cover:

- Missing, valid, and ambiguous save directories.
- Exact-size and invalid external files.
- Capture and hash verification.
- Duplicate-content warning.
- Apply with automatic Stash creation.
- Process-running lock.
- Current changed between scan and replacement.
- Failed staging, failed replacement, and rollback.
- Interrupted transactions recovered on startup.
- Built-in hiding and restoration.
- Stash promotion to Preset.

Format-generation tests are separate from switching tests. A format research
failure must not compromise the ability to copy and restore opaque save files.

## 12. Open Decisions

The following items remain implementation or product details rather than
changes to the core model:

- Whether a missing Current should default to the Windows account name or ask
  for confirmation before creating the first filename.
- Whether later format research supports safe structural validation beyond the
  fixed size.
- Measured executable-size impact once storage is linked into the application.
- Alias validation and default naming rules.
- Built-in resource versioning across application upgrades.
- Licensing and provenance notes for distributed binary save resources.
