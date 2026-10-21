# Contributing to FSRoneclick

Thanks for helping! Bug reports, game compatibility reports, documentation fixes, and code are all welcome.

## Reporting

- **Bugs** — use the *Bug report* issue form and include the execution log (GUI → *Execution Log* → *Copy Output*, or the CLI output).
- **Game compatibility** — use the *Game compatibility report* form. Reports of games that *don't* work are just as useful.
- **Security issues** — please don't open a public issue; see [SECURITY.md](SECURITY.md).
- **In-game upscaling or frame-generation artifacts** usually come from OptiScaler itself; check the [OptiScaler issue tracker](https://github.com/optiscaler/OptiScaler/issues) too.

## Development setup

Requirements: Windows 10/11 x64 and [Rust](https://www.rust-lang.org/tools/install) 1.88 or newer.

```powershell
git clone https://github.com/bbayram-l/fsroneclick.git
cd fsroneclick
cargo run               # launch the GUI
cargo run -- --help     # CLI usage
```

Before opening a pull request, run the same checks CI runs:

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Project layout

| Path | Purpose |
|---|---|
| `src/main.rs` | CLI argument handling; launches the GUI when no arguments are given |
| `src/gui.rs` | egui desktop interface; long-running work happens on background threads |
| `src/game.rs` | Game discovery (Steam, Epic, standalone folders), executable and API detection, mod status |
| `src/installer.rs` | Install / uninstall, `OptiScaler.ini` and `ReShade.ini` generation, the rollback manifest |
| `src/net.rs` | Downloads, the local cache (`%LOCALAPPDATA%\FSRoneclick\cache`), archive extraction |
| `src/compatibility.rs` | Embedded OptiScaler compatibility list and AMD FSR Redstone catalog |
| `src/updater.rs` | Live compatibility-list sync and upstream version checks |
| `src/renodx.rs` | RenoDX HDR mod catalog parsing and game matching |
| `src/gpu.rs` | GPU detection and the Windows per-app GPU preference |
| `src/fidelityfx.rs` | Native FidelityFX (FSR 1/2/3) title detection |
| `tests/` | Integration tests |

## Guidelines

- **Never touch game files directly.** Anything placed in a game folder must go through `installer::Deployment`,
  which backs up files it replaces and records everything in the manifest so uninstall can restore the game exactly.
  Add a test in `tests/installer_rollback.rs` when you change install or uninstall behavior.
- **Treat downloaded and remote data as untrusted.** Validate file names and URLs that come from wikis or APIs
  (see `renodx::parse_mods_catalog`), and keep downloads going through `net::download_with_progress`.
- **Verify what gets executed.** Binaries are downloaded with `net::get_or_download_verified` (pinned SHA-256) or
  `net::get_latest_github_asset` (SHA-256 published by GitHub). To bump a pinned component such as OptiScaler or
  ReShade, update its version, URL, and SHA-256 together in `src/installer.rs`; `tests/downloads_and_updates.rs`
  checks that they agree.
- **Keep claims accurate.** README text and UI labels should describe what the code actually does.
- **Dependencies.** Please open an issue before adding a new crate.
- **Commit messages** follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`, ...).

### Compatibility data

`src/compatibility.rs` holds the embedded compatibility tables. When adding or fixing an entry, keep `norm_name` /
`norm_title` equal to `compatibility::normalize_title(name)` (lowercase alphanumerics only). Users can also refresh
the OptiScaler list at runtime with `--update-compat`, which caches the live wiki data locally.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
