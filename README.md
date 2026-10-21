# FSRoneclick

Automated deployment and runtime orchestrator for AMD FSR 3.1 / 4.1.1 "Redstone", Frame Generation, OptiScaler, ReShade, and RenoDX HDR.

[![Release](https://img.shields.io/github/v/release/bbayram-l/fsroneclick?style=flat-square)](https://github.com/bbayram-l/fsroneclick/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/bbayram-l/fsroneclick/ci.yml?style=flat-square&label=CI)](https://github.com/bbayram-l/fsroneclick/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)
[![Platform: Windows](https://img.shields.io/badge/Platform-Windows%2010%20%2F%2011%20x64-informational.svg?style=flat-square)](https://github.com/bbayram-l/fsroneclick)
[![No Runtime Needed](https://img.shields.io/badge/Dependencies-None%20(Standalone%20EXE)-success.svg?style=flat-square)](https://github.com/bbayram-l/fsroneclick/releases)

---

## Download & Quick Start (No Rust Required)

FSRoneclick is distributed as a **fully self-contained, pre-compiled Windows executable**. You do **not** need Rust, Cargo, Python, Visual Studio, or any development tools installed.

1. Go to the [**Latest GitHub Release**](https://github.com/bbayram-l/fsroneclick/releases/latest) page.
2. Download **`fsroneclick-vX.Y.Z-windows-x64.zip`** (or download `fsroneclick.exe` directly).
3. Extract the folder anywhere on your computer (e.g. `Downloads` or `Desktop`).
4. Double-click **`fsroneclick.exe`** to launch the application.

> **Windows SmartScreen**: release builds are not code-signed yet, so Windows may show *"Windows protected your PC"*.
> Choose **More info → Run anyway**. You can verify the download against the published `.sha256` file:
> `Get-FileHash .\fsroneclick.exe -Algorithm SHA256`

---

## Before You Start

- **Single-player / offline only for protected games.** FSRoneclick installs DLL proxies (e.g. `dxgi.dll`) into game
  folders. Kernel anti-cheat systems (Easy Anti-Cheat, BattlEye, Vanguard, Ricochet) can block these or flag your
  account. FSRoneclick shows a warning when it detects anti-cheat, but **you are responsible for where you use it**.
- **64-bit games only.** OptiScaler and ReShade64 do not work in 32-bit games, so installs into them are refused.
- **Use at your own risk.** Mods can cause crashes or visual glitches. Uninstall restores the game folder (see
  [What FSRoneclick Changes](#what-fsroneclick-changes-on-your-pc)), and Steam/Epic "Verify files" is always a fallback.

---

## Inspiration & Reference

This project was inspired by **[DLSS5oneclick](https://github.com/faisalkindi/DLSS5oneclick)** by **[Faisal Al-Kindi (faisalkindi)](https://github.com/faisalkindi)**.

While *DLSS5oneclick* focuses on automating the installation of the DLSS 5 Feeder / ReShade add-on pipeline for NVIDIA setups, **FSRoneclick** adopts a similar automated GUI and CLI workflow tailored for **[OptiScaler](https://github.com/optiscaler/OptiScaler)**, **AMD FSR 3.1 / 4.1.1 "Redstone"**, Frame Generation, and AMD Radeon (RDNA 3 / RDNA 2) as well as universal GPU configurations.

---

## Overview

**FSRoneclick** automates the installation, configuration, and maintenance of AMD FidelityFX Super Resolution (FSR 3.1 and FSR 4.1.1 "Redstone"), Frame Generation, ReShade, and RenoDX HDR for PC games.

PC games often lock Frame Generation or advanced upscalers behind proprietary hardware checks (e.g. requiring RTX 40/50 series GPUs). FSRoneclick works around these restrictions:
- Automatically detects installed games in Steam libraries, the Epic Games Launcher, and common standalone folders (`C:\Games`, `D:\Games`, `E:\Games`, `GOG Games`).
- Automatically pairs the game executable with the optimal proxy hook (`dxgi.dll`, `winmm.dll`, `d3d12.dll`, or `version.dll`).
- Deploys **OptiPatcher** for games where it is known to help, to neutralize vendor checks that crash games when DLSS Frame Generation is enabled.
- Auto-generates `OptiScaler.ini` configurations tuned for your GPU architecture and the game's known workarounds.
- Deploys **ReShade 6.4.1** and **RenoDX** for native HDR, plus AMD FidelityFX CAS sharpening.
- Disables watermark overlays and debug banners by default for a clean presentation.

---

## Key Features

### Automated Detection & Smart Injection
- **Library Scanner**: Finds every Steam library listed in `libraryfolders.vdf` (Steam's install location is read from the registry), Epic Games Launcher installs, and games in common non-store folders.
- **Binary Targeting**: Targets the real engine binary inside subdirectories (e.g. `FarCry5.exe` inside `bin\`) rather than launcher wrappers.
- **Hook Strategy Selection**: Uses the proxy hook recommended by the community compatibility list to avoid launch crashes.

### Hardware-Aware Optimization
- **RDNA 3 / RX 7000 Acceleration**: Detects RDNA 3 GPUs from the display adapter information Windows records and enables the FSR 4.1.1 path (`Fsr4Update`).
- **Universal Fallback**: Uses FSR 3.1 for RDNA 2, Intel Arc, NVIDIA, and other GPUs.
- **Clean Display**: Watermark flags (`Fsr4EnableWatermark = false`, `EnableWatermark = false`) are disabled by default.

### Crash Prevention & Vendor-Lock Bypass
- **OptiPatcher Integration**: Deploys `OptiPatcher.asi` (with ASI loading enabled in OptiScaler) for games where the compatibility list marks it as supported.
- **Companion Libraries**: Stages the supporting runtimes shipped in the OptiScaler package (fakenvapi, dlssg-to-fsr3, AMD FidelityFX and Intel XeSS DLLs, D3D12 Agility SDK).

### Visual Enhancement & HDR
- **AMD FidelityFX CAS**: Installs the Contrast Adaptive Sharpening ReShade shader, with an embedded copy used when it cannot be downloaded.
- **RenoDX Native HDR**: Matches the game against the live RenoDX mod catalog and installs the game-specific HDR add-on for ReShade.
- **REFramework**: Stages REFramework automatically for Capcom RE Engine games.

### Dynamic Database & Updates
- **Large Compatibility Database**: ~700 titles from the OptiScaler community compatibility list and 400+ titles from AMD's FSR "Redstone" catalog are embedded.
- **Live Sync**: `--update-compat` (or *Sync Compatibility Wiki DB* in the app) downloads and caches the latest OptiScaler compatibility list.

### Safe & Non-Destructive
- **Manifest-Driven Clean Rollback**: Every placed file is recorded in `.fsroneclick-manifest`. Uninstall removes only those files.
- **Original Files Preserved**: If a game already ships a file FSRoneclick needs to replace (for example its own `amd_fidelityfx_dx12.dll`), the original is kept as `<name>.fsroneclick-backup` and restored on uninstall.
- **Safe to Retry**: If a file is in use (the game is still running), uninstall says so and keeps it in the manifest; close the game and uninstall again.

---

## What FSRoneclick Changes on Your PC

| Location | What | Undone by uninstall? |
|---|---|---|
| Game executable folder | Proxy DLL, OptiScaler runtimes and `OptiScaler.ini`, `ReShade64.dll` and `ReShade.ini`, `reshade-shaders\`, `plugins\OptiPatcher.asi`, RenoDX add-on, `dinput8.dll` (RE Engine), `.fsroneclick-manifest` | Yes — files are removed and any replaced originals restored |
| `%LOCALAPPDATA%\FSRoneclick\cache` | Downloaded archives, extracted packages, cached compatibility list | No — delete the folder to reclaim space |
| `HKCU\Software\Microsoft\DirectX\UserGpuPreferences` | Sets the game to the *High performance* GPU (the same setting as Windows *Graphics settings*) | No — change it in Windows *Settings → Display → Graphics* |

Downloads come from GitHub (OptiScaler, OptiPatcher, REFramework, RenoDX, shaders) and reshade.me.

---

## In-Game Usage

1. Open **FSRoneclick** and select your game from the list.
2. Verify the recommended configuration (GPU architecture, hook type, and add-ons) and click **Install Enhancements**.
3. Launch the game through your normal launcher (Steam, Epic, or game executable).
4. In the game's **Video / Display Settings**:
   - Set **Upscaling** to **DLSS** (OptiScaler intercepts this call and executes AMD FSR 3.1/4.1.1).
   - Set **Frame Generation** to **DLSS Frame Generation** or **ON** (intercepted and executed via FSR Frame Generation).
5. Default In-Game Keybindings:
   - `Insert` or `F12`: Toggle OptiScaler in-game configuration menu.
   - `Home`: Toggle ReShade overlay and shader pipeline.

---

## Command Line Interface (CLI)

FSRoneclick can also be executed headlessly for scripting or automation. Commands exit with code `0` on success and `1` on failure.

```bash
# List all detected games and their installation status
fsroneclick.exe --list-games

# Inspect a game directory or executable (graphics API, DLSS, anti-cheat, RenoDX, compatibility)
fsroneclick.exe --check "D:\Games\Far Cry 5\bin"

# Install FSR and enhancements for a specific game directory
fsroneclick.exe --install "D:\Games\Far Cry 5\bin"

# Uninstall injected files and restore stock game state
fsroneclick.exe --remove "D:\Games\Far Cry 5\bin"

# Show component versions and check GitHub for a newer OptiScaler release
fsroneclick.exe --check-updates

# Synchronize local compatibility database with upstream community sources
fsroneclick.exe --update-compat
```

---

## Building from Source (Developers Only)

> **Note**: Building from source is only required if you are contributing to the codebase. Regular users should download the ready-to-run release executable.

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) 1.88 or later
- Windows 10 / 11 64-bit

### Build Steps
```bash
# Clone repository
git clone https://github.com/bbayram-l/fsroneclick.git
cd fsroneclick

# Run the test suite (INI generation, compatibility lookups, wiki parsing, install/uninstall rollback)
cargo test

# Build optimized release binary
cargo build --release
```

The standalone executable will be generated at `target/release/fsroneclick.exe`.

---

## Contributing

Bug reports, game compatibility reports, and pull requests are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md).
Please report security issues privately as described in [SECURITY.md](SECURITY.md). This project follows a
[Code of Conduct](CODE_OF_CONDUCT.md). Notable changes are listed in the [CHANGELOG](CHANGELOG.md).

---

## Acknowledgments & Credits

This tool automates, coordinates, and configures the work of several brilliant open-source projects and researchers. The credit belongs to:

- **[Faisal Al-Kindi (faisalkindi)](https://github.com/faisalkindi)** — **[DLSS5oneclick](https://github.com/faisalkindi/DLSS5oneclick)**, for the inspiring desktop UI concept, library detection layout, and one-click automation workflow.
- **[cdozdil](https://github.com/cdozdil)** and the **[OptiScaler team](https://github.com/optiscaler/OptiScaler)** — **[OptiScaler](https://github.com/optiscaler/OptiScaler)** (GPL-3.0), the middleware translating DLSS and XeSS API calls into AMD FSR 3.1, FSR 4.1.1 "Redstone", and Frame Generation, and **[OptiPatcher](https://github.com/optiscaler/OptiPatcher)** (MIT), which patches vendor checks that crash games when DLSS Frame Generation is requested on non-RTX GPUs.
- The OptiScaler community, for the **[Compatibility List](https://github.com/optiscaler/OptiScaler/wiki/Compatibility-List)** that FSRoneclick's per-game tuning is derived from.
- **[Advanced Micro Devices (AMD)](https://www.amd.com)** — **[AMD FidelityFX SDK](https://github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK)** (MIT License), providing temporal upscaling, frame generation, and Contrast Adaptive Sharpening.
- **[crosire](https://github.com/crosire)** — **[ReShade](https://reshade.me)** (BSD 3-Clause) and **[reshade-shaders](https://github.com/crosire/reshade-shaders)**, the injection framework and shader pipeline.
- **[clshortfuse](https://github.com/clshortfuse)** and the RenoDX mod authors — **[RenoDX](https://github.com/clshortfuse/renodx)** (MIT), providing native HDR reconstruction and tonemapping add-ons.
- **[praydog](https://github.com/praydog)** — **[REFramework](https://github.com/praydog/REFramework)** (MIT), staged for Capcom RE Engine games.
- **[OopyDoopy](https://github.com/OopyDoopy)** — **[CAS-for-ReShade](https://github.com/OopyDoopy/CAS-for-Reshade)** (MIT), the ReShade port of AMD CAS.
- **[emilk](https://github.com/emilk)** — **[egui / eframe](https://github.com/emilk/egui)** (MIT / Apache-2.0), the lightweight immediate-mode GUI toolkit powering the desktop interface.

FSRoneclick downloads these components from their official sources at install time and does not redistribute them.
For license details, see [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

---

## Trademark Notice

AMD, Radeon, FidelityFX, and FSR are trademarks of Advanced Micro Devices, Inc. NVIDIA, GeForce, RTX, DLSS, and Reflex are trademarks of NVIDIA Corporation. Intel, Arc, and XeSS are trademarks of Intel Corporation. This project is an independent open-source utility and is not affiliated with, endorsed by, or sponsored by any of these companies.

---

## License

This project is licensed under the **MIT License**. See the [LICENSE](LICENSE) file for details.
