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

## Packaging And Lifecycle

- [ ] Finalize semantic application version.
- [x] Embed the finalized application icon in both the native window and
  executable, and expose package-derived Windows file and product versions.
- [ ] Build and inspect the distributable package.
- [ ] Verify first run, upgrade with existing version-one data, and removal.
- [ ] Verify removal does not delete user StoredSaves without explicit consent.
- [x] Document backup expectations, known limitations, and third-party asset
  notices in user-facing release documentation.
