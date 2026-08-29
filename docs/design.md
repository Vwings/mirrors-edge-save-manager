# Mirror's Edge Save Manager Design

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
that it is literally under `%USERPROFILE%\Documents`, because Windows controls
the folder's physical location.

Current is always the file named from the current Windows account:
`<account-name>.dat`. Other `.dat` files in `Savefiles` are treated as user-owned
history or backups and are ignored by discovery and mutation operations.

Discovery is read-only and produces one of three explicit states:

```text
SaveDirectoryMissing
CurrentMissing
CurrentFound(CurrentSave)
```

- `SaveDirectoryMissing` means the expected `Savefiles` directory does not
  exist. Discovery must not create it.
- `CurrentMissing` means the directory exists but the account-named `.dat` file
  is absent, regardless of other `.dat` files in the directory.
- `CurrentFound` means the account-named path is a regular file.

Windows path matching supplies the normal case-insensitive filename behavior.
If the account-named path exists but is not a regular file, discovery reports an
operational error. Failure to inspect the directory or target path is also an
operational error rather than a missing-Current state.

Application-owned transaction artifacts and user backup `.dat` files never
participate in Current discovery. Transaction recovery owns the reserved
replacement, rollback, and failed files.

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
- A completed-campaign save with all time trials unlocked.
- A completed-campaign and one-star time-trial save, if a valid file can be
  produced and verified.

User-created Presets can come from Current, Stash, or an imported external
`.dat` file.

Built-in Presets are hidden rather than physically deleted. They can be
restored to the visible collection.

Each built-in Preset has a stable UUID logical ID and a positive resource
version. Payload revisions retain the logical ID and increment the version;
materially different Presets receive a new logical ID. Hidden state is keyed by
logical ID, so upgrading a hidden resource does not expose it again. A newly
introduced logical ID is visible by default.

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

### Delete StoredSave

Only user-created Presets and Stashes can be permanently deleted. Current and
built-in Presets never expose this operation. The UI requires an explicit
confirmation that names the StoredSave; deletion is never a row-selection side
effect and there is no automatic Stash retention policy in version one.

Deletion uses the same process check, application mutation lock, and unfinished
transaction check as other mutations. After validating the ID and metadata, the
repository atomically renames the StoredSave directory to an ignored tombstone
in the same parent directory. That rename commits logical deletion without a
partially visible entry. Physical tombstone cleanup is best effort; a cleanup
failure may retain inaccessible bytes but must not resurrect a partially
deleted StoredSave or endanger Current.

### Import External Save

The user selects a `.dat` file, the application validates it, captures it as a
StoredSave, and asks for an alias and optional description.

Identical content may be imported more than once. The application should warn
about the matching hash but must not silently reject the operation.

### StoredSave Naming

Aliases are trimmed before storage, must not be empty, and are limited to 80
Unicode characters. User-entered aliases that violate these rules are rejected
rather than truncated.

When the caller does not provide an alias, Current captures use the StoredSave
classification and local timestamp, such as `Stash 2026-08-01 14:30:00` or
`Preset 2026-08-01 14:30:00`. External imports first use the source filename
stem; if no usable stem exists, they fall back to `Preset` plus the local
timestamp. Generated aliases pass through the same validation as user-entered
aliases.

### Validate Save File

The first version validates an opaque save by requiring a regular file with the
observed fixed size of `9,134,256` bytes and by computing SHA-256 over the exact
content. It does not reject a file based on unknown internal fields. Structural
validation can be added only after format research distinguishes invariants
from ordinary save data.

## 6. Storage Layout

The application owns data below LocalAppData:

```text
%LOCALAPPDATA%\Mirror's Edge Save Manager\
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

### Persisted-Data Migration Policy

Version one reads only the schema versions it explicitly implements. An unknown
StoredSave metadata, settings, or transaction-journal schema is never guessed,
partially rewritten, or deleted; it produces an actionable unsupported-data or
blocked-recovery state while preserving the original files.

Any future schema migration must be implemented and tested as a distinct,
version-to-version operation before a new writer is enabled. It must validate
the complete source document, preserve immutable payload bytes and fingerprints,
write and flush a separate replacement, verify that replacement, and publish it
atomically. Transaction journals are not migrated while unfinished: the owning
application version must recover them first, or the newer version must retain a
compatible reader for that exact journal schema. No release may silently reset
settings or discard a StoredSave merely because its schema is newer or unknown.

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
- Use an application-level lock to prevent two manager instances from acting
  concurrently.

All mutating application operations use one mutation guard. The guard first
attempts to acquire a non-blocking, session-local Windows named mutex and then
checks the game process state. Failure to acquire the mutex reports that another
manager operation is active; a running game reports a separate blocked reason.
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

File locks or denied filesystem operations may cause replacement to fail. These
must produce an actionable error and never be handled by deleting the original
Current first.

When Current is missing but the native save directory exists, first activation
is a separate operation rather than an ordinary Apply because there is no
Current to capture as an automatic Stash. The application obtains the current
Windows account name, presents the resulting `<account>.dat` filename for user
confirmation, and only then materializes a selected verified StoredSave at that
exact derived path. The account name is never accepted as a path: it must be a
single valid Windows filename stem. A missing save directory, an existing
account-named Current, a running game, or an unconfirmed filename blocks
activation.

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
%LOCALAPPDATA%\Mirror's Edge Save Manager\transactions\<id>.json
<Current directory>\.mirrors-edge-save-manager-<id>.replacement.dat
<Current directory>\.mirrors-edge-save-manager-<id>.rollback.dat
<Current directory>\.mirrors-edge-save-manager-<id>.failed.dat
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
operation: apply
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

### 7.5 First-Activation Transaction

First activation creates an account-named Current only when the native save
directory exists and discovery reports `CurrentMissing`. It uses the same
mutation guard and unfinished-journal gate as Apply. Other `.dat` files do not
participate in the operation.

An activation journal uses schema version 1 and contains:

```text
schema_version: 1
transaction_id
operation: activate
phase: Prepared
created_at
updated_at
stored_save_id
current_path
staging_path
replacement_fingerprint: size + SHA-256
```

The staging path is the same-directory transaction-derived replacement path:

```text
<Current directory>\.mirrors-edge-save-manager-<id>.replacement.dat
```

The durable order is:

1. Acquire the mutation guard and require no unfinished journal.
2. Require an existing native save directory and a missing account-named
   Current.
3. Require the user-confirmed filename to equal the derived account filename.
4. Materialize the selected StoredSave into the create-new staging path, flush
   it, and verify its complete fingerprint.
5. Publish the `Prepared` activation journal.
6. Recheck the game process and require Current is still missing.
7. Move staging to Current with `MoveFileExW` and `MOVEFILE_WRITE_THROUGH`,
   without replacement permission.
8. Reopen and verify Current, then delete the journal to commit.

Startup recovery validates the schema, UUID, native account-named Current path,
derived staging path, and complete fingerprints. Let `N` be the selected
fingerprint, `M` a missing path, and `X` any other or unreadable state:

| Current | Staging | Recovery action |
| --- | --- | --- |
| `M` | `N` | Finish activation by moving staging to Current, verify, then delete journal. |
| `M` | `M` | Staging was lost before publication; delete journal. |
| `N` | `M` | Activation succeeded; verify Current, then delete journal. |

Any state containing `X`, both paths present, a mismatched path, or an
unexpected fingerprint is blocked and preserves all artifacts. First activation
never overwrites an existing Current and never uses a backup `.dat` as Current.

### 7.6 Actionable Application Errors

Application operations retain their concrete diagnostic errors and additionally
classify each failure with a stable user action. This classification is
independent of Slint and contains no translated display text. The UI maps the
structured operation and action values to localized guidance while logs and
support views can retain the original paths, fingerprints, OS errors, and source
chain.

The supported action categories distinguish invalid aliases and imports, a
running game, another manager operation, unfinished recovery, manually blocked
recovery, missing native save setup, first activation, changed Current state,
filesystem access failures, damaged StoredSave data, invalid promotion targets,
filename confirmation, unsupported platforms, retryable platform failures, and
internal failures that should be reported.

Classification never performs filesystem work. In particular, journal-related
apply or activation failures conservatively request transaction recovery, while
invalid journals, contradictory artifacts, and failed rollback request manual
resolution. Automatic retries or cleanup must remain explicit application
operations so error presentation cannot weaken transaction safety.

## 8. User Interface Direction

Current is the persistent visual anchor of the application, not an ordinary
list item or a third equal page. The normal window is a compact single-column
workspace: Current first, then one Preset/Stash library. A permanently visible
operation or diagnostics column is not justified by the product's small action
set.

- Current shows its role, modification time, and direct capture actions. Raw
  paths, full hashes, the account-derived filename, and healthy process or
  recovery labels do not compete with the normal workflow.
- Preset and Stash remain two views of one StoredSave collection. Compact tabs
  expose one collection at a time and retain counts for the inactive view.
- Each StoredSave row exposes its available actions directly. Apply is a
  labeled primary action; Edit and Delete use consistent vector icons; a Stash
  additionally offers the labeled `Make Preset` action. Selecting a row never
  opens a generic action menu or details layer.
- Built-in Presets expose only Apply. User Presets expose Edit, Delete, and
  Apply. Stashes expose Edit, Make Preset, Delete, and Apply.
- Icon-only controls use familiar vector symbols and always have an accessible
  name. Text labels, hover, pressed, focus, and disabled states make the normal
  workflow understandable without relying on floating Tooltip popups. A later
  diagnostics surface may add contextual help where the action cannot be made
  self-explanatory in place.
- Apply opens a dedicated in-window modal over a dimmed, inactive workspace.
  It explains, under `What happens after you confirm`, the automatic sequence:
  back up Current, apply the selected save, then verify and finish or roll back
  on failure. The numbered items are explanatory only; they are never clickable
  user steps. This is safety confirmation, not a permanent preview panel.
- Edit and Delete use their own focused in-window modals. Delete is permanent,
  names the target explicitly, requires confirmation, and is available only for
  user StoredSaves. Make Preset executes directly and refreshes the collection.
- Manual capture appears as a direct Current-to-Stash or Current-to-Preset
  action. External import remains a direct library action.
- While `MirrorsEdge.exe` is running, a full-content safety overlay blocks the
  workspace and shows only the running-game reason plus a concise Current
  summary. The application polls process state in the background and restores a
  refreshed workspace automatically after the game closes.
- Manual refresh is a small icon action. The overview also refreshes after each
  operation and when the window regains focus.
- When the native save directory does not exist, Current shows the actionable
  instruction to launch Mirror's Edge once so the game can create it. Apply is
  visibly unavailable in that state instead of opening a confirmation whose
  final action is silently disabled. Other `.dat` files never substitute for
  the account-named Current.
- Version one ships in English and Simplified Chinese. First launch follows the
  Windows display language, falling back to English when no supported locale
  matches. A compact language selector in the top bar lets the user override
  that choice, and the explicit choice is persisted in application settings.
- The top bar shows the application version with low visual emphasis so bug
  reports can identify the running build without opening a separate surface.
- Built-in Presets carry a compact localized `Built-in` tag. The tag communicates
  origin and read-only behavior without adding another action or category.
- Before release, typography is reviewed as a complete hierarchy rather than by
  increasing every size uniformly. Normal body text, metadata, buttons, tabs,
  and modal guidance must remain comfortably readable while preserving the
  compact single-column layout.

### 8.1 Production Visual Language

The production UI follows the original game's functional visual language rather
than a generic dark dashboard theme:

- White and very light gray are the dominant surfaces. Current may use a subtle
  cool-gray plane to distinguish the live save, but large charcoal surfaces are
  not part of the production interface.
- Red is reserved for the next meaningful action, the selected Apply source,
  and critical blocking states. Decorative red bars and repeated red borders
  weaken its navigation role and are not used.
- Amber may communicate warnings. Blue is informational only and must not imply
  the primary action. State is always accompanied by text rather than color
  alone.
- Information is organized with broad flat planes, strong alignment, generous
  empty space, and a small number of geometric anchors. Repeated ornamental
  cards, gradients, shadows, and dense status chrome are avoided.
- StoredSave alias and description are the primary human-facing identity.
  Source filenames, exact hashes, and storage paths remain available to logs or
  diagnostics but are hidden from the normal workflow.
- Current is identified by its product role and modification time in the normal
  workflow. Its account-derived `.dat` filename is diagnostic information, not
  a useful user-facing name and not a substitute for StoredSave aliases.
- The normal workflow is visible without opening a modal: inspect Current and
  act directly on a StoredSave row. Only an action that needs focused input or
  destructive confirmation opens an in-window modal.
- Apply and first activation share the dedicated operation modal. A neutral
  explanatory sequence describes ordering without persistent `Backup` or
  `Apply` status regions and without implying that the user must perform the
  numbered items. First activation replaces the backup step with its
  no-existing-Current state and includes the derived account-named filename
  before confirmation.
- Non-destructive Current capture is available directly from the Current plane
  as either Stash or Preset and uses the validated timestamp alias default until
  metadata editing is exposed. External import uses a native Windows `.dat`
  picker, followed by the same complete validation and repository capture rules
  as every other import path; the picker filter is convenience, not validation.
- User StoredSave alias and description are read-only in rows. Edit opens a
  focused modal with visible labeled fields and Save/Cancel controls. Losing
  focus within a field does not create a hidden always-editable row state.
  Stash promotion is a direct secondary action and never rewrites payload bytes;
  built-in metadata remains read-only.
- Version one does not expose built-in Preset hiding or restoration in the UI.
  Visibility is a low-frequency display preference rather than a StoredSave
  classification, and adding a recovery surface for it would compete with the
  primary Preset, Stash, and Apply workflow. The tested storage capability may
  remain for a later settings surface.
- The interface does not number permanent regions as a forced tutorial. Numbered
  steps appear only in the Apply modal, where sequence is a data-safety
  guarantee.
- Every interactive row and control uses pointer, hover, pressed, and persistent
  selected states. Decorative chevrons are not used as the only indication that
  a row is actionable, and rows do not need redundant Select buttons.
- While a mutation is running, the active modal becomes progress feedback and
  the dimmed workspace remains inactive. A refreshed overview replaces stale
  state when the operation finishes.
- Internal content uses stretch constraints rather than desktop coordinates.
  The compact fixed-size workspace keeps list scrolling internal and modal
  geometry independent from collection length.

Version one uses a compact fixed-size native Windows window centered on the
primary display at startup. The final logical dimensions are chosen during
manual scaling inspection after the single-column layout is implemented.
StoredSave collections scroll within that surface, so resizing and maximizing
do not add required product capability. The
native title bar is retained for reliable dragging, minimizing, closing,
keyboard shortcuts, system menus, snap behavior, and accessibility. A custom
frameless title bar is not part of version one unless those behaviors are first
implemented and verified explicitly.

This direction is based on DICE's descriptions of the original art style as
bright, clean, graphical, flat, and deliberately low in visual noise, with red
Runner Vision identifying where the player should go next:

- `https://www.ign.com/articles/2008/11/13/artist-in-residence-mirrors-edge`
- `https://www.gamedeveloper.com/design/the-philosophy-of-faith-a-mirror-s-edge-interview`
- `https://www.ea.com/news/runners-vision-in-mirrors-edge-catalyst`

The exact desktop proportions are product design choices, not claimed as DICE
interface specifications. The transferable invariant is that color and layout
must guide action rather than decorate every region.

## 9. Save Format Research

Read-only research has established the fixed container layout, Profile and
Ghost-like regions, known integrity layers, and an isolated Time Trial unlock
operation. The concise findings and remaining unknowns are maintained in
`docs/save-format-research.md`; local samples remain ignored under
`scratch/save-format/samples/`.

Deeper reverse engineering is paused. Version one does not parse or edit
proprietary save content, and no production path may change unknown bytes.

### Future Format Tooling

The project should continue researching the complete save structure even though
version one treats save bytes as opaque. A future format tool may provide a
read-only save inspector and field-level diff first, followed only by narrowly
scoped editing once field dependencies and integrity checks are proven.

This is a future direction, not a version-one commitment. It must not weaken the
copy, capture, Apply, automatic-Stash, rollback, or recovery guarantees of the
manager. Built-in resources remain read-only, and any future edited result
should be exported as a new StoredSave rather than modifying the source in
place. The overlap with community tools such as MirrorsEdgeTweaks should be
evaluated before committing to an editor UI; safe offline storage management,
inspection, comparison, and recovery are the distinct potential benefits.

## 10. Built-in Data and Distribution

Built-in saves are shipped as compressed, verified resources. The fixed-size
files compress to approximately 9--26 KiB in the current resource set.

The release build must measure decompression code size before finalizing the
format. User-added saves are stored in LocalAppData and are independent of the
program package.

Version one embeds New Game, completed-campaign, 69% speedrun, and
completed-campaign with all-time-trials-unlocked saves. Each embedded manifest
records a stable UUID, resource version, alias, description, source filename,
exact uncompressed size, SHA-256, and gzip bytes. Materialization uses
create-new semantics and verifies the complete output before it can participate
in activation or Apply.

Built-in Presets are read-only views of embedded resources rather than copied
user entries. Editing their metadata or promoting them is rejected. Visibility
is the only persisted per-user mutation and is stored in schema-versioned
`settings.json` under LocalAppData. Hiding a built-in records its logical ID but
does not remove the embedded bytes. Unknown hidden IDs are retained so a
temporarily removed resource remains hidden if a later version restores it.

The completed-campaign and 69% resources originated as community-circulated
saves with unknown original authors and download locations. New Game and the
clean all-Time-Trials-unlocked resource are controlled game outputs. The assets
are not covered by the project's GPL grant; `resources/built-in/NOTICE.md`
records provenance, fingerprints, and the accepted redistribution risk.

## 11. Testing Boundary

Before implementation is considered safe, tests must cover:

- Missing and valid account-named Current states, including directories that
  contain unrelated backup `.dat` files.
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

- Whether later format research supports safe structural validation beyond the
  fixed size.
- Whether the project should add a post-version-one save inspector, structured
  diff, or narrowly scoped editor after the required fields and integrity rules
  are understood, and which capabilities should remain delegated to community
  tools such as MirrorsEdgeTweaks.
- Measured executable-size impact once storage is linked into the application.
