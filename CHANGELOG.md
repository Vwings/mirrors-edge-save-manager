# Changelog

All notable changes to Mirror's Edge Save Manager are documented here. Detailed
GitHub Release descriptions are maintained in `docs/release-notes/`.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-08-30

### Added

- Windows x64 save manager for Mirror's Edge (2008).
- Automatic discovery of the account-named Current save through the Windows
  Documents known folder.
- Immutable Preset and Stash storage with capture, import, promotion, editing,
  deletion, and duplicate prevention.
- Four verified read-only built-in Presets separated from user Presets.
- Safe Apply transactions with automatic Current preservation, staging,
  verification, rollback data, and startup recovery.
- Last-Apply provenance and changed-since-Apply tracking for Current and
  captured Stashes.
- English and Simplified Chinese interfaces.

### Security

- Mutations are blocked while Mirror's Edge runs or another manager holds the
  application lock.
- Current fingerprints and replacement artifacts are rechecked immediately
  before committing a replacement.

[Unreleased]: https://github.com/Vwings/mirrors-edge-save-manager/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Vwings/mirrors-edge-save-manager/releases/tag/v0.1.0
