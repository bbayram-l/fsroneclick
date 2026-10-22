# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Removed

- Unused direct `egui` dependency; the app uses egui through `eframe`.

## [0.1.0] - 2026-10-21

First public release: 1-click OptiScaler (FSR 3.1 / FSR 4 + Frame Generation), ReShade, RenoDX, and CAS deployment
with a desktop GUI and CLI. The notes below describe what changed from the earlier, unpublished prototype.

### Fixed

- Installing no longer destroys game files it replaces (for example a game's own `amd_fidelityfx_dx12.dll` or
  `libxess.dll`). They are kept as `<name>.fsroneclick-backup` and restored by uninstall.
- Running *Update Configuration* (reinstall) no longer orphans files from the earlier install; manifests are merged.
- The manifest is updated as each file is placed, so a failed or interrupted install can still be rolled back.
  If a single file cannot be written, the game's original is put back immediately.
- Uninstall reports files it cannot remove (for example while the game is running) and keeps them in the
  manifest so it can simply be run again, instead of claiming success and forgetting them.
- Uninstall ignores manifest entries that point outside the game folder, and handles each file once even if the
  manifest lists it twice or with different letter case.
- Interrupted downloads are no longer cached as if complete (which made later installs fail until the cache was
  cleared), and a cached copy that fails checksum verification is downloaded again.
- RDNA 3 GPUs now actually get FSR 4: OptiScaler's INT8 model (`Fsr4ForceEnableInt8`) is enabled for them.
  Previously only `Fsr4Update` was set, which RDNA 3 cannot run without the INT8 model.
- `OptiScaler.ini` no longer contains settings OptiScaler v0.9.4 does not have (`[General]`, `SpoofNvidia`,
  `SpoofGpu`, `ReshadePath`).
- The in-app keybinding guide listed `F12` (not an OptiScaler key) and said `End` toggles ReShade; in OptiScaler
  v0.9.4 `End` toggles frame generation and `Page Up` / `Page Down` control the FPS overlay.
- REFramework and OptiPatcher were cached forever after the first download; new upstream builds are now picked up.
- RenoDX add-ons are refreshed on every install (snapshots are rebuilt in place), and two add-ons with the same file
  name from different sources no longer share a cache entry.
- The app window no longer takes several seconds (much longer on a busy disk) to appear: game libraries are scanned
  in the background. Rescanning no longer drops game folders that were added by hand.
- REFramework for RE Engine games is downloaded from the current universal nightly (`REFramework.zip`); the old
  URL no longer existed, so it was silently skipped.
- Removed the built-in RenoDX fallback list, whose download links no longer existed and could override live
  catalog entries. RenoDX downloads from the wiki are now validated (HTTPS from GitHub, plain `*.addon64` names).
- Compatibility lookups no longer match unrelated games by substring through very short titles
  (e.g. "Tyr", "'83"), or match every entry for titles without letters or digits.
- The GUI could get stuck showing a running task when *Check Upstream Versions* or *Sync Compatibility Wiki DB*
  was used during an install.
- *Component Status* / `--check-updates` rows are real checks now: the latest OptiScaler, ReShade, and FSRoneclick
  releases are compared numerically, the compatibility list shows when it was last synced, and rows without an
  online source say *Not checked* instead of claiming to be current.
- CLI output piped into a command that stops reading early (e.g. `fsroneclick --list-games | head`) no longer
  crashes; the command finishes and reports its real exit code.
- The game library scan is about 5× faster (3.6 s → 0.7 s for 75 games here) with identical results.

### Changed

- ReShade updated from 6.4.1 to 6.8.0 (add-on build, SHA-256 pinned).
- Installing into a 32-bit game is refused, since OptiScaler and ReShade64 are 64-bit only.
- The CLI exits with code `1` when a command fails and warns when anti-cheat is detected.
- Steam libraries are located through the install path Steam records in the registry, in addition to the common
  default folders.
- The embedded fallback CAS shader now carries AMD's MIT license notice.

### Removed

- Unused `image` dependency.

### Added

- AMD RDNA 4 (Radeon RX 9000) support: detected GPUs get FSR 4.1.1 with its native FP8 model.
- SHA-256 verification of downloaded runtimes: pinned checksums for OptiScaler and ReShade, and the checksums GitHub
  publishes for the latest REFramework and OptiPatcher releases.
- Double-clicking `fsroneclick.exe` no longer leaves a console window open behind the app. Running it from a
  terminal is unchanged.
- CI workflow (format, clippy, tests, minimum Rust version), issue and pull request templates, Dependabot,
  CONTRIBUTING, SECURITY, and CODE_OF_CONDUCT documents.

### Security

- RenoDX downloads on github.com must be release assets, since anyone can upload issue attachments there.

[Unreleased]: https://github.com/bbayram-l/fsroneclick/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/bbayram-l/fsroneclick/releases/tag/v0.1.0
