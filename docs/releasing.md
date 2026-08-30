# Release Process

This document defines the maintainer workflow for preparing and publishing a
GitHub Release. Version-one feature behavior and safety requirements remain in
`docs/design.md`; acceptance evidence remains in `docs/release-checklist.md`.

## Release Model

Releases use semantic package versions and matching `v`-prefixed Git tags:

```text
Cargo.toml version 0.1.0 -> Git tag v0.1.0
```

The `Prepare Release` workflow performs all automated checks, builds and
inspects the Windows executable, prepares the release assets, creates the
annotated tag, and uploads the assets to a Draft Release. A maintainer publishes
that draft only after inspecting the downloaded assets. No workflow publishes a
release automatically.

## Continuous Integration

`.github/workflows/ci.yml` runs on pushes and pull requests targeting `main`, and
can also be started manually. Its Windows job runs:

```powershell
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

## Before Preparing a Release

1. Synchronize remote branches and workflow-created release tags:

   ```powershell
   git fetch origin --prune --tags
   ```

2. Freeze the release scope and complete the relevant items in
   `docs/release-checklist.md`.
3. Update `Cargo.toml`, `Cargo.lock`, both READMEs, `CHANGELOG.md`, and the
   release notes at `docs/release-notes/v<version>.md` as needed.
4. Commit and push every intended release change to `main`.
5. Require a clean worktree and a passing `CI` workflow for that exact commit.
6. Confirm that the requested tag and GitHub Release do not already exist.

## Prepare the Draft

Authenticate GitHub CLI once with `gh auth login`, then trigger the workflow
from the repository root:

```powershell
gh workflow run prepare-release.yml --ref main -f version=0.1.0
```

`--ref main` selects the workflow and source commit from `main`. The version
input omits the `v` prefix. The workflow rejects a non-`main` source, a malformed
version, a mismatch with `Cargo.toml`, missing release notes, or an existing tag.

After formatting, tests, Clippy, the release build, metadata inspection, and
package verification succeed, the workflow creates the annotated tag at the
captured `GITHUB_SHA`. It then creates a Draft Release with `--verify-tag` and
uploads:

```text
mirrors-edge-save-manager-windows-x64.zip
SHA256SUMS.txt
```

The archive contains only `mirrors-edge-save-manager.exe`; the application
remains a standalone single-file distribution after extraction. The checksum
covers the downloaded ZIP. If any step before tag creation fails, no tag or
Release is created.

## Inspect the Draft

Download the actual Draft Release assets into a new directory:

```powershell
gh release download v0.1.0 --dir release-check
```

Compare the archive hash with `SHA256SUMS.txt`, then extract it and confirm it
contains only `mirrors-edge-save-manager.exe`. Run that executable and confirm
the icon and Windows version metadata, both interface languages, Current
discovery, and the agreed final smoke test against a separately backed-up game
profile.

If the draft is rejected, do not publish it. Delete the Draft Release and its
remote tag, fix the problem on `main`, and run `Prepare Release` again. Never
move or reuse a tag after its Release has been published.

## Publish

Publish only the inspected draft:

```powershell
gh release edit v0.1.0 --draft=false
```

Then verify that the public Release page, archive download, and checksum download
work without maintainer credentials. Record the final verification and release
state in `docs/status.md` and `docs/release-checklist.md`.
