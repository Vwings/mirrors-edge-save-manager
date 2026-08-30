# Release Acceptance Checklist

This checklist tracks evidence required before the first public Windows release.
Never perform a destructive scenario against the only copy of a real Current.
Use temporary Documents trees, controlled test accounts, or a separately backed
up game profile for mutation and interruption testing.

## Automated Baseline

- [x] `cargo fmt --check`
- [x] `cargo test`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo build --release`
- [x] Confirm `CI` passes on a GitHub-hosted Windows runner.
- [ ] Measure release executable size and cold startup time.

## Native Environment

- [x] Resolve Documents and LocalAppData through Windows known folders.
- [x] Discover only the account-named Current while unrelated `.dat` files exist.
- [x] Validate and hash the complete native Current without modifying it.
- [x] Confirm the native save and application-data directories are ordinary
  directories owned by the current account.
- [x] Launch two manager instances concurrently in read-only state.
- [x] Block a real cross-process mutation mutex before creating a StoredSave.
- [x] Detect an exact-name game process, intercept mutations, and refresh after
  it exits.

## Filesystem Failure Matrix

- [x] Reproduce a real Windows Current sharing violation in a temporary save
  tree; preserve Current and recover the unfinished transaction after release.

## Interruption And Recovery

- [ ] Force termination before replacement publication in a disposable tree.
- [ ] Force termination immediately before and after `ReplaceFileW`.
- [ ] Force termination during rollback and during verified cleanup.
- [ ] Restart and verify the documented recovery action for each interruption.
- [ ] Verify a contradictory or malformed transaction remains blocked and keeps
  every artifact for diagnosis.

## Windows And Display Matrix

- [ ] Windows 10 x64 acceptance pass.
- [ ] Windows 11 x64 acceptance pass.
- [x] Simulated 100%, 125%, and 150% Slint scaling without layout overlap.
- [x] Repeatedly switch English and Simplified Chinese in the software-rendered
  release without losing window responsiveness.
- [ ] Physical multi-monitor test with different per-monitor DPI values.

## Final Usability Smoke

- [ ] Confirm successful-operation feedback disappears after four seconds.
- [ ] Confirm Import appears only for Presets and Clear Stash only for Stashes.
- [ ] Confirm Clear Stash requires confirmation and preserves Current and all
  Presets.
- [ ] Confirm manual Current capture allows name and description review before
  writing.
- [ ] Confirm every StoredSave row shows creation time to the second, with or
  without a description.
- [ ] Confirm duplicate Presets are blocked and no Stash is created when either
  a verified Preset or Stash already preserves Current.
- [ ] Confirm Current-captured rows retain their Apply source alias and
  changed-since-Apply state after restart and after the source is renamed or
  deleted.
- [ ] Confirm duplicate Current capture stops before metadata entry, saved copies
  matching Current cannot open Apply, and last-Apply change status survives a
  restart.

## Packaging And Lifecycle

- [ ] Finalize semantic application version.
- [x] Embed the finalized application icon in both the native window and
  executable, and expose package-derived Windows file and product versions.
- [ ] Build and inspect the distributable package.
- [ ] Verify first run, upgrade with existing version-one data, and removal.
- [ ] Verify removal does not delete user StoredSaves without explicit consent.
- [x] Document backup expectations, known limitations, and third-party asset
  notices in user-facing release documentation.
