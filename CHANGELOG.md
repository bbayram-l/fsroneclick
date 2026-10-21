# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
  cleared). A corrupt cached OptiScaler archive is now discarded so the next attempt downloads it again.
- REFramework for RE Engine games is downloaded from the current universal nightly (`REFramework.zip`); the old
  URL no longer existed, so it was silently skipped.
- Removed the built-in RenoDX fallback list, whose download links no longer existed and could override live
  catalog entries. RenoDX downloads from the wiki are now validated (HTTPS from GitHub, plain `*.addon64` names).
- Compatibility lookups no longer match unrelated games by substring through very short titles
  (e.g. "Tyr", "'83"), or match every entry for titles without letters or digits.
- The GUI could get stuck showing a running task when *Check Upstream Versions* or *Sync Compatibility Wiki DB*
  was used during an install.
- *Component Status* reports the ReShade version that is actually installed (6.4.1, previously shown as 6.3.3).

### Changed

- Installing into a 32-bit game is refused, since OptiScaler and ReShade64 are 64-bit only.
- The CLI exits with code `1` when a command fails and warns when anti-cheat is detected.
- Steam libraries are located through the install path Steam records in the registry, in addition to the common
  default folders.
- The embedded fallback CAS shader now carries AMD's MIT license notice.

### Removed

- Unused `image` dependency.

### Added

- CI workflow (format, clippy, tests), issue and pull request templates, Dependabot, CONTRIBUTING, SECURITY, and
  CODE_OF_CONDUCT documents.

## [0.1.0] - 2026-09-13

- Initial release: 1-click OptiScaler (FSR 3.1 / FSR 4 + Frame Generation), ReShade, RenoDX, and CAS deployment
  with a desktop GUI and CLI.

[Unreleased]: https://github.com/bbayram-l/fsroneclick/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/bbayram-l/fsroneclick/releases/tag/v0.1.0
