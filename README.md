# FSRoneclick

Automated deployment and runtime orchestrator for AMD FSR 3.1 / 4.1.1 "Redstone", Frame Generation, OptiScaler, ReShade, and RenoDX HDR.

[![Release](https://img.shields.io/github/v/release/bbayram-l/fsroneclick?style=flat-square)](https://github.com/bbayram-l/fsroneclick/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)
[![Platform: Windows](https://img.shields.io/badge/Platform-Windows%2010%20%2F%2011%20x64-informational.svg?style=flat-square)](https://github.com/bbayram-l/fsroneclick)
[![No Runtime Needed](https://img.shields.io/badge/Dependencies-None%20(Standalone%20EXE)-success.svg?style=flat-square)](https://github.com/bbayram-l/fsroneclick/releases)

---

## Download & Quick Start (No Rust Required)

FSRoneclick is distributed as a **fully self-contained, pre-compiled Windows executable**. You do **not** need Rust, Cargo, Python, Visual Studio, or any development tools installed.

1. Go to the [**Latest GitHub Release**](https://github.com/bbayram-l/fsroneclick/releases) page.
2. Download **`fsroneclick-windows-x64.zip`** (or download `fsroneclick.exe` directly).
3. Extract the folder anywhere on your computer (e.g. `Downloads` or `Desktop`).
4. Double-click **`fsroneclick.exe`** to launch the application.

---

## Inspiration & Reference

This project was inspired by **[DLSS5oneclick](https://github.com/faisalkindi/DLSS5oneclick)** by **[Faisal Al-Kindi (faisalkindi)](https://github.com/faisalkindi)**.

While *DLSS5oneclick* focuses on automating the installation of the DLSS 5 Feeder / ReShade add-on pipeline for NVIDIA setups, **FSRoneclick** adopts a similar automated GUI and CLI workflow tailored for **[OptiScaler](https://github.com/optiscaler/OptiScaler)**, **AMD FSR 3.1 / 4.1.1 "Redstone"**, Frame Generation, and AMD Radeon (RDNA 3 / RDNA 2) as well as universal GPU configurations.

---

## Overview

**FSRoneclick** automates the installation, configuration, and maintenance of AMD FidelityFX Super Resolution (FSR 3.1 and FSR 4.1.1 "Redstone"), Frame Generation, ReShade, and RenoDX HDR for PC games.

PC games often lock Frame Generation or advanced upscalers behind proprietary hardware checks (e.g. requiring RTX 40/50 series GPUs). FSRoneclick bypasses these artificial restrictions:
- Automatically detects installed games across all Steam drives, custom paths, and standalone directories (`D:\Games`, `C:\Games`, Epic Games, GOG).
- Automatically pairs the game executable with the optimal proxy hook (`dxgi.dll`, `version.dll`, or `winmm.dll`).
- Injects **OptiPatcher** to neutralize in-memory vendor lock signatures, preventing crashes when DLSS Frame Generation is enabled.
- Auto-generates hardware-tuned `OptiScaler.ini` configurations optimized for your specific GPU architecture.
- Injects **ReShade 6.x** and **RenoDX** for native HDR color grading, alongside embedded offline sharpening shaders.
- Disables watermark overlays and debug banners by default for a clean presentation.

---

## Key Features

### Automated Detection & Smart Injection
- **Universal Library Scanner**: Locates games across multi-drive Steam configurations (`libraryfolders.vdf`), registry hives, and non-store folders.
- **Binary Targeting**: Accurately targets the true engine binary inside subdirectories (e.g. `FarCry5.exe` inside `bin\`) rather than launcher wrappers.
- **Hook Strategy Selection**: Automatically selects the safest DLL proxy hook to prevent game launch crashes or anti-tamper collisions.

### Hardware-Aware Optimization
- **RDNA 3 / RX 7000 Acceleration**: Automatically detects RDNA 3 hardware via Windows Display Driver Model (WDDM) queries and configures INT8 tensor acceleration for FSR 4.1.1 execution.
- **Universal Fallback**: Applies stable FP16/FP32 paths for RDNA 2, Intel Arc, and legacy architectures.
- **Clean Display**: All debug logging overlays and watermark flags (`Fsr4EnableWatermark = false`, `EnableWatermark = false`) are disabled by default.

### Crash Prevention & Vendor-Lock Bypass
- **OptiPatcher Integration**: Deploys `OptiPatcher.asi` with an ASI loader to defeat proprietary checks in games that crash when DLSS Frame Generation is toggled on non-RTX cards.
- **Modular Companion Staging**: Stages required companion libraries (`nvngx.dll`, `libxess.dll`, `dxcore.dll`) so games never suffer unhandled null-pointer exceptions when querying vendor extensions.

### Visual Enhancement & HDR
- **FidelityFX CAS & LumeniteFX**: Injects high-performance Contrast Adaptive Sharpening and tonemapping shaders without external downloads.
- **RenoDX Native HDR**: Integrates ReShade 6.x runtime with pre-configured RenoDX add-ons for true native HDR output in supported titles.

### Dynamic Database & Updates
- **400+ Verified Titles**: Indexed against AMD's official Redstone compatibility catalog and the OptiScaler community database.
- **In-App Updater**: Synchronizes latest compatibility tables and checks for upstream component releases directly from within the UI or CLI.

### Safe & Non-Destructive
- **Manifest-Driven Clean Rollback**: Tracks every placed file in `.fsroneclick-manifest`. Uninstallation cleanly deletes only injected files, leaving original game files untouched.

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

FSRoneclick can also be executed headlessly for scripting or automation:

```bash
# List all detected games and their installation status
fsroneclick.exe --list-games

# Install FSR and enhancements for a specific game directory
fsroneclick.exe --install "D:\Games\Far Cry 5\bin"

# Uninstall injected files and restore stock game state
fsroneclick.exe --remove "D:\Games\Far Cry 5\bin"

# Check for updates to OptiScaler, ReShade, RenoDX, and FSRoneclick
fsroneclick.exe --check-updates

# Synchronize local compatibility database with upstream community sources
fsroneclick.exe --update-compat
```

---

## Building from Source (Developers Only)

> **Note**: Building from source is only required if you are contributing to the codebase. Regular users should download the ready-to-run release executable.

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (1.75 or later)
- Windows 10 / 11 64-bit

### Build Steps
```bash
# Clone repository
git clone https://github.com/bbayram-l/fsroneclick.git
cd fsroneclick

# Run unit tests (11 automated tests covering INI generation, RDNA 3 tuning, parser, and manifest rollback)
cargo test

# Build optimized release binary
cargo build --release
```

The standalone executable will be generated at `target/release/fsroneclick.exe`.

---

## Acknowledgments & Credits

This tool automates, coordinates, and configures the work of several brilliant open-source projects and researchers. The credit belongs to:

- **[Faisal Al-Kindi (faisalkindi)](https://github.com/faisalkindi)** — **[DLSS5oneclick](https://github.com/faisalkindi/DLSS5oneclick)**, for the inspiring desktop UI concept, library detection layout, and one-click automation workflow.
- **[cdozdil](https://github.com/cdozdil)** and the **[OptiScaler team](https://github.com/optiscaler/OptiScaler)** — **[OptiScaler](https://github.com/optiscaler/OptiScaler)** (GPL-3.0 / MIT components), the middleware translating DLSS and XeSS API calls into AMD FSR 3.1, FSR 4.1.1 "Redstone", and Frame Generation.
- **[Advanced Micro Devices (AMD)](https://www.amd.com)** — **[AMD FidelityFX SDK](https://github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK)** (MIT License), providing temporal upscaling, frame generation, and RDNA 3 WMMA INT8 tensor acceleration.
- **[crosire](https://github.com/crosire)** — **[ReShade](https://reshade.me)** (BSD 3-Clause) and **[reshade-shaders](https://github.com/crosire/reshade-shaders)**, the injection framework and shader pipeline.
- **[clshortfuse](https://github.com/clshortfuse)** and the RenoDX community — **[RenoDX](https://github.com/clshortfuse/renodx)**, providing native HDR reconstruction and tonemapping add-ons.
- **[Afzaal (Kaidō)](https://github.com/umar-afzaal)** — **[LumeniteFX](https://github.com/umar-afzaal/LumeniteFX)**, the motion-vector provider and tonemapping shader suite.
- **[OptiPatcher / LukeFZ / DLSS Enabler community](https://github.com/optiscaler/OptiScaler/wiki)** — **[OptiPatcher](https://github.com/optiscaler/OptiScaler/wiki)**, for memory and signature patches that bypass proprietary RTX vendor locks to prevent game crashes.
- **[emilk](https://github.com/emilk)** — **[egui / eframe](https://github.com/emilk/egui)** (MIT / Apache-2.0), the lightweight immediate-mode GUI toolkit powering the desktop interface.
- **[JetBrains](https://github.com/JetBrains/JetBrainsMono)** — **JetBrains Mono** font (SIL Open Font License 1.1).

For individual license texts and component breakdowns, please refer to [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

---

## Trademark Notice

AMD, Radeon, FidelityFX, and FSR are trademarks of Advanced Micro Devices, Inc. This project is an independent open-source utility and is not affiliated with, endorsed by, or sponsored by Advanced Micro Devices, Inc.

---

## License

This project is licensed under the **MIT License**. See the [LICENSE](LICENSE) file for details.
