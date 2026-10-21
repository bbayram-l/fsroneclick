# Third-Party Notices and Credits

FSRoneclick is an MIT-licensed automation utility. It coordinates the work of many other projects, and the credit
for the underlying technologies belongs entirely to their authors.

FSRoneclick **does not redistribute** OptiScaler, ReShade, RenoDX, REFramework, or OptiPatcher. They are downloaded
from their official sources when you choose to install them, and each remains under its own license.

---

## Inspiration

### [DLSS5oneclick](https://github.com/faisalkindi/DLSS5oneclick)
- **Author**: [Faisal Al-Kindi (faisalkindi)](https://github.com/faisalkindi)
- **Role**: Inspiration for the application layout, game library scanning approach, and 1-click deployment workflow.
  FSRoneclick implements an equivalent workflow for AMD Radeon GPUs and universal FSR/OptiScaler setups. No code is shared.
- **License**: MIT License

---

## Components downloaded at install time

### [OptiScaler](https://github.com/optiscaler/OptiScaler)
- **Authors**: [cdozdil](https://github.com/cdozdil) and the OptiScaler team
- **Role**: Translation layer that intercepts DLSS, XeSS, and FSR calls and runs AMD FSR 3.1 / FSR 4 and frame generation.
  The release package also contains components such as fakenvapi, dlssg-to-fsr3, and AMD FidelityFX / Intel XeSS
  runtimes, each under its own license as documented in that package.
- **License**: GNU General Public License v3.0
- **Source**: release `v0.9.4` from the GitHub releases page

### [OptiPatcher](https://github.com/optiscaler/OptiPatcher)
- **Authors**: OptiScaler team and contributors
- **Role**: ASI plugin patching vendor checks in games that crash when DLSS Frame Generation is requested on non-RTX GPUs.
- **License**: MIT License

### [ReShade](https://reshade.me) and [reshade-shaders](https://github.com/crosire/reshade-shaders)
- **Author**: [crosire](https://github.com/crosire) (Patrick Mours)
- **Role**: Post-processing injector (add-on build 6.8.0) and the `ReShade.fxh` / `ReShadeUI.fxh` shader headers.
- **License**: ReShade — BSD 3-Clause License; shader headers — CC0-1.0

### [RenoDX](https://github.com/clshortfuse/renodx)
- **Authors**: [clshortfuse](https://github.com/clshortfuse) and the RenoDX mod authors
- **Role**: Per-game native HDR add-ons for ReShade. Mods are listed in the RenoDX wiki and hosted by their individual
  maintainers.
- **License**: MIT License

### [REFramework](https://github.com/praydog/REFramework)
- **Author**: [praydog](https://github.com/praydog)
- **Role**: Modding framework (`dinput8.dll`) staged for Capcom RE Engine games, from the
  [REFramework nightly builds](https://github.com/praydog/REFramework-nightly).
- **License**: MIT License

### [CAS-for-ReShade](https://github.com/OopyDoopy/CAS-for-Reshade)
- **Author**: [OopyDoopy](https://github.com/OopyDoopy)
- **Role**: ReShade port of AMD FidelityFX Contrast Adaptive Sharpening (`CAS.fx`).
- **License**: MIT License (includes AMD's MIT notice below)

---

## Included in the FSRoneclick binary

### AMD FidelityFX Contrast Adaptive Sharpening (embedded fallback shader)
When `CAS.fx` cannot be downloaded, FSRoneclick writes an embedded CAS shader derived from AMD FidelityFX CAS:

```
Copyright (c) 2017-2019 Advanced Micro Devices, Inc. All rights reserved.

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the Software without restriction, including without limitation
the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to
permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of
the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO
THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### [egui / eframe](https://github.com/emilk/egui)
- **Author**: [Emil Ernerfeldt (emilk)](https://github.com/emilk) and contributors
- **Role**: Immediate-mode GUI framework for the desktop interface.
- **License**: MIT License or Apache License 2.0
- **Bundled fonts** (via `epaint_default_fonts`): Hack (MIT License and Bitstream Vera License),
  Ubuntu Light (Ubuntu Font Licence 1.0), Noto Emoji (SIL Open Font License 1.1), and emoji-icon-font (MIT License).

### Rust crates
The binary statically links Rust crates including reqwest, serde, serde_json, zip, sevenz-rust, regex, anyhow, and
windows-sys, together with their dependencies. They are licensed under permissive licenses (predominantly
MIT and/or Apache-2.0; also BSD, ISC, Zlib, BSL-1.0, Unicode-3.0, CC0-1.0, and Unlicense). Run
`cargo tree --edges normal` for the full list; each crate's license is declared in its `Cargo.toml`.

---

## Data sources

- **OptiScaler compatibility list** — the embedded table in `src/compatibility.rs` and the optional live sync
  (`--update-compat`) are derived from the community-maintained
  [OptiScaler wiki Compatibility List](https://github.com/optiscaler/OptiScaler/wiki/Compatibility-List).
  Thanks to everyone who tests games and documents workarounds there.
- **AMD FSR "Redstone" catalog** — the list of supported titles follows AMD's public list of FSR-supported games.

---

## Trademarks

AMD, Radeon, FidelityFX, and FSR are trademarks of Advanced Micro Devices, Inc. NVIDIA, GeForce, RTX, DLSS, and
Reflex are trademarks of NVIDIA Corporation. Intel, Arc, and XeSS are trademarks of Intel Corporation. All other
names are the property of their respective owners. FSRoneclick is not affiliated with, endorsed by, or sponsored by
any of these companies.
